use serde::Deserialize;

#[cfg(feature = "self-update")]
use {
    crate::utils::NAME,
    log::{debug, error},
    retry::{OperationResult, delay::Fibonacci, retry},
    sha2::{Digest, Sha256},
    std::fs,
    std::io::{self, Read, Write},
    std::path::Path,
    std::path::PathBuf,
    std::time::Duration,
};

#[derive(Debug, Deserialize, Clone)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    #[serde(rename = "browser_download_url")]
    pub download_url: String,
}

#[derive(Default, Debug, Clone)]
pub struct SelfUpdateState {
    pub latest_release: Option<Release>,
    pub status: SelfUpdateStatus,
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub enum SelfUpdateStatus {
    Updating,
    #[default]
    Checking,
    Done,
    Failed,
}

impl std::fmt::Display for SelfUpdateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Checking => "Checking updates...",
            Self::Updating => "Updating...",
            Self::Failed => "Failed to check update!",
            Self::Done => "Done",
        };
        write!(f, "{s}")
    }
}

// Published app releases use stable major.minor.patch tags. Ignore other tags.
#[cfg(any(feature = "self-update", test))]
fn parse_release_version(tag: &str) -> Option<(u64, u64, u64)> {
    let mut parts = tag.strip_prefix('v').unwrap_or(tag).split('.');
    let mut next = || {
        let part = parts.next()?;
        if part.is_empty()
            || !part.bytes().all(|b| b.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        part.parse().ok()
    };
    let version = (next()?, next()?, next()?);
    parts.next().is_none().then_some(version)
}

#[cfg(any(feature = "self-update", test))]
fn is_newer_release(release: &str, current: &str) -> bool {
    match (
        parse_release_version(release),
        parse_release_version(current),
    ) {
        (Some(release), Some(current)) => release > current,
        _ => false,
    }
}

/// Require HTTPS and bound connection setup and stalled reads.
#[cfg(feature = "self-update")]
fn update_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_resolve(Some(Duration::from_secs(15)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        // Allow slow downloads to continue while reads make progress.
        .timeout_per_read(Some(Duration::from_secs(30)))
        .timeout_per_write(Some(Duration::from_secs(30)))
        .build()
        .into()
}

#[cfg(feature = "self-update")]
fn parse_checksum(content: &str, asset_name: &str) -> Result<[u8; 32], String> {
    let line = content.trim_end_matches(['\r', '\n']);
    let (hash, filename) = line
        .split_once("  ")
        .or_else(|| line.split_once(" *"))
        .ok_or("Invalid checksum file")?;
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || filename != asset_name {
        return Err("Invalid checksum or asset name".to_string());
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}

#[cfg(feature = "self-update")]
fn download_verified_asset(
    agent: &ureq::Agent,
    release: &Release,
    asset: &ReleaseAsset,
    destination: &Path,
) -> Result<(), String> {
    // These names are produced by build_artifacts.yml.
    let suffix = if Path::new(&asset.name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        "-checksum.txt"
    } else {
        "-checksum"
    };
    let checksum_name = format!("{}{suffix}", asset.name);
    let checksum_asset = release
        .assets
        .iter()
        .find(|a| a.name == checksum_name)
        .ok_or_else(|| format!("Release has no checksum file {checksum_name}"))?;
    let checksum = agent
        .get(&checksum_asset.download_url)
        .config()
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(4096)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let expected = parse_checksum(&checksum, &asset.name)?;

    debug!("Downloading {}", asset.download_url);
    let mut response = agent
        .get(&asset.download_url)
        .call()
        .map_err(|e| e.to_string())?;
    let mut reader = response.body_mut().as_reader();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])
            .map_err(|e| e.to_string())?;
        hasher.update(&buffer[..count]);
    }
    if hasher.finalize().as_slice() != expected {
        return Err(format!("Checksum mismatch for {}", asset.name));
    }
    Ok(())
}

/// Downloads the latest release file that matches `bin_name`, renames the current
/// executable to a temp path, renames the new version as the original file name,
/// then returns both the original file name (new version) and temp path (old version)
#[cfg(feature = "self-update")]
#[allow(clippy::unused_async, reason = "Called by an Iced task")]
pub async fn download_update_to_temp_file(
    bin_name: &str,
    release: Release,
) -> Result<(PathBuf, PathBuf), ()> {
    let current_bin_path = std::env::current_exe().map_err(|_| ())?;
    install_update(&current_bin_path, bin_name, &release, &update_agent())
        .map(|backup| (current_bin_path, backup))
        .map_err(|e| error!("Couldn't update {NAME}: {e}"))
}

/// Keep downloads private and on the same filesystem as the executable.
#[cfg(feature = "self-update")]
fn staging_dir(directory: &Path) -> io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(".uad-ng-update-");
    // The default directory permissions depend on the umask.
    #[cfg(unix)]
    builder.permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700));
    builder.tempdir_in(directory)
}

#[cfg(feature = "self-update")]
fn install_update(
    current: &Path,
    bin_name: &str,
    release: &Release,
    agent: &ureq::Agent,
) -> Result<PathBuf, String> {
    let directory = current
        .parent()
        .ok_or("Executable has no parent directory")?;
    let staging = staging_dir(directory).map_err(|e| e.to_string())?;
    let asset_name = if cfg!(target_os = "windows") {
        bin_name.to_string()
    } else {
        format!("{bin_name}.tar.gz")
    };
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| format!("Release has no asset {asset_name}"))?;
    let downloaded = staging.path().join("download");
    download_verified_asset(agent, release, asset, &downloaded)?;

    #[cfg(target_os = "windows")]
    let executable = downloaded;
    #[cfg(not(target_os = "windows"))]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        let executable = staging.path().join("executable");
        extract_binary_from_tar(&downloaded, &executable, bin_name).map_err(|e| e.to_string())?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        executable
    };
    if fs::metadata(&executable).map_err(|e| e.to_string())?.len() == 0 {
        return Err("Downloaded executable is empty".to_string());
    }
    replace_executable(current, &executable)
}

#[cfg(feature = "self-update")]
fn replace_executable(current: &Path, downloaded: &Path) -> Result<PathBuf, String> {
    // Windows cannot delete the executable of a running process. With a fixed
    // name, the next update replaces the backup instead of leaving another one.
    let backup = current.with_file_name(".uad-ng-backup");
    rename(current, &backup)?;
    if let Err(error) = rename(downloaded, current) {
        if let Err(restore_error) = rename(&backup, current) {
            return Err(format!(
                "{error}; could not restore executable: {restore_error}. Original saved at {}",
                backup.display()
            ));
        }
        return Err(error);
    }
    Ok(backup)
}

#[cfg(not(feature = "self-update"))]
pub fn get_latest_release() -> Result<Option<Release>, ()> {
    Ok(None)
}

#[cfg(feature = "self-update")]
pub fn get_latest_release() -> Result<Option<Release>, ()> {
    debug!("Checking for {NAME} update");

    if let Ok(mut res) = update_agent().get("https://api.github.com/repos/Universal-Debloater-Alliance/universal-android-debloater-next-generation/releases/latest")
        .config().timeout_global(Some(Duration::from_secs(30))).build()
        .call() {
        let release: Release = serde_json::from_value(
            res.body_mut().read_json::<serde_json::Value>()
                .map_err(|_| ())?
                .clone(),
        )
        .map_err(|_| ())?;

        if is_newer_release(&release.tag_name, env!("CARGO_PKG_VERSION")) {
            Ok(Some(release))
        } else {
            Ok(None)
        }
    } else {
        debug!("Failed to check {NAME} update");
        Err(())
    }
}

/// Extracts the binary from a `tar.gz` archive to `temp_file` path
#[cfg(feature = "self-update")]
#[cfg(not(target_os = "windows"))]
fn extract_binary_from_tar(
    archive_path: &Path,
    temp_file: &Path,
    bin_name: &str,
) -> io::Result<()> {
    use flate2::read::GzDecoder;
    use std::fs::File;
    use tar::Archive;
    let mut archive = Archive::new(GzDecoder::new(File::open(archive_path)?));

    let mut found = false;

    for file in archive.entries()? {
        let mut file = file?;

        if file.path()?.as_ref() == Path::new(bin_name) {
            if found || !file.header().entry_type().is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid executable entry",
                ));
            }
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(temp_file)?;
            io::copy(&mut file, &mut output)?;
            found = true;
        }
    }
    // Read through the gzip trailer so truncated or corrupt archives fail too.
    let mut decoder = archive.into_inner();
    io::copy(&mut decoder, &mut io::sink())?;
    if found {
        Ok(())
    } else {
        Err(io::ErrorKind::NotFound.into())
    }
}

/// Hardcoded binary names for each compilation target
/// that gets published to the GitHub Release
#[cfg(feature = "self-update")]
pub const BIN_NAME: &str = {
    #[cfg(target_os = "windows")]
    {
        "uad-ng-windows.exe"
    }

    #[cfg(all(target_os = "macos", any(target_arch = "x86_64", target_arch = "x86")))]
    {
        "uad-ng-macos-intel"
    }

    #[cfg(all(target_os = "macos", any(target_arch = "arm", target_arch = "aarch64")))]
    {
        "uad-ng-macos"
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "uad-ng-linux"
    }
};

/// Rename a file or directory to a new name, retrying if the operation fails because of permissions
///
/// Will retry for ~30 seconds with longer and longer delays between each, to allow for virus scan
/// and other automated operations to complete.
#[cfg(feature = "self-update")]
pub fn rename<F, T>(from: F, to: T) -> Result<(), String>
where
    F: AsRef<Path>,
    T: AsRef<Path>,
{
    // 21 Fibonacci steps starting at 1 ms is ~28 seconds total
    // See https://github.com/rust-lang/rustup/pull/1873 where this was used by Rustup to work around
    // virus scanning file locks
    let from = from.as_ref();
    let to = to.as_ref();

    retry(Fibonacci::from_millis(1).take(21), || {
        match fs::rename(from, to) {
            Ok(()) => OperationResult::Ok(()),
            Err(e) => match e.kind() {
                io::ErrorKind::PermissionDenied => OperationResult::Retry(e),
                _ => OperationResult::Err(e),
            },
        }
    })
    .map_err(|e| e.to_string())
}

/// Remove a file, retrying if the operation fails because of permissions
///
/// Will retry for ~30 seconds with longer and longer delays between each, to allow for virus scan
/// and other automated operations to complete.
#[cfg(feature = "self-update")]
pub fn remove_file<P>(path: P) -> Result<(), String>
where
    P: AsRef<Path>,
{
    // 21 Fibonacci steps starting at 1 ms is ~28 seconds total
    // See https://github.com/rust-lang/rustup/pull/1873 where this was used by Rustup to work around
    // virus scanning file locks
    let path = path.as_ref();

    retry(
        Fibonacci::from_millis(1).take(21),
        || match fs::remove_file(path) {
            Ok(()) => OperationResult::Ok(()),
            Err(e) => match e.kind() {
                io::ErrorKind::PermissionDenied => OperationResult::Retry(e),
                _ => OperationResult::Err(e),
            },
        },
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_versions_compare_numerically() {
        for (release, current) in [
            ("v1.10.0", "1.9.0"),
            ("1.3.10", "1.3.9"),
            ("v2.0.0", "1.99.99"),
        ] {
            assert!(is_newer_release(release, current));
            assert!(!is_newer_release(current, release));
        }
        assert!(!is_newer_release("v1.3.1", "1.3.1"));
    }

    #[test]
    fn malformed_and_prerelease_tags_are_ignored() {
        for tag in [
            "",
            "dev-build",
            "v1.3",
            "v1.3.1.4",
            "v2.0.0-rc.1",
            "v+2.0.0",
            "v02.0.0",
            "v18446744073709551616.0.0",
        ] {
            assert!(parse_release_version(tag).is_none(), "accepted {tag}");
        }
        assert!(!is_newer_release("v2.0.0", "invalid"));
    }
}

#[cfg(all(test, feature = "self-update"))]
mod download_tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    const BIN: &str = "uad-ng-test.exe";
    const ABC_HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[cfg(unix)]
    #[test]
    fn staging_directory_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let staging = staging_dir(dir.path()).unwrap();
        let mode = fs::metadata(staging.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    fn response(body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend(body);
        response
    }

    fn serve(responses: Vec<Vec<u8>>, stall: bool) -> (Release, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for response in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                socket.write_all(&response).unwrap();
                if stall && request.starts_with(b"GET /asset ") {
                    thread::sleep(Duration::from_millis(200));
                }
            }
        });
        let name = if cfg!(target_os = "windows") {
            BIN.to_string()
        } else {
            format!("{BIN}.tar.gz")
        };
        let suffix = if cfg!(target_os = "windows") {
            "-checksum.txt"
        } else {
            "-checksum"
        };
        (
            Release {
                tag_name: "v2.0.0".into(),
                assets: vec![
                    ReleaseAsset {
                        name: name.clone(),
                        download_url: format!("http://{address}/asset"),
                    },
                    ReleaseAsset {
                        name: format!("{name}{suffix}"),
                        download_url: format!("http://{address}/checksum"),
                    },
                ],
            },
            server,
        )
    }

    fn checksum(body: &[u8]) -> Vec<u8> {
        use std::fmt::Write as _;
        let mut hash = String::new();
        for byte in Sha256::digest(body) {
            write!(hash, "{byte:02x}").unwrap();
        }
        let name = if cfg!(target_os = "windows") {
            BIN.to_string()
        } else {
            format!("{BIN}.tar.gz")
        };
        format!("{hash}  {name}\n").into_bytes()
    }

    fn test_agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .proxy(None)
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into()
    }

    #[test]
    fn checksum_requires_the_expected_filename_and_one_valid_hash() {
        for separator in ["  ", " *"] {
            assert_eq!(
                parse_checksum(
                    &format!("{}{separator}{BIN}\r\n", ABC_HASH.to_uppercase()),
                    BIN
                )
                .unwrap()
                .as_slice(),
                Sha256::digest(b"abc").as_slice()
            );
        }
        for content in [
            String::new(),
            format!("{ABC_HASH}  other"),
            format!("{ABC_HASH}  {BIN}\n{ABC_HASH}  {BIN}"),
            format!("{}  {BIN}", "g".repeat(64)),
            format!("1234  {BIN}"),
        ] {
            assert!(parse_checksum(&content, BIN).is_err());
        }
    }

    #[test]
    fn failed_downloads_leave_the_executable_unchanged() {
        let cases = [
            (response(b"wrong"), checksum(b"abc"), false),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\nabc".to_vec(),
                checksum(b"abc"),
                false,
            ),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n".to_vec(),
                checksum(b"abc"),
                true,
            ),
            (
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
                checksum(b"abc"),
                false,
            ),
            (response(b""), checksum(b""), false),
        ];
        for (asset, digest, stall) in cases {
            let dir = tempfile::tempdir().unwrap();
            let current = dir.path().join(BIN);
            fs::write(&current, b"original").unwrap();
            let (release, server) = serve(vec![response(&digest), asset], stall);
            let agent = if stall {
                ureq::Agent::config_builder()
                    .proxy(None)
                    .timeout_per_read(Some(Duration::from_millis(100)))
                    .timeout_global(Some(Duration::from_secs(5)))
                    .build()
                    .into()
            } else {
                test_agent()
            };
            assert!(install_update(&current, BIN, &release, &agent).is_err());
            server.join().unwrap();
            assert_eq!(fs::read(&current).unwrap(), b"original");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn missing_or_malformed_checksums_cannot_replace_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join(BIN);
        fs::write(&current, b"original").unwrap();
        let (mut release, server) = serve(vec![response(b"invalid")], false);
        assert!(install_update(&current, BIN, &release, &test_agent()).is_err());
        server.join().unwrap();
        release.assets.pop();
        assert!(install_update(&current, BIN, &release, &test_agent()).is_err());
        assert_eq!(fs::read(&current).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn production_client_rejects_plain_http() {
        assert!(matches!(
            update_agent().get("http://127.0.0.1/asset").call(),
            Err(ureq::Error::RequireHttpsOnly(_))
        ));
    }

    #[test]
    fn failed_installation_restores_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join(BIN);
        fs::write(&current, b"original").unwrap();
        assert!(replace_executable(&current, &dir.path().join("missing")).is_err());
        assert_eq!(fs::read(&current).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(not(target_os = "windows"))]
    fn archive(entries: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        for (name, kind, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(*kind);
            header.set_size(bytes.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            archive.append_data(&mut header, name, *bytes).unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn verified_download_replaces_the_executable_and_retains_a_backup() {
        #[cfg(target_os = "windows")]
        let asset = b"new executable".to_vec();
        #[cfg(not(target_os = "windows"))]
        let asset = archive(&[
            ("README", tar::EntryType::Regular, b"notes"),
            (BIN, tar::EntryType::Regular, b"new executable"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join(BIN);
        fs::write(&current, b"original").unwrap();
        let (release, server) = serve(vec![response(&checksum(&asset)), response(&asset)], false);
        let backup = install_update(&current, BIN, &release, &test_agent()).unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(&current).unwrap(), b"new executable");
        assert_eq!(fs::read(&backup).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);

        // Windows cannot delete the backup while it runs: the next update replaces it.
        let newer = dir.path().join("newer");
        fs::write(&newer, b"newer executable").unwrap();
        assert_eq!(replace_executable(&current, &newer).unwrap(), backup);
        assert_eq!(fs::read(&backup).unwrap(), b"new executable");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn invalid_archives_cannot_replace_the_executable() {
        let valid = archive(&[(BIN, tar::EntryType::Regular, b"new executable")]);
        let cases = [
            b"not an archive".to_vec(),
            archive(&[("other", tar::EntryType::Regular, b"wrong file")]),
            archive(&[(BIN, tar::EntryType::Symlink, b"")]),
            archive(&[
                (BIN, tar::EntryType::Regular, b"one"),
                (BIN, tar::EntryType::Regular, b"two"),
            ]),
            valid[..valid.len() - 4].to_vec(),
        ];
        for asset in cases {
            let dir = tempfile::tempdir().unwrap();
            let current = dir.path().join(BIN);
            fs::write(&current, b"original").unwrap();
            let (release, server) =
                serve(vec![response(&checksum(&asset)), response(&asset)], false);
            assert!(install_update(&current, BIN, &release, &test_agent()).is_err());
            server.join().unwrap();
            assert_eq!(fs::read(&current).unwrap(), b"original");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }
}

use serde::Deserialize;

#[cfg(feature = "self-update")]
use {
    crate::{NAME, http_client},
    log::{debug, error},
    retry::{OperationResult, delay::Fibonacci, retry},
    sha2::{Digest, Sha256},
    std::fs,
    std::io::{self, Read, Write},
    std::path::Path,
    std::path::PathBuf,
    std::time::Duration,
};

/// GitHub API endpoint of the newest published release.
#[cfg(feature = "self-update")]
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/Universal-Debloater-Alliance/universal-android-debloater-next-generation/releases/latest";

/// Time allowed for small API and checksum requests.
#[cfg(feature = "self-update")]
const METADATA_TIMEOUT: Duration = Duration::from_secs(30);

/// Time allowed for downloading a release binary or archive.
#[cfg(feature = "self-update")]
const ASSET_TIMEOUT: Duration = Duration::from_secs(600);

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
            Self::Checking => "Checking for updates...",
            Self::Updating => "Updating...",
            Self::Failed => "Could not check for updates",
            Self::Done => "Done",
        };
        write!(f, "{s}")
    }
}

/// `major.minor.patch` of a release tag such as `v1.2.3`, or `None` for other tags.
#[must_use]
pub fn parse_version(tag: &str) -> Option<(u64, u64, u64)> {
    let mut parts = tag.strip_prefix('v').unwrap_or(tag).split('.');
    let mut next = || parts.next()?.parse().ok();
    let version = (next()?, next()?, next()?);
    parts.next().is_none().then_some(version)
}

/// Whether `release_tag` names a newer version than `current_version`.
/// Compares numbers, so `1.10.0` is newer than `1.9.0`.
#[must_use]
pub fn is_newer_release(release_tag: &str, current_version: &str) -> bool {
    match (parse_version(release_tag), parse_version(current_version)) {
        (Some(release), Some(current)) => release > current,
        _ => false,
    }
}

/// Downloads `asset` to `dest_file` and checks it against the SHA-256 file that
/// every release publishes next to it, so a corrupted or tampered download never
/// replaces the running executable (CWE-494). On a mismatch the file is deleted.
#[cfg(feature = "self-update")]
#[allow(clippy::unused_async, reason = "`.call` is equivalent to `.await`")]
pub async fn download_verified_asset(
    release: &Release,
    asset: &ReleaseAsset,
    dest_file: PathBuf,
) -> Result<(), String> {
    let checksum_name = checksum_asset_name(&asset.name);
    let checksum_asset = release
        .assets
        .iter()
        .find(|a| a.name == checksum_name)
        .ok_or_else(|| format!("Release has no checksum file {checksum_name}"))?;
    let checksum_file = http_client::agent(METADATA_TIMEOUT)
        .get(&checksum_asset.download_url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(4096)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let expected = parse_sha256_file(&checksum_file)?;

    debug!("downloading file from {}", asset.download_url);
    let mut response = http_client::agent(ASSET_TIMEOUT)
        .get(&asset.download_url)
        .call()
        .map_err(|e| e.to_string())?;
    let mut reader = response.body_mut().as_reader();
    let mut file = fs::File::create(&dest_file).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let copied = loop {
        match reader.read(&mut buffer) {
            Ok(0) => break file.flush(),
            Ok(read) => {
                hasher.update(&buffer[..read]);
                if let Err(e) = file.write_all(&buffer[..read]) {
                    break Err(e);
                }
            }
            Err(e) => break Err(e),
        }
    };
    drop(file);

    let actual = to_lower_hex(&hasher.finalize());
    if let Err(e) = copied {
        let _ = fs::remove_file(&dest_file);
        return Err(e.to_string());
    }
    if actual != expected {
        let _ = fs::remove_file(&dest_file);
        return Err(format!(
            "Checksum mismatch for {}: expected {expected}, got {actual}",
            asset.name
        ));
    }
    Ok(())
}

/// Name of the checksum file the release workflow publishes for `asset_name`
/// (`.github/workflows/build_artifacts.yml`).
#[cfg(feature = "self-update")]
fn checksum_asset_name(asset_name: &str) -> String {
    if Path::new(asset_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        format!("{asset_name}-checksum.txt")
    } else {
        format!("{asset_name}-checksum")
    }
}

/// The hash from `sha256sum` output (`<64 hex digits>  <file name>`), lowercased.
#[cfg(feature = "self-update")]
fn parse_sha256_file(content: &str) -> Result<String, String> {
    content
        .split_whitespace()
        .next()
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "Checksum file is not in sha256sum format".to_string())
}

#[cfg(feature = "self-update")]
fn to_lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

/// Downloads the latest release file that matches `bin_name`, renames the current
/// executable to a temp path, renames the new version as the original file name,
/// then returns both the original file name (new version) and temp path (old version)
#[cfg(feature = "self-update")]
pub async fn download_update_to_temp_file(
    bin_name: &str,
    release: Release,
) -> Result<(PathBuf, PathBuf), ()> {
    let current_bin_path = std::env::current_exe().map_err(|_| ())?;

    // Path to download the new version to
    let download_path = current_bin_path
        .parent()
        .ok_or(())?
        .join(format!("tmp_{bin_name}"));

    // Path to temporarily force rename current process to, se we can then
    // rename `download_path` to `current_bin_path` and then launch new version
    // cleanly as `current_bin_path`
    let tmp_path = current_bin_path
        .parent()
        .ok_or(())?
        .join(format!("tmp2_{bin_name}"));

    // MacOS and Linux release are gziped tarball
    #[cfg(not(target_os = "windows"))]
    {
        let asset_name = format!("{bin_name}.tar.gz");

        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .cloned()
            .ok_or(())?;

        let archive_path = current_bin_path.parent().ok_or(())?.join(&asset_name);

        if let Err(e) = download_verified_asset(&release, &asset, archive_path.clone()).await {
            error!("Couldn't download {NAME} update: {e}");
            return Err(());
        }

        if extract_binary_from_tar(&archive_path, &download_path).is_err() {
            error!("Couldn't extract {NAME} release tarball");
            return Err(());
        }

        std::fs::remove_file(&archive_path).map_err(|_| ())?;
    }

    // For Windows we download the new binary directly
    #[cfg(target_os = "windows")]
    {
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == bin_name)
            .cloned()
            .ok_or(())?;

        if let Err(e) = download_verified_asset(&release, &asset, download_path.clone()).await {
            error!("Couldn't download {NAME} update: {e}");
            return Err(());
        }
    }

    // Make the file executable
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&download_path).map_err(|_| ())?.permissions();
        permissions.set_mode(0o755);
        if let Err(e) = fs::set_permissions(&download_path, permissions) {
            error!("[SelfUpdate] Couldn't set permission to temp file: {e}");
            return Err(());
        }
    }

    if let Err(e) = rename(&current_bin_path, &tmp_path) {
        error!("[SelfUpdate] Couldn't rename from current to temporary binary path: {e}");
        return Err(());
    }
    if let Err(e) = rename(&download_path, &current_bin_path) {
        error!("[SelfUpdate] Couldn't rename from downloaded to current binary path: {e}");
        return Err(());
    }

    Ok((current_bin_path, tmp_path))
}

#[cfg(not(feature = "self-update"))]
pub fn get_latest_release() -> Result<Option<Release>, ()> {
    Ok(None)
}

#[cfg(feature = "self-update")]
pub fn get_latest_release() -> Result<Option<Release>, ()> {
    debug!("Checking for {NAME} update");

    let mut response = http_client::agent(METADATA_TIMEOUT)
        .get(LATEST_RELEASE_URL)
        .call()
        .map_err(|e| debug!("Failed to check {NAME} update: {e}"))?;
    let release: Release = response
        .body_mut()
        .read_json()
        .map_err(|e| debug!("Unexpected release metadata: {e}"))?;

    Ok(is_newer_release(&release.tag_name, env!("CARGO_PKG_VERSION")).then_some(release))
}

/// Extracts the binary from a `tar.gz` archive to `temp_file` path
#[cfg(feature = "self-update")]
#[cfg(not(target_os = "windows"))]
pub fn extract_binary_from_tar(archive_path: &Path, temp_file: &Path) -> io::Result<()> {
    use flate2::read::GzDecoder;
    use std::fs::File;
    use tar::Archive;
    let mut archive = Archive::new(GzDecoder::new(File::open(archive_path)?));

    let mut temp_file = File::create(temp_file)?;

    for file in archive.entries()? {
        let mut file = file?;

        let path = file.path()?;
        if path.to_str().is_some() {
            io::copy(&mut file, &mut temp_file)?;
            return Ok(());
        }
    }
    Err(io::ErrorKind::NotFound.into())
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

/// Runs `operation`, retrying while it fails with `PermissionDenied`.
///
/// Retries for ~28 seconds (21 Fibonacci steps from 1 ms) with longer and longer
/// delays, so a virus scan or another automated process can release its file lock.
/// Rustup uses the same approach: <https://github.com/rust-lang/rustup/pull/1873>
#[cfg(feature = "self-update")]
fn retry_while_locked(mut operation: impl FnMut() -> io::Result<()>) -> Result<(), String> {
    retry(Fibonacci::from_millis(1).take(21), || match operation() {
        Ok(()) => OperationResult::Ok(()),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => OperationResult::Retry(e),
        Err(e) => OperationResult::Err(e),
    })
    .map_err(|e| e.to_string())
}

/// Rename a file or directory, retrying while another process holds it.
#[cfg(feature = "self-update")]
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<(), String> {
    retry_while_locked(|| fs::rename(from.as_ref(), to.as_ref()))
}

/// Remove a file, retrying while another process holds it.
#[cfg(feature = "self-update")]
pub fn remove_file(path: impl AsRef<Path>) -> Result<(), String> {
    retry_while_locked(|| fs::remove_file(path.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers_not_text() {
        assert!(is_newer_release("v1.10.0", "1.9.0"));
        assert!(is_newer_release("1.3.10", "1.3.9"));
        assert!(!is_newer_release("v1.3.1", "1.3.1"));
        assert!(!is_newer_release("v1.2.9", "1.3.0"));
        assert!(!is_newer_release("dev-build", "1.3.1"));
        assert!(!is_newer_release("v1.3", "1.3.1"));
        assert!(!is_newer_release("v1.3.1.4", "1.3.1"));
    }

    #[cfg(feature = "self-update")]
    #[test]
    fn checksum_files_match_the_release_workflow_names() {
        assert_eq!(
            checksum_asset_name("uad-ng-windows.exe"),
            "uad-ng-windows.exe-checksum.txt"
        );
        assert_eq!(
            checksum_asset_name("uad-ng-linux.tar.gz"),
            "uad-ng-linux.tar.gz-checksum"
        );
    }

    #[cfg(feature = "self-update")]
    #[test]
    fn checksum_file_parsing_rejects_malformed_content() {
        let hash = "A".repeat(64);
        assert_eq!(
            parse_sha256_file(&format!("{hash}  uad-ng-linux.tar.gz")),
            Ok("a".repeat(64))
        );
        assert!(parse_sha256_file("").is_err());
        assert!(parse_sha256_file("abc  file").is_err());
        assert!(parse_sha256_file(&"g".repeat(64)).is_err());
    }

    #[cfg(feature = "self-update")]
    #[test]
    fn sha256_hex_matches_a_known_vector() {
        assert_eq!(
            to_lower_hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

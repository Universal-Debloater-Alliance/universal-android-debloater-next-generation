use crate::core::utils::NAME;
use serde::Deserialize;
use retry::{OperationResult, delay::Fibonacci};
use std::fs;
use std::io;
use std::io::copy;
use std::path::Path;
use std::path::PathBuf;

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

#[derive(Debug)]
pub enum UpdateError {
    Network(String),
    JsonParse(String),
    FileIo(String),
    InvalidVersion(String),
    RateLimit,
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::Network(e) => write!(f, "Network error: {}", e),
            UpdateError::JsonParse(e) => write!(f, "JSON parsing error: {}", e),
            UpdateError::FileIo(e) => write!(f, "File I/O error: {}", e),
            UpdateError::InvalidVersion(e) => write!(f, "Invalid version: {}", e),
            UpdateError::RateLimit => write!(f, "GitHub API rate limit exceeded"),
        }
    }
}

/// Download a file from the internet
#[cfg(feature = "self-update")]
#[allow(clippy::unused_async, reason = "`.call` is equivalent to `.await`")]
pub async fn download_file(url: &str, dest_file: PathBuf) -> Result<(), UpdateError> {
    debug!("downloading file from {url}");

    let result = retry(Fibonacci::from_millis(100).take(5), || {
        match ureq::get(url).timeout(std::time::Duration::from_secs(10)).call() {
            Ok(response) => {
                if response.status() == 429 {
                    OperationResult::Retry(UpdateError::RateLimit)
                } else if response.status() == 200 {
                    OperationResult::Ok(response)
                } else {
                    OperationResult::Err(UpdateError::Network(format!("HTTP {}", response.status())))
                }
            }
            Err(e) => OperationResult::Err(UpdateError::Network(e.to_string())),
        }
    });

    match result {
        Ok(response) => {
            let mut file = fs::File::create(&dest_file).map_err(|e| UpdateError::FileIo(e.to_string()))?;
            copy(&mut response.body_mut().as_reader(), &mut file)
                .map_err(|e| UpdateError::FileIo(e.to_string()))?;
            Ok(())
        }
        Err(UpdateError::RateLimit) => Err(UpdateError::RateLimit),
        Err(e) => Err(e),
    }
}

/// Downloads the latest release file that matches `bin_name`, renames the current
/// executable to a temp path, renames the new version as the original file name,
/// then returns both the original file name (new version) and temp path (old version)
#[cfg(feature = "self-update")]
pub async fn download_update_to_temp_file(
    bin_name: &str,
    release: Release,
) -> Result<(PathBuf, PathBuf), UpdateError> {
    let current_bin_path = std::env::current_exe().map_err(|e| UpdateError::FileIo(e.to_string()))?;

    let download_path = current_bin_path
        .parent()
        .ok_or(UpdateError::FileIo("No parent directory".to_string()))?
        .join(format!("tmp_{bin_name}"));

    let tmp_path = current_bin_path
        .parent()
        .ok_or(UpdateError::FileIo("No parent directory".to_string()))?
        .join(format!("tmp2_{bin_name}"));

    #[cfg(not(target_os = "windows"))]
    {
        let asset_name = format!("{bin_name}.tar.gz");
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .cloned()
            .ok_or(UpdateError::FileIo("Asset not found".to_string()))?;

        let archive_path = current_bin_path.parent().ok_or(UpdateError::FileIo("No parent directory".to_string()))?.join(&asset_name);

        download_file(&asset.download_url, archive_path.clone()).await?;
        extract_binary_from_tar(&archive_path, &download_path).map_err(|e| UpdateError::FileIo(e.to_string()))?;
        fs::remove_file(&archive_path).map_err(|e| UpdateError::FileIo(e.to_string()))?;
    }

    #[cfg(target_os = "windows")]
    {
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == bin_name)
            .cloned()
            .ok_or(UpdateError::FileIo("Asset not found".to_string()))?;

        download_file(&asset.download_url, download_path.clone()).await?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&download_path).map_err(|e| UpdateError::FileIo(e.to_string()))?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&download_path, permissions).map_err(|e| UpdateError::FileIo(e.to_string()))?;
    }

    rename(&current_bin_path, &tmp_path)?;
    rename(&download_path, &current_bin_path)?;

    Ok((current_bin_path, tmp_path))
}

#[cfg(not(feature = "self-update"))]
pub fn get_latest_release() -> Result<Option<Release>, ()> {
    Ok(None)
}

#[cfg(feature = "self-update")]
pub fn get_latest_release() -> Result<Option<Release>, UpdateError> {
    debug!("Checking for {NAME} update");

    let result = retry(Fibonacci::from_millis(100).take(5), || {
        match ureq::get("https://api.github.com/repos/Universal-Debloater-Alliance/universal-android-debloater/releases/latest")
            .timeout(std::time::Duration::from_secs(10))
            .call()
        {
            Ok(response) => {
                if response.status() == 429 {
                    OperationResult::Retry(UpdateError::RateLimit)
                } else if response.status() == 200 {
                    OperationResult::Ok(response)
                } else {
                    OperationResult::Err(UpdateError::Network(format!("HTTP {}", response.status())))
                }
            }
            Err(e) => OperationResult::Err(UpdateError::Network(e.to_string())),
        }
    });

    match result {
        Ok(response) => {
            let json = response.body_mut().read_json::<serde_json::Value>()
                .map_err(|e| UpdateError::JsonParse(e.to_string()))?;
            let release: Release = serde_json::from_value(json)
                .map_err(|e| UpdateError::JsonParse(e.to_string()))?;

            let release_version = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
            if release_version != "dev-build" && release_version > env!("CARGO_PKG_VERSION") {
                Ok(Some(release))
            } else {
                Ok(None)
            }
        }
        Err(UpdateError::RateLimit) => Err(UpdateError::RateLimit),
        Err(e) => Err(e),
    }
}

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
        if path.to_str().map(|s| s.contains("uad-ng")).unwrap_or(false) {
            io::copy(&mut file, &mut temp_file)?;
            return Ok(());
        }
    }
    Err(io::ErrorKind::NotFound.into())
}

#[cfg(feature = "self-update")]
pub const BIN_NAME: &str = {
    #[cfg(target_os = "windows")]
    { "uad-ng-windows.exe" }
    #[cfg(all(target_os = "macos", any(target_arch = "x86_64", target_arch = "x86")))]
    { "uad-ng-macos-intel" }
    #[cfg(all(target_os = "macos", any(target_arch = "arm", target_arch = "aarch64")))]
    { "uad-ng-macos" }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    { "uad-ng-linux" }
};

#[cfg(feature = "self-update")]
pub fn rename<F, T>(from: F, to: T) -> Result<(), String>
where
    F: AsRef<Path>,
    T: AsRef<Path>,
{
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

#[cfg(feature = "self-update")]
pub fn remove_file<P>(path: P) -> Result<(), String>
where
    P: AsRef<Path>,
{
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

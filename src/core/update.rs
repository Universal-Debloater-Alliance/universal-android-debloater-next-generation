use crate::core::utils::NAME;
use serde::Deserialize;
use retry::{OperationResult, delay::Fibonacci};
use std::fs;
use std::io;
use std::io::copy;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

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
    RateLimit(u64), // Include Retry-After duration in seconds
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::Network(e) => write!(f, "Network error: {}", e),
            UpdateError::JsonParse(e) => write!(f, "JSON parsing error: {}", e),
            UpdateError::FileIo(e) => write!(f, "File I/O error: {}", e),
            UpdateError::InvalidVersion(e) => write!(f, "Invalid version: {}", e),
            UpdateError::RateLimit(seconds) => write!(f, "GitHub API rate limit exceeded, retry after {} seconds", seconds),
        }
    }
}

/// Download a file from the internet
#[cfg(feature = "self-update")]
#[allow(clippy::unused_async, reason = "`.call` is equivalent to `.await`")]
pub async fn download_file(url: &str, dest_file: PathBuf) -> Result<(), UpdateError> {
    debug!("Downloading file from {}", url);

    let result = retry(Fibonacci::from_millis(100).take(5), || {
        match ureq::get(url)
            .timeout(Duration::from_secs(15)) // Increased timeout for CI
            .set("User-Agent", &format!("{}/{}", NAME, env!("CARGO_PKG_VERSION"))) // Proper User-Agent
            .call()
        {
            Ok(response) => {
                if response.status() == 429 {
                    // Check Retry-After header
                    let retry_after = response
                        .header("Retry-After")
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(60); // Default to 60 seconds
                    debug!("Rate limit hit, retry after {} seconds", retry_after);
                    OperationResult::Retry(UpdateError::RateLimit(retry_after))
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
            let mut file = fs::File::create(&dest_file).map_err(|e| UpdateError::FileIo(format!("Failed to create file {}: {}", dest_file.display(), e)))?;
            copy(&mut response.into_reader(), &mut file)
                .map_err(|e| UpdateError::FileIo(format!("Failed to write to file {}: {}", dest_file.display(), e)))?;
            debug!("Successfully downloaded file to {}", dest_file.display());
            Ok(())
        }
        Err(UpdateError::RateLimit(seconds)) => {
            std::thread::sleep(Duration::from_secs(seconds)); // Respect Retry-After
            Err(UpdateError::RateLimit(seconds))
        }
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
    let current_bin_path = std::env::current_exe()
        .map_err(|e| UpdateError::FileIo(format!("Failed to get current executable: {}", e)))?;

    let download_path = current_bin_path
        .parent()
        .ok_or(UpdateError::FileIo("No parent directory for current executable".to_string()))?
        .join(format!("tmp_{bin_name}"));

    let tmp_path = current_bin_path
        .parent()
        .ok_or(UpdateError::FileIo("No parent directory for current executable".to_string()))?
        .join(format!("tmp2_{bin_name}"));

    #[cfg(not(target_os = "windows"))]
    {
        let asset_name = format!("{bin_name}.tar.gz");
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .cloned()
            .ok_or(UpdateError::FileIo(format!("Asset {} not found", asset_name)))?;

        let archive_path = current_bin_path
            .parent()
            .ok_or(UpdateError::FileIo("No parent directory".to_string()))?
            .join(&asset_name);

        debug!("Downloading archive to {}", archive_path.display());
        download_file(&asset.download_url, archive_path.clone()).await?;
        debug!("Extracting binary from {}", archive_path.display());
        extract_binary_from_tar(&archive_path, &download_path)
            .map_err(|e| UpdateError::FileIo(format!("Failed to extract tar: {}", e)))?;
        debug!("Removing archive {}", archive_path.display());
        fs::remove_file(&archive_path)
            .map_err(|e| UpdateError::FileIo(format!("Failed to remove archive {}: {}", archive_path.display(), e)))?;
    }

    #[cfg(target_os = "windows")]
    {
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == bin_name)
            .cloned()
            .ok_or(UpdateError::FileIo(format!("Asset {} not found", bin_name)))?;

        debug!("Downloading Windows binary to {}", download_path.display());
        download_file(&asset.download_url, download_path.clone()).await?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&download_path)
            .map_err(|e| UpdateError::FileIo(format!("Failed to get metadata for {}: {}", download_path.display(), e)))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&download_path, permissions)
            .map_err(|e| UpdateError::FileIo(format!("Failed to set permissions for {}: {}", download_path.display(), e)))?;
    }

    debug!("Renaming current binary {} to {}", current_bin_path.display(), tmp_path.display());
    rename(&current_bin_path, &tmp_path)
        .map_err(|e| UpdateError::FileIo(format!("Failed to rename binary: {}", e)))?;
    debug!("Renaming downloaded binary {} to {}", download_path.display(), current_bin_path.display());
    rename(&download_path, &current_bin_path)
        .map_err(|e| UpdateError::FileIo(format!("Failed to rename downloaded binary: {}", e)))?;

    Ok((current_bin_path, tmp_path))
}

#[cfg(not(feature = "self-update"))]
pub fn get_latest_release() -> Result<Option<Release>, ()> {
    Ok(None)
}

#[cfg(feature = "self-update")]
pub fn get_latest_release() -> Result<Option<Release>, UpdateError> {
    debug!("Checking for {} update", NAME);

    let result = retry(Fibonacci::from_millis(100).take(5), || {
        match ureq::get("https://api.github.com/repos/Universal-Debloater-Alliance/universal-android-debloater/releases/latest")
            .timeout(Duration::from_secs(15)) // Increased timeout for CI
            .set("User-Agent", &format!("{}/{}", NAME, env!("CARGO_PKG_VERSION"))) // Proper User-Agent
            .call()
        {
            Ok(response) => {
                if response.status() == 429 {
                    // Check Retry-After header
                    let retry_after = response
                        .header("Retry-After")
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(60); // Default to 60 seconds
                    debug!("Rate limit hit, retry after {} seconds", retry_after);
                    OperationResult::Retry(UpdateError::RateLimit(retry_after))
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
            let body = response.into_string().map_err(|e| UpdateError::JsonParse(format!("Failed to read response: {}", e)))?;
            if body.is_empty() {
                return Err(UpdateError::JsonParse("Empty response from GitHub API".to_string()));
            }
            let json = serde_json::from_str::<serde_json::Value>(&body)
                .map_err(|e| UpdateError::JsonParse(format!("Failed to parse JSON: {}", e)))?;
            let release: Release = serde_json::from_value(json)
                .map_err(|e| UpdateError::JsonParse(format!("Failed to deserialize release: {}", e)))?;

            let release_version = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
            if release_version != "dev-build" && release_version > env!("CARGO_PKG_VERSION") {
                debug!("Found newer release: {}", release_version);
                Ok(Some(release))
            } else {
                debug!("No newer release found (current: {}, latest: {})", env!("CARGO_PKG_VERSION"), release_version);
                Ok(None)
            }
        }
        Err(UpdateError::RateLimit(seconds)) => {
            std::thread::sleep(Duration::from_secs(seconds)); // Respect Retry-After
            Err(UpdateError::RateLimit(seconds))
        }
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
            debug!("Extracting binary from tar: {}", path.display());
            io::copy(&mut file, &mut temp_file)?;
            return Ok(());
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "Binary not found in archive"))
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
    debug!("Renaming {} to {}", from.display(), to.display());
    retry(Fibonacci::from_millis(1).take(21), || {
        match fs::rename(from, to) {
            Ok(()) => OperationResult::Ok(()),
            Err(e) => match e.kind() {
                io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound => OperationResult::Retry(e),
                _ => OperationResult::Err(e),
            },
        }
    })
    .map_err(|e| format!("Failed to rename {} to {}: {}", from.display(), to.display(), e))
}

#[cfg(feature = "self-update")]
pub fn remove_file<P>(path: P) -> Result<(), String>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    debug!("Removing file {}", path.display());
    retry(
        Fibonacci::from_millis(1).take(21),
        || match fs::remove_file(path) {
            Ok(()) => OperationResult::Ok(()),
            Err(e) => match e.kind() {
                io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound => OperationResult::Retry(e),
                _ => OperationResult::Err(e),
            },
        },
    )
    .map_err(|e| format!("Failed to remove {}: {}", path.display(), e))
}

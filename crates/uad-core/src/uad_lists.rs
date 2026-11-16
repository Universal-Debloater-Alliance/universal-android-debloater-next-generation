use crate::CACHE_DIR;
use crate::utils::{format_diff_time_from_now, last_modified_date};
use log::warn;
use retry::{OperationResult, delay::Fixed, retry};
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

pub const LIST_FNAME: &str = "uad_lists.json";

#[allow(
    clippy::large_include_file,
    reason = "https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/discussions/608"
)]
// not `const`, because it's too big
pub static DATA: &str = include_str!("../../../resources/assets/uad_lists.json");

#[derive(Deserialize, Debug, Clone, PartialEq, Hash, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub list: UadList,
    pub description: String,
    dependencies: Vec<String>,
    needed_by: Vec<String>,
    labels: Vec<String>,
    pub removal: Removal,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UadList {
    #[default]
    All,
    Aosp,
    Carrier,
    Google,
    Misc,
    Oem,
    Pending,
    Unlisted,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum UadListState {
    #[default]
    Downloading,
    Done,
    Failed,
}

impl std::fmt::Display for UadListState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let date = last_modified_date(CACHE_DIR.join(LIST_FNAME));
        let s = match self {
            Self::Downloading => "Checking updates...".to_string(),
            Self::Done => format!("Done (last was {})", format_diff_time_from_now(date)),
            Self::Failed => "Failed to check update!".to_string(),
        };
        write!(f, "{s}")
    }
}

impl UadList {
    pub const ALL: [Self; 8] = [
        Self::All,
        Self::Aosp,
        Self::Carrier,
        Self::Google,
        Self::Misc,
        Self::Oem,
        Self::Pending,
        Self::Unlisted,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "All lists",
            Self::Aosp => "aosp",
            Self::Carrier => "carrier",
            Self::Google => "google",
            Self::Misc => "misc",
            Self::Oem => "oem",
            Self::Pending => "pending",
            Self::Unlisted => "unlisted",
        }
    }
}

impl std::fmt::Display for UadList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<UadList> for Cow<'_, str> {
    fn from(list: UadList) -> Self {
        Cow::Borrowed(list.as_str())
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageState {
    All,
    #[default]
    Enabled,
    Uninstalled,
    Disabled,
}

impl PackageState {
    pub const ALL: [Self; 4] = [Self::All, Self::Enabled, Self::Uninstalled, Self::Disabled];
}

impl std::fmt::Display for PackageState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::All => "All states",
                Self::Enabled => "Enabled",
                Self::Uninstalled => "Uninstalled",
                Self::Disabled => "Disabled",
            }
        )
    }
}

pub trait Opposite {
    fn opposite(&self, disable: bool) -> PackageState;
}

impl Opposite for PackageState {
    fn opposite(&self, disable: bool) -> Self {
        match self {
            Self::Enabled => {
                if disable {
                    Self::Disabled
                } else {
                    Self::Uninstalled
                }
            }
            Self::Uninstalled | Self::Disabled => Self::Enabled,
            Self::All => Self::All,
        }
    }
}

// Bad names. To be changed!
#[derive(Default, Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Removal {
    #[default]
    Recommended,
    Advanced,
    Expert,
    Unsafe,
    Unlisted,
    All,
}

impl Removal {
    pub const ALL: [Self; 6] = [
        Self::All,
        Self::Recommended,
        Self::Advanced,
        Self::Expert,
        Self::Unsafe,
        Self::Unlisted,
    ];
    pub const CATEGORIES: [Self; 5] = [
        Self::Recommended,
        Self::Advanced,
        Self::Expert,
        Self::Unsafe,
        Self::Unlisted,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "All removals",
            Self::Recommended => "Recommended",
            Self::Advanced => "Advanced",
            Self::Expert => "Expert",
            Self::Unsafe => "Unsafe",
            Self::Unlisted => "Unlisted",
        }
    }
}

impl std::fmt::Display for Removal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<Removal> for Cow<'_, str> {
    fn from(list: Removal) -> Self {
        Cow::Borrowed(list.as_str())
    }
}

pub type PackageHashMap = HashMap<String, Package>;
pub fn load_debloat_lists(remote: bool) -> Result<PackageHashMap, PackageHashMap> {
    let cached_uad_lists = CACHE_DIR.join(LIST_FNAME);
    if !remote {
        return Ok(get_local_lists(&cached_uad_lists));
    }

    let agent = list_agent();
    let url = format!(
        "https://raw.githubusercontent.com/Universal-Debloater-Alliance/\
         universal-android-debloater-next-generation/main/resources/assets/{LIST_FNAME}"
    );
    load_remote_list(
        &agent,
        &url,
        &cached_uad_lists,
        Fixed::from_millis(1000).take(60),
    )
}

fn list_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_resolve(Some(Duration::from_secs(15)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        // Bound inactivity, not the time needed for a large download.
        .timeout_per_read(Some(Duration::from_secs(30)))
        .timeout_per_write(Some(Duration::from_secs(30)))
        .build()
        .into()
}

fn load_remote_list(
    agent: &ureq::Agent,
    url: &str,
    cached_uad_lists: &Path,
    delays: impl IntoIterator<Item = Duration>,
) -> Result<PackageHashMap, PackageHashMap> {
    retry(delays, || {
        match agent.get(url).call() {
            Ok(mut response) => {
                // https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/discussions/608
                let text = match response
                    .body_mut()
                    .with_config()
                    .limit(8 * 1024 * 1024)
                    .read_to_string()
                {
                    Ok(text) => text,
                    Err(ureq::Error::BodyExceedsLimit(_)) => {
                        warn!("remote list is bigger than 8MiB");
                        return OperationResult::Err(());
                    }
                    Err(e) => {
                        warn!("Could not read remote debloat list: {e}");
                        return retry_unless_timed_out(&e);
                    }
                };
                match parse_and_cache_list(&text, cached_uad_lists) {
                    Ok(list) => OperationResult::Ok(list),
                    Err(e) => {
                        warn!("Remote debloat list is not valid: {e}");
                        OperationResult::Err(())
                    }
                }
            }
            Err(e) => {
                warn!("Could not load remote debloat list: {e}");
                retry_unless_timed_out(&e)
            }
        }
    })
    .map_err(|_| get_local_lists(cached_uad_lists))
}

/// A connection that stalled would most likely stall again on every retry,
/// delaying the fallback to the local list for minutes.
fn retry_unless_timed_out(e: &ureq::Error) -> OperationResult<PackageHashMap, ()> {
    if matches!(e, ureq::Error::Timeout(_)) {
        OperationResult::Err(())
    } else {
        OperationResult::Retry(())
    }
}

fn parse_and_cache_list(
    text: &str,
    cache_path: &Path,
) -> Result<PackageHashMap, serde_json::Error> {
    let list = serde_json::from_str(text)?;
    // Invalid downloads must not overwrite a usable cached list.
    if let Err(e) = fs::write(cache_path, text) {
        warn!("Could not cache remote debloat list: {e}");
    }
    Ok(list)
}

fn get_local_lists(cache_path: &Path) -> PackageHashMap {
    if let Ok(text) = fs::read_to_string(cache_path) {
        match serde_json::from_str(&text) {
            Ok(list) => return list,
            Err(e) => warn!("Cached debloat list is not valid: {e}"),
        }
    }
    serde_json::from_str(DATA).expect("embedded debloat list must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const TEST_LIST: &str = r#"{
        "com.example.app": {
            "list": "Misc", "description": "Test package", "removal": "Recommended",
            "dependencies": [], "neededBy": [], "labels": []
        }
    }"#;

    #[test]
    fn invalid_download_preserves_cached_list() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(LIST_FNAME);
        fs::write(&path, TEST_LIST).unwrap();
        for invalid in ["{", r#"{"com.example.app": {}}"#] {
            assert!(parse_and_cache_list(invalid, &path).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), TEST_LIST);
        }
    }

    #[test]
    fn valid_download_is_cached_and_used_offline() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(LIST_FNAME);
        let downloaded = parse_and_cache_list(TEST_LIST, &path).unwrap();
        assert_eq!(get_local_lists(&path), downloaded);
        assert_eq!(downloaded.len(), 1);
        assert!(downloaded.contains_key("com.example.app"));
    }

    #[test]
    fn missing_or_invalid_cache_falls_back_to_embedded_list() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(LIST_FNAME);
        let embedded: PackageHashMap = serde_json::from_str(DATA).unwrap();
        assert_eq!(get_local_lists(&path), embedded);
        fs::write(&path, "invalid JSON").unwrap();
        assert_eq!(get_local_lists(&path), embedded);
    }

    #[test]
    fn cache_write_failure_does_not_discard_a_valid_download() {
        let dir = tempdir().unwrap();
        // A directory cannot be overwritten with the downloaded JSON.
        let downloaded = parse_and_cache_list(TEST_LIST, dir.path()).unwrap();
        assert!(downloaded.contains_key("com.example.app"));
        assert!(dir.path().is_dir());
    }

    #[test]
    fn test_parse_json() {
        let _: PackageHashMap = serde_json::from_str(DATA).expect("Unable to parse");
    }

    fn serve_lists(attempts: Vec<(Vec<u8>, Duration)>) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/list", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for (body, delay) in attempts {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    TEST_LIST.len()
                )
                .unwrap();
                for chunk in body.chunks(8) {
                    std::thread::sleep(delay);
                    if stream.write_all(chunk).is_err() {
                        break;
                    }
                }
            }
        });
        (url, server)
    }

    fn local_agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .https_only(false)
            .proxy(None)
            .timeout_per_read(Some(Duration::from_millis(200)))
            .build()
            .into()
    }

    #[test]
    fn a_progressing_list_download_can_exceed_the_idle_timeout() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(LIST_FNAME);
        let (url, server) = serve_lists(vec![(
            TEST_LIST.as_bytes().to_vec(),
            Duration::from_millis(30),
        )]);
        let started = std::time::Instant::now();
        assert!(load_remote_list(&local_agent(), &url, &path, []).is_ok());
        server.join().unwrap();
        assert!(started.elapsed() > Duration::from_millis(200));
        assert_eq!(fs::read_to_string(path).unwrap(), TEST_LIST);
    }

    #[test]
    fn stalled_list_download_falls_back_to_the_cache_without_retrying() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(LIST_FNAME);
        fs::write(&path, TEST_LIST).unwrap();
        let (url, server) = serve_lists(vec![(
            TEST_LIST.as_bytes().to_vec(),
            Duration::from_millis(300),
        )]);
        let mut retries = 0;
        let delays = std::iter::from_fn(|| {
            retries += 1;
            Some(Duration::ZERO)
        })
        .take(3);
        let fallback = load_remote_list(&local_agent(), &url, &path, delays).unwrap_err();
        server.join().unwrap();
        assert_eq!(retries, 0);
        assert_eq!(fallback, serde_json::from_str(TEST_LIST).unwrap());
        assert_eq!(fs::read_to_string(path).unwrap(), TEST_LIST);
    }

    #[test]
    fn interrupted_list_download_retries_before_updating_the_cache() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(LIST_FNAME);
        let (url, server) = serve_lists(vec![
            (TEST_LIST.as_bytes()[..10].to_vec(), Duration::ZERO),
            (TEST_LIST.as_bytes().to_vec(), Duration::ZERO),
        ]);
        assert!(load_remote_list(&local_agent(), &url, &path, [Duration::ZERO]).is_ok());
        server.join().unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), TEST_LIST);
    }
}

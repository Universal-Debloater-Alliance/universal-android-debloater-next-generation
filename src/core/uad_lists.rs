use crate::CACHE_DIR;
use crate::core::utils::{format_diff_time_from_now, last_modified_date};
use retry::{OperationResult, delay::Fixed, retry};
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub const LIST_FNAME: &str = "uad_lists.json";

#[allow(
    clippy::large_include_file,
    reason = "https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/discussions/608"
)]
// not `const`, because it's too big
pub static DATA: &str = include_str!("../../resources/assets/uad_lists.json");

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

#[derive(Default, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Default, Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
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
    let cached_uad_lists: PathBuf = CACHE_DIR.join(LIST_FNAME);
    let mut error = false;
    let list: PackageHashMap = if remote {
        retry(Fixed::from_millis(1000).take(60), || {
            match ureq::get(
                &format!("https://raw.githubusercontent.com/Universal-Debloater-Alliance/universal-android-debloater/\
           main/resources/assets/{LIST_FNAME}"),
            )
            .call()
            {
                Ok(data) => {
                    // TODO: max resp size is 10MB, list is ~1.3MB;
                    // TODO: https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/discussions/608
                    #[warn(clippy::expect_used, reason = "this will panic if GH servers rate-limit the user, or many other reasons.")]
                    let text = data.into_string().expect("response should be Ok type");
                    fs::write(cached_uad_lists.clone(), &text).expect("Unable to write file");
                    let list: PackageHashMap = serde_json::from_str(&text).expect("Unable to parse");
                    OperationResult::Ok(list)
                }
                Err(e) => {
                    warn!("Could not load remote debloat list: {e}");
                    error = true;
                    OperationResult::Retry(PackageHashMap::new())
                }
            }
        })
        .unwrap_or_else(|_| get_local_lists())
    } else {
        warn!("Could not load remote debloat list");
        get_local_lists()
    };

    (if error { Err } else { Ok })(list)
}

fn get_local_lists() -> PackageHashMap {
    let cached_uad_lists = CACHE_DIR.join(LIST_FNAME);
    serde_json::from_str(
        fs::read_to_string(cached_uad_lists)
            .as_deref()
            .unwrap_or(DATA),
    )
    .expect("Unable to parse")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_json() {
        let _: PackageHashMap = serde_json::from_str(DATA).expect("Unable to parse");
    }
}

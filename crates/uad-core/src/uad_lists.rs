use crate::relative_time::{format_diff_time_from_now, last_modified_date};
use crate::{CACHE_DIR, http_client};
use log::warn;
use retry::{OperationResult, delay::Fixed, retry};
use serde::{Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

pub const LIST_FNAME: &str = "uad_lists.json";

/// Time allowed for one download attempt of the remote list.
const LIST_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

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
    /// Packages this package needs in order to work.
    pub dependencies: Vec<String>,
    /// Packages that need this package in order to work.
    pub needed_by: Vec<String>,
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

    /// What the rating means for someone deciding whether to remove a package.
    /// Follows the "Removal definitions" on the project wiki (Debloat-Lists page).
    #[must_use]
    pub const fn explanation(self) -> &'static str {
        match self {
            Self::Recommended => {
                "Safe to remove for most people. Still read the description: you may use this app."
            }
            Self::Advanced => {
                "Breaks minor or obscure features, or removes a useful default app \
                 (keyboard, gallery, launcher) that you should replace first."
            }
            Self::Expert => {
                "Breaks widespread or important features. Should not cause a boot loop, \
                 but this is not guaranteed. Prefer disabling over uninstalling."
            }
            Self::Unsafe => {
                "Can break vital parts of the system and has a very high risk of a boot loop. \
                 Do not remove unless you know exactly what you are doing."
            }
            Self::Unlisted => {
                "Not reviewed by the UAD-ng community yet. Its effect on your device is unknown."
            }
            Self::All => "",
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
        let agent = http_client::agent(LIST_DOWNLOAD_TIMEOUT);
        // About 20 s of retries while offline, then the cached or embedded list.
        retry(Fixed::from_millis(2000).take(10), || {
            match agent
                .get(format!(
                    "https://raw.githubusercontent.com\
                    /Universal-Debloater-Alliance\
                    /universal-android-debloater-next-generation\
                    /main\
                    /resources\
                    /assets\
                    /{LIST_FNAME}"
                ))
                .call()
            {
                Ok(mut data) => {
                    // https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/discussions/608
                    let text = match data
                        .body_mut()
                        .with_config()
                        .limit(1 << (3 + 10 + 10))
                        .read_to_string()
                    {
                        Ok(text) => text,
                        Err(e) => {
                            // Too big (over 8 MiB) or a broken transfer: retrying will not help.
                            warn!("Could not read remote debloat list: {e}");
                            error = true;
                            return OperationResult::Err(());
                        }
                    };
                    match serde_json::from_str::<PackageHashMap>(&text) {
                        Ok(list) => {
                            // Only cache a list that parses, so a bad download never
                            // replaces a good cached copy.
                            if let Err(e) = fs::write(&cached_uad_lists, &text) {
                                warn!("Could not cache remote debloat list: {e}");
                            }
                            OperationResult::Ok(list)
                        }
                        Err(e) => {
                            warn!("Remote debloat list is not valid: {e}");
                            error = true;
                            OperationResult::Err(())
                        }
                    }
                }
                Err(e) => {
                    warn!("Could not load remote debloat list: {e}");
                    error = true;
                    OperationResult::Retry(())
                }
            }
        })
        .unwrap_or_else(|_| get_local_lists())
    } else {
        get_local_lists()
    };

    (if error { Err } else { Ok })(list)
}

/// Loads the cached list, or the list embedded at build time
/// when the cache is missing or cannot be parsed.
fn get_local_lists() -> PackageHashMap {
    let cached_uad_lists = CACHE_DIR.join(LIST_FNAME);
    if let Ok(text) = fs::read_to_string(&cached_uad_lists) {
        match serde_json::from_str(&text) {
            Ok(list) => return list,
            Err(e) => warn!(
                "Cached debloat list {} is not valid ({e}), using the embedded list",
                cached_uad_lists.display()
            ),
        }
    }
    serde_json::from_str(DATA).expect("the embedded list is checked by `test_parse_json`")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::{Deserializer, IgnoredAny, MapAccess, Visitor};

    #[test]
    fn test_parse_json() {
        let _: PackageHashMap = serde_json::from_str(DATA).expect("Unable to parse");
    }

    /// Top-level keys in file order, duplicates included.
    /// (A `HashMap` silently keeps only the last of two duplicate keys.)
    struct Keys(Vec<String>);

    impl<'de> Deserialize<'de> for Keys {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct KeysVisitor;
            impl<'de> Visitor<'de> for KeysVisitor {
                type Value = Keys;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("a JSON object")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Keys, A::Error> {
                    let mut keys = vec![];
                    while let Some((key, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                        keys.push(key);
                    }
                    Ok(Keys(keys))
                }
            }
            deserializer.deserialize_map(KeysVisitor)
        }
    }

    #[test]
    fn no_duplicate_package_names() {
        let Keys(keys) = serde_json::from_str(DATA).expect("Unable to parse");
        let mut seen = std::collections::HashSet::new();
        let duplicates: Vec<&String> = keys.iter().filter(|k| !seen.insert(*k)).collect();
        assert_eq!(duplicates, Vec::<&String>::new());
    }

    #[test]
    fn every_package_has_a_description() {
        let lists: PackageHashMap = serde_json::from_str(DATA).expect("Unable to parse");
        let mut empty: Vec<&String> = lists
            .iter()
            .filter(|(_, p)| p.description.trim().is_empty())
            .map(|(name, _)| name)
            .collect();
        empty.sort();
        assert_eq!(empty, Vec::<&String>::new());
    }

    /// `A.dependencies` lists `B` exactly when `B.neededBy` lists `A`.
    #[test]
    fn relation_links_are_recorded_on_both_packages() {
        let lists: PackageHashMap = serde_json::from_str(DATA).expect("Unable to parse");
        let mut one_sided = vec![];
        for (name, package) in &lists {
            for requirement in &package.dependencies {
                if lists
                    .get(requirement)
                    .is_some_and(|r| !r.needed_by.contains(name))
                {
                    one_sided.push(format!("{name} -> {requirement}"));
                }
            }
            for dependent in &package.needed_by {
                if lists
                    .get(dependent)
                    .is_some_and(|d| !d.dependencies.contains(name))
                {
                    one_sided.push(format!("{dependent} -> {name}"));
                }
            }
        }
        one_sided.sort();
        assert_eq!(one_sided, Vec::<String>::new());
    }
}

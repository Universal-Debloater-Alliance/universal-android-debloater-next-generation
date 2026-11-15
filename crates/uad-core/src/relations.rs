//! Relationships between listed packages, built from the `dependencies` and
//! `neededBy` fields of the debloat list.
//!
//! The list is curated by hand, so a link is sometimes recorded on only one of
//! its two packages. [`Relations`] merges both directions, so either field is
//! enough to relate two packages.

use crate::uad_lists::{PackageHashMap, PackageState, Removal};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// What relation analysis needs to know about a package on the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevicePackage {
    pub state: PackageState,
    pub removal: Removal,
}

/// Why a package is suggested for removal together with the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelatedReason {
    /// The package needs this package, which is being removed, so it would break anyway.
    Needs(String),
    /// The package is only needed by this package, which is being removed.
    OnlyUsedBy(String),
}

impl std::fmt::Display for RelatedReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Needs(package) => write!(f, "needs {package}"),
            Self::OnlyUsedBy(package) => write!(f, "only used by {package}"),
        }
    }
}

/// A package related to the selection, with the reason it is related.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedPackage {
    pub name: String,
    pub reason: RelatedReason,
}

/// An enabled package that needs a package that is about to be removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtRisk {
    /// The package that may stop working.
    pub dependent: String,
    /// The package it needs, which is being removed.
    pub requirement: String,
}

/// The result of [`Relations::plan_removal`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemovalPlan {
    /// Enabled packages that can be removed together with the selection.
    /// Always filled, so the user can see them before opting in.
    pub related: Vec<RelatedPackage>,
    /// Related packages rated `Unsafe`. They are never added automatically.
    pub excluded_unsafe: Vec<RelatedPackage>,
    /// Enabled packages left on the device that need a removed package.
    pub at_risk: Vec<AtRisk>,
}

/// Both directions of every package relationship in the debloat list.
#[derive(Debug, Clone, Default)]
pub struct Relations {
    /// package -> packages it needs
    requirements: HashMap<String, BTreeSet<String>>,
    /// package -> packages that need it
    dependents: HashMap<String, BTreeSet<String>>,
}

impl Relations {
    #[must_use]
    pub fn new(lists: &PackageHashMap) -> Self {
        let mut relations = Self::default();
        for (name, package) in lists {
            for requirement in &package.dependencies {
                relations.link(name, requirement);
            }
            for dependent in &package.needed_by {
                relations.link(dependent, name);
            }
        }
        relations
    }

    /// Records that `dependent` needs `requirement`. Self-links are ignored.
    fn link(&mut self, dependent: &str, requirement: &str) {
        if dependent == requirement {
            return;
        }
        self.requirements
            .entry(dependent.to_string())
            .or_default()
            .insert(requirement.to_string());
        self.dependents
            .entry(requirement.to_string())
            .or_default()
            .insert(dependent.to_string());
    }

    /// Packages that `package` needs, sorted by name.
    #[must_use]
    pub fn requirements(&self, package: &str) -> Vec<&str> {
        self.requirements
            .get(package)
            .map_or_default(|set| set.iter().map(String::as_str).collect())
    }

    /// Packages that need `package`, sorted by name.
    #[must_use]
    pub fn dependents(&self, package: &str) -> Vec<&str> {
        self.dependents
            .get(package)
            .map_or_default(|set| set.iter().map(String::as_str).collect())
    }

    /// Works out which packages are affected when `removing` is removed.
    ///
    /// `device` returns the state of a package for the target user, or `None`
    /// when it is not installed for that user. Only enabled packages count:
    /// a disabled or uninstalled package cannot break and needs no removal.
    ///
    /// A package is related when it is enabled, not already being removed, and either
    /// - needs a package being removed, or
    /// - is needed by a package being removed and by no enabled package that stays.
    ///
    /// The search repeats until nothing new is found, so chains are followed.
    /// `Unsafe` packages are reported in `excluded_unsafe` and never added.
    ///
    /// `at_risk` describes the outcome of the choice in `include_related`.
    #[must_use]
    pub fn plan_removal<F>(
        &self,
        removing: &[String],
        device: F,
        include_related: bool,
    ) -> RemovalPlan
    where
        F: Fn(&str) -> Option<DevicePackage>,
    {
        let is_enabled =
            |name: &str| device(name).is_some_and(|p| p.state == PackageState::Enabled);
        let selection: BTreeSet<&str> = removing.iter().map(String::as_str).collect();
        let mut set = selection.clone();
        let mut related: Vec<RelatedPackage> = vec![];
        let mut unsafe_found: BTreeMap<&str, RelatedReason> = BTreeMap::new();

        loop {
            let mut found: Vec<(&str, RelatedReason)> = vec![];
            for &removed in &set {
                for dependent in self.dependents(removed) {
                    if !set.contains(dependent) && is_enabled(dependent) {
                        found.push((dependent, RelatedReason::Needs(removed.to_string())));
                    }
                }
                for requirement in self.requirements(removed) {
                    if set.contains(requirement) || !is_enabled(requirement) {
                        continue;
                    }
                    let still_needed = self
                        .dependents(requirement)
                        .into_iter()
                        .any(|other| !set.contains(other) && is_enabled(other));
                    if !still_needed {
                        found.push((requirement, RelatedReason::OnlyUsedBy(removed.to_string())));
                    }
                }
            }

            let mut added = false;
            for (name, reason) in found {
                if set.contains(name) || unsafe_found.contains_key(name) {
                    continue;
                }
                if device(name).is_some_and(|p| p.removal == Removal::Unsafe) {
                    unsafe_found.insert(name, reason);
                    continue;
                }
                set.insert(name);
                related.push(RelatedPackage {
                    name: name.to_string(),
                    reason,
                });
                added = true;
            }
            if !added {
                break;
            }
        }

        let removed: BTreeSet<&str> = if include_related { set } else { selection };
        let mut at_risk = vec![];
        for &requirement in &removed {
            for dependent in self.dependents(requirement) {
                if !removed.contains(dependent) && is_enabled(dependent) {
                    at_risk.push(AtRisk {
                        dependent: dependent.to_string(),
                        requirement: requirement.to_string(),
                    });
                }
            }
        }

        RemovalPlan {
            related,
            excluded_unsafe: unsafe_found
                .into_iter()
                .map(|(name, reason)| RelatedPackage {
                    name: name.to_string(),
                    reason,
                })
                .collect(),
            at_risk,
        }
    }

    /// For packages about to be restored or enabled, lists the packages they need
    /// that are installed but not enabled, and not part of `restoring` either.
    /// Each entry is `(package, requirement)`.
    #[must_use]
    pub fn missing_requirements<F>(&self, restoring: &[String], device: F) -> Vec<(String, String)>
    where
        F: Fn(&str) -> Option<DevicePackage>,
    {
        let restoring_set: BTreeSet<&str> = restoring.iter().map(String::as_str).collect();
        let mut missing = vec![];
        for &package in &restoring_set {
            for requirement in self.requirements(package) {
                let not_enabled =
                    device(requirement).is_some_and(|p| p.state != PackageState::Enabled);
                if not_enabled && !restoring_set.contains(requirement) {
                    missing.push((package.to_string(), requirement.to_string()));
                }
            }
        }
        missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lists(json: &str) -> PackageHashMap {
        serde_json::from_str(json).expect("test list must parse")
    }

    fn entry(removal: &str, deps: &[&str], needed_by: &[&str]) -> String {
        format!(
            r#"{{"list":"Oem","description":"","dependencies":{deps:?},"neededBy":{needed_by:?},"labels":[],"removal":"{removal}"}}"#
        )
    }

    /// Every listed package is installed and enabled, with the given ratings.
    fn all_enabled(json_lists: &PackageHashMap) -> impl Fn(&str) -> Option<DevicePackage> + '_ {
        |name| {
            json_lists.get(name).map(|p| DevicePackage {
                state: PackageState::Enabled,
                removal: p.removal,
            })
        }
    }

    fn names(related: &[RelatedPackage]) -> Vec<&str> {
        related.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn links_are_merged_from_both_fields() {
        let l = lists(&format!(
            r#"{{"app":{},"helper":{},"overlay":{}}}"#,
            entry("Recommended", &["helper"], &[]),
            entry("Recommended", &[], &[]),
            entry("Advanced", &[], &["app"]),
        ));
        let r = Relations::new(&l);
        assert_eq!(r.requirements("app"), vec!["helper", "overlay"]);
        assert_eq!(r.dependents("helper"), vec!["app"]);
        assert_eq!(r.dependents("overlay"), vec!["app"]);
    }

    #[test]
    fn orphaned_requirements_and_dependents_are_related() {
        let l = lists(&format!(
            r#"{{"app":{},"service":{},"overlay":{}}}"#,
            entry("Recommended", &["service"], &[]),
            entry("Recommended", &[], &[]),
            entry("Advanced", &["app"], &[]),
        ));
        let plan = Relations::new(&l).plan_removal(&["app".into()], all_enabled(&l), true);
        assert_eq!(names(&plan.related), vec!["overlay", "service"]);
        assert_eq!(plan.at_risk, vec![]);
    }

    #[test]
    fn shared_requirement_is_kept_while_another_app_needs_it() {
        let l = lists(&format!(
            r#"{{"health":{},"cloud":{},"hms":{}}}"#,
            entry("Recommended", &["hms"], &[]),
            entry("Recommended", &["hms"], &[]),
            entry("Advanced", &[], &[]),
        ));
        let r = Relations::new(&l);
        let plan = r.plan_removal(&["health".into()], all_enabled(&l), true);
        assert_eq!(plan.related, vec![]);

        // Once the other user of `hms` is uninstalled, `hms` becomes removable.
        let device = |name: &str| {
            l.get(name).map(|p| DevicePackage {
                state: if name == "cloud" {
                    PackageState::Uninstalled
                } else {
                    PackageState::Enabled
                },
                removal: p.removal,
            })
        };
        let plan_after = r.plan_removal(&["health".into()], device, true);
        assert_eq!(names(&plan_after.related), vec!["hms"]);
    }

    #[test]
    fn unsafe_packages_are_never_added() {
        let l = lists(&format!(
            r#"{{"lib":{},"core":{}}}"#,
            entry("Advanced", &[], &[]),
            entry("Unsafe", &["lib"], &[]),
        ));
        let plan = Relations::new(&l).plan_removal(&["lib".into()], all_enabled(&l), true);
        assert_eq!(plan.related, vec![]);
        assert_eq!(names(&plan.excluded_unsafe), vec!["core"]);
        assert_eq!(
            plan.at_risk,
            vec![AtRisk {
                dependent: "core".into(),
                requirement: "lib".into()
            }]
        );
    }

    /// `pay` needs `framework`, and both are `Recommended`.
    fn pay_needs_framework() -> PackageHashMap {
        lists(&format!(
            r#"{{"pay":{},"framework":{}}}"#,
            entry("Recommended", &["framework"], &[]),
            entry("Recommended", &[], &[]),
        ))
    }

    #[test]
    fn at_risk_follows_the_include_choice() {
        let l = pay_needs_framework();
        let r = Relations::new(&l);
        let without = r.plan_removal(&["framework".into()], all_enabled(&l), false);
        assert_eq!(names(&without.related), vec!["pay"]);
        assert_eq!(without.at_risk.len(), 1);
        let with = r.plan_removal(&["framework".into()], all_enabled(&l), true);
        assert_eq!(with.at_risk, vec![]);
    }

    #[test]
    fn chains_are_followed_and_disabled_packages_ignored() {
        let l = lists(&format!(
            r#"{{"a":{},"b":{},"c":{},"d":{}}}"#,
            entry("Recommended", &["b"], &[]),
            entry("Recommended", &["c"], &[]),
            entry("Recommended", &[], &[]),
            entry("Recommended", &["a"], &[]),
        ));
        let device = |name: &str| {
            l.get(name).map(|p| DevicePackage {
                state: if name == "d" {
                    PackageState::Disabled
                } else {
                    PackageState::Enabled
                },
                removal: p.removal,
            })
        };
        let plan = Relations::new(&l).plan_removal(&["a".into()], device, true);
        assert_eq!(names(&plan.related), vec!["b", "c"]);
    }

    #[test]
    fn restoring_reports_requirements_that_are_not_enabled() {
        let l = pay_needs_framework();
        let device = |name: &str| {
            l.get(name).map(|p| DevicePackage {
                state: PackageState::Uninstalled,
                removal: p.removal,
            })
        };
        let r = Relations::new(&l);
        assert_eq!(
            r.missing_requirements(&["pay".into()], device),
            vec![("pay".to_string(), "framework".to_string())]
        );
        assert_eq!(
            r.missing_requirements(&["pay".into(), "framework".into()], device),
            vec![]
        );
    }

    #[test]
    fn embedded_list_builds_relations() {
        let l: PackageHashMap =
            serde_json::from_str(crate::uad_lists::DATA).expect("embedded list must parse");
        let r = Relations::new(&l);
        assert!(
            r.requirements("com.samsung.android.spay")
                .contains(&"com.samsung.android.spayfw")
        );
    }
}

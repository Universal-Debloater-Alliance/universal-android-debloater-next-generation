use std::collections::HashMap;
use uad_core::relations::{DevicePackage, RemovalPlan};
use uad_core::sync::User;
use uad_core::uad_lists::{PackageState, Removal};

use super::List;

impl List {
    /// Selects every selected package on all given users as well (multi-user mode).
    pub fn mirror_selection(&mut self, users: &[User]) {
        let selected: Vec<usize> = self.selected_packages.iter().map(|s| s.1).collect();
        for user in users.iter().filter(|u| !u.protected) {
            for &index in &selected {
                if let Some(package) = self
                    .phone_packages
                    .get_mut(user.index)
                    .and_then(|packages| packages.get_mut(index))
                {
                    package.selected = true;
                    if !self.selected_packages.contains(&(user.index, index)) {
                        self.selected_packages.push((user.index, index));
                    }
                }
            }
        }
    }

    /// State and rating of every package of one user, by name.
    pub(super) fn device_lookup(&self, i_user: usize) -> HashMap<&str, DevicePackage> {
        self.phone_packages
            .get(i_user)
            .map(|packages| {
                packages
                    .iter()
                    .map(|p| {
                        (
                            p.name.as_str(),
                            DevicePackage {
                                state: p.state,
                                removal: p.removal,
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Relation analysis of `selection` (package indexes of one user):
    /// the removal plan for the packages being removed, and the
    /// `(package, requirement)` pairs that block the packages being restored.
    pub(super) fn removal_plan(
        &self,
        i_user: usize,
        selection: &[usize],
    ) -> (RemovalPlan, Vec<(String, String)>) {
        let (mut removing, mut restoring) = (vec![], vec![]);
        for package in selection
            .iter()
            .filter_map(|&i| self.phone_packages.get(i_user)?.get(i))
        {
            if package.state == PackageState::Enabled {
                removing.push(package.name.clone());
            } else {
                restoring.push(package.name.clone());
            }
        }
        let device = self.device_lookup(i_user);
        let lookup = |name: &str| device.get(name).copied();
        let plan = if removing.is_empty() {
            RemovalPlan::default()
        } else {
            self.relations
                .plan_removal(&removing, lookup, self.include_related)
        };
        (
            plan,
            self.relations.missing_requirements(&restoring, lookup),
        )
    }

    /// Indexes of the related packages to remove together with `selection`.
    pub(super) fn related_indices(&self, i_user: usize, selection: &[usize]) -> Vec<usize> {
        let (plan, _) = self.removal_plan(i_user, selection);
        plan.related
            .iter()
            .filter_map(|r| {
                self.phone_packages[i_user]
                    .iter()
                    .position(|p| p.name == r.name)
            })
            .collect()
    }

    /// A single-package action is confirmed first unless it is a plain removal of a
    /// `Recommended` package, or a plain restore, with no relations involved.
    pub(super) fn needs_confirmation(&self, i_user: usize, i_package: usize) -> bool {
        let Some(package) = self.phone_packages[i_user].get(i_package) else {
            return false;
        };
        let (plan, missing) = self.removal_plan(i_user, &[i_package]);
        (package.state == PackageState::Enabled && package.removal != Removal::Recommended)
            || !plan.related.is_empty()
            || !plan.excluded_unsafe.is_empty()
            || !plan.at_risk.is_empty()
            || !missing.is_empty()
    }
}

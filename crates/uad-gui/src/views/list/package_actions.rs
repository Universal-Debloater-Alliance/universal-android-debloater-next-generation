use uad_core::config::DeviceSettings;
use uad_core::sync::{AdbError, Phone, apply_pkg_state_commands};
use uad_core::uad_lists::{Opposite, PackageState, Removal, UadList};

use super::{List, Message, PackageInfo, VerifyAndFallbackResult};
use crate::views::settings::Settings;
use crate::widgets::package_row::PackageRow;
use iced::Task;

impl List {
    // Handle verification completion on the UI thread after async work
    pub(super) fn on_verify_and_fallback_finished(
        &mut self,
        result: VerifyAndFallbackResult,
    ) -> Task<Message> {
        let package = &mut self.phone_packages[result.i_user][result.index];
        if let Some(notification) = result.notification {
            self.fallback_notifications.push(notification);
        }
        if let Some(err) = result.error_modal {
            self.error_modal = Some(err);
        }
        package.state = result.new_state;
        package.selected = false;
        // Drop only this (user, package) pair. Other selections stay selected.
        self.selected_packages
            .retain(|&x| x != (result.i_user, result.index));
        Self::filter_package_lists(self);
        if result.i_user == self.selected_user.unwrap_or_default().index
            && result.index == self.current_package_index
        {
            self.refresh_details();
        }
        Task::none()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Complex verification and fallback logic"
    )]
    //this functions matches the adb command to two types, when it is a soft issue and when it is a hard issue
    // soft issue: utilizes core::sync::attempt_fallback in the case the state that wanted to be changed wasn't in fact changed
    // hard issue: does basically nothing in most cases FIXME, besides handling enabling packages.
    // also this really should be refactored, these are gigantic code blocks
    pub(super) fn on_verify_and_fallback(
        &mut self,
        res: Result<PackageInfo, AdbError>,
        settings: &Settings,
        selected_device: &Phone,
    ) -> Task<Message> {
        match res {
            Ok(p) => {
                // Snapshot minimal info to move into background task
                let i_user = p.i_user;
                let index = p.index;
                let pkg_name = self.phone_packages[i_user][index].name.clone();
                let current_state = self.phone_packages[i_user][index].state;
                let wanted_state = current_state.opposite(settings.device.disable_mode);
                let before_cross_user_states = p.before_cross_user_states.clone();
                let device = selected_device.clone();
                let user_id = device.user_list[i_user].id;

                // Offload verification + potential fallback to background
                Task::perform(
                    async move {
                        // Blocking ADB calls happen here (off UI thread)
                        let actual_state_opt = uad_core::sync::get_package_state(
                            device.adb_id.as_str(),
                            &pkg_name,
                            Some(user_id),
                        );

                        match actual_state_opt {
                            Some(actual_state) if actual_state == wanted_state => {
                                // Check cross-user behavior
                                let error_modal = uad_core::sync::detect_cross_user_behavior(
                                    &pkg_name,
                                    device.adb_id.as_str(),
                                    user_id,
                                    wanted_state,
                                    actual_state,
                                    &device,
                                    &before_cross_user_states,
                                )
                                .map(|notification| {
                                    format!(
                                        "Cross-User Behavior Detected:\n\n{notification}\n\n\
                                        This is unusual behavior that may be specific to your device manufacturer (OEM). \
                                        The package state has been successfully changed on the target user."
                                    )
                                });

                                VerifyAndFallbackResult {
                                    i_user,
                                    index,
                                    new_state: wanted_state,
                                    notification: None,
                                    error_modal,
                                }
                            }
                            actual_state_opt => {
                                // Package doesn't exist (None) or has wrong state - try fallback
                                let actual_state =
                                    actual_state_opt.unwrap_or(PackageState::Uninstalled);
                                // Create CorePackage for fallback
                                let core_package = uad_core::sync::CorePackage {
                                    name: pkg_name.clone(),
                                    description: String::new(),
                                    removal: Removal::All,
                                    state: current_state,
                                    list: UadList::Unlisted,
                                };
                                let fallback_result = uad_core::sync::attempt_fallback(
                                    &core_package,
                                    wanted_state,
                                    actual_state,
                                    device.user_list[i_user],
                                    &device,
                                );

                                let state_description = match actual_state_opt {
                                    Some(PackageState::Uninstalled) => "remains uninstalled",
                                    Some(PackageState::Disabled) => "was disabled",
                                    Some(PackageState::Enabled) => "was enabled",
                                    Some(PackageState::All) => {
                                        "unexpected state (error determining state)"
                                    }
                                    None => "does not exist on device",
                                };

                                let (notification, new_state) = match fallback_result {
                                    Ok(fallback_action) => (
                                        Some(format!(
                                            "Package '{pkg_name}' was {} but {} instead. Fallback: {fallback_action}",
                                            match wanted_state {
                                                PackageState::Uninstalled => "uninstalled",
                                                PackageState::Disabled => "disabled",
                                                PackageState::Enabled => "enabled",
                                                PackageState::All => "modified",
                                            },
                                            state_description
                                        )),
                                        actual_state,
                                    ),
                                    Err(err) => (
                                        Some(format!(
                                            "Package '{pkg_name}' verification failed: {err}"
                                        )),
                                        current_state, // no change if fallback failed
                                    ),
                                };

                                VerifyAndFallbackResult {
                                    i_user,
                                    index,
                                    new_state,
                                    notification,
                                    error_modal: None,
                                }
                            }
                        }
                    },
                    Message::VerifyAndFallbackFinished,
                )
            }

            //this is an example of how errors should be done, this is hard to implement however.
            // Err(AdbError::SecurityException(err)) => {

            // }
            //FIXME: this has to be changed as soon as possible, generic errors are bad practice
            // keep in mind most solutions to hard errors like this are going to be hacky
            // and weird.
            Err(AdbError::Generic(err)) => {
                // this is such a hacky solution
                if err.contains("Shell cannot change component state") {
                    // look for the "pm enable --user " to get the command
                    if let Some(start_index) = err.find("pm enable --user ") {
                        // slice the string starting from the end of the prefix
                        let remainder = &err[start_index + "pm enable --user ".len()..];
                        // split by whitespace to get the arguments: [ID, PACKAGE, ...]
                        let parts: Vec<&str> = remainder.split_whitespace().collect();

                        if parts.len() >= 2 {
                            let user_id_str = parts[0];
                            let pkg_name_str = parts[1];

                            let found_user = selected_device
                                .user_list
                                .iter()
                                .enumerate()
                                .find(|(_, u)| u.id.to_string() == user_id_str);

                            if let Some((i_user, user_ref)) = found_user {
                                // find the package index
                                let found_pkg = self.phone_packages[i_user]
                                    .iter()
                                    .enumerate()
                                    .find(|(_, p)| p.name == pkg_name_str);

                                if let Some((index, _)) = found_pkg {
                                    let user = *user_ref;
                                    let device = selected_device.clone();
                                    let pkg_name = pkg_name_str.to_string();

                                    // fallback task
                                    return Task::perform(
                                        async move {
                                            let temp_pkg = uad_core::sync::CorePackage {
                                                name: pkg_name.clone(),
                                                description: String::new(),
                                                removal: Removal::All,
                                                state: PackageState::Disabled,
                                                list: UadList::Unlisted,
                                            };

                                            // get uninstall command
                                            let uninstall_cmds =
                                                uad_core::sync::apply_pkg_state_commands(
                                                    &temp_pkg,
                                                    PackageState::Uninstalled,
                                                    user,
                                                    &device,
                                                );

                                            if let Some(cmd) = uninstall_cmds.first() {
                                                let _ = uad_core::sync::run_adb_shell_action(
                                                    &device.adb_id,
                                                    cmd,
                                                );
                                            }

                                            // reinstall/enable
                                            let temp_pkg_uninstalled =
                                                uad_core::sync::CorePackage {
                                                    name: pkg_name.clone(),
                                                    description: String::new(),
                                                    removal: Removal::All,
                                                    state: PackageState::Uninstalled,
                                                    list: UadList::Unlisted,
                                                };

                                            let enable_cmds =
                                                uad_core::sync::apply_pkg_state_commands(
                                                    &temp_pkg_uninstalled,
                                                    PackageState::Enabled,
                                                    user,
                                                    &device,
                                                );

                                            // lets enable the new package
                                            if let Some(cmd) = enable_cmds.first() {
                                                let _ = uad_core::sync::run_adb_shell_action(
                                                    &device.adb_id,
                                                    cmd,
                                                );
                                            }

                                            //verifies the final state for thread confirmation
                                            let actual_state = uad_core::sync::get_package_state(
                                                &device.adb_id,
                                                &pkg_name,
                                                Some(user.id),
                                            )
                                            .unwrap_or(PackageState::Uninstalled);
                                            VerifyAndFallbackResult {
                                                i_user,
                                                index,
                                                new_state: actual_state,
                                                notification: Some(format!(
                                                    "Attempted recovery for {pkg_name}"
                                                )),
                                                error_modal: None,
                                            }
                                        },
                                        Message::VerifyAndFallbackFinished,
                                    );
                                }
                            }
                        }
                    }
                }

                // Fallthrough: default error handling if parsing failed or unrelated error
                self.error_modal = Some(err);
                Task::none()
            }
        }
    }

    /// Toggles one package, plus `related` packages of the same user.
    pub(super) fn run_single_action(
        &mut self,
        settings: &Settings,
        selected_device: &Phone,
        (i_user, i_package): (usize, usize),
        related: &[usize],
    ) -> Task<Message> {
        self.fallback_notifications.clear();
        let mut tasks = vec![];
        for &index in std::iter::once(&i_package).chain(related) {
            self.phone_packages[i_user][index].selected = true;
            tasks.extend(build_action_pkg_commands(
                &self.phone_packages,
                selected_device,
                &settings.device,
                (i_user, index),
            ));
        }
        Task::batch(tasks)
    }

    pub(super) fn on_confirm_pending_action(
        &mut self,
        settings: &Settings,
        selected_device: &Phone,
    ) -> Task<Message> {
        let Some((i_user, i_package)) = self.pending_action.take() else {
            return Task::none();
        };
        let related = if self.include_related {
            self.related_indices(i_user, &[i_package])
        } else {
            vec![]
        };
        self.run_single_action(settings, selected_device, (i_user, i_package), &related)
    }
}

pub(super) fn build_action_pkg_commands(
    packages: &[Vec<PackageRow>],
    device: &Phone,
    settings: &DeviceSettings,
    selection: (usize, usize),
) -> Vec<Task<Message>> {
    let pkg = &packages[selection.0][selection.1];
    let wanted_state = pkg.state.opposite(settings.disable_mode);

    let mut commands = vec![];
    for u in device.user_list.iter().filter(|&&u| {
        !u.protected
            && packages
                .get(u.index)
                .and_then(|user_pkgs| user_pkgs.get(selection.1))
                .is_some_and(|row_pkg| {
                    // Only apply to users where package is explicitly selected
                    // OR if multi_user_mode is enabled AND this is the initiating user
                    row_pkg.selected || (settings.multi_user_mode && u.index == selection.0)
                })
    }) {
        let u_pkg = &packages[u.index][selection.1];
        let wanted_state = if settings.multi_user_mode {
            wanted_state
        } else {
            u_pkg.state.opposite(settings.disable_mode)
        };

        let actions = apply_pkg_state_commands(&u_pkg.into(), wanted_state, *u, device);

        for (j, action) in actions.into_iter().enumerate() {
            let p_info = PackageInfo {
                i_user: u.index,
                index: selection.1,
                removal: pkg.removal.to_string(),
                // Will be filled asynchronously before running the adb action
                before_cross_user_states: vec![],
            };
            // Clone data before async block to avoid borrowing issues
            let device_serial = device.adb_id.clone();
            let package_name = u_pkg.name.clone();
            let user_id = u.id;
            let phone = device.clone();
            // In the end there is only one package state change
            // even if we run multiple adb commands
            commands.push(Task::perform(
                async move {
                    run_adb_action_with_before_states(
                        &device_serial,
                        &action,
                        p_info,
                        &package_name,
                        user_id,
                        &phone,
                    )
                },
                if j == 0 {
                    Message::VerifyAndFallback
                } else {
                    |_| Message::Nothing
                },
            ));
        }
    }
    commands
}

pub(super) fn run_adb_action_with_before_states(
    device_serial: &str,
    action: &str,
    mut p_info: PackageInfo,
    package_name: &str,
    target_user_id: u16,
    phone: &Phone,
) -> Result<PackageInfo, AdbError> {
    // Capture before-state in background to avoid blocking UI thread
    let before_states = uad_core::sync::capture_cross_user_states(
        package_name,
        device_serial,
        target_user_id,
        phone,
    );
    p_info.before_cross_user_states = before_states;
    // Run the ADB action
    uad_core::sync::run_adb_shell_action(device_serial, action)?;
    Ok(p_info)
}

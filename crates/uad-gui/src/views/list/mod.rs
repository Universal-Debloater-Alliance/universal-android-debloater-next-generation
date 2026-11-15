use log::{error, info, warn};
use std::path::PathBuf;
use uad_core::export::export_selection;
use uad_core::opener::open_url;
use uad_core::package_text::matches_search;
use uad_core::relations::Relations;
use uad_core::sync::fetch_packages;
use uad_core::sync::{AdbError, Phone, User};
use uad_core::uad_lists::{
    PackageHashMap, PackageState, Removal, UadList, UadListState, load_debloat_lists,
};

use crate::views::settings::Settings;
use crate::widgets::package_row::{Message as RowMessage, PackageRow};
use iced::Task;
use iced::widget::text_editor;

#[derive(Debug, Default, Clone)]
pub struct PackageInfo {
    pub i_user: usize,
    pub index: usize,
    pub removal: String,
    pub before_cross_user_states: Vec<(u16, PackageState)>,
}

#[derive(Default, Debug, Clone)]
pub enum LoadingState {
    DownloadingList,
    #[default]
    FindingPhones,
    LoadingPackages,
    UpdatingUad,
    Ready,
    RestoringDevice(String),
    FailedToUpdate,
}

#[derive(Default, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "Not a state-machine")]
pub struct List {
    pub loading_state: LoadingState,
    pub uad_lists: PackageHashMap,
    /// `dependencies`/`neededBy` links of `uad_lists`, in both directions
    relations: Relations,
    /// packages of all users of the phone
    pub phone_packages: Vec<Vec<PackageRow>>,
    /// `phone_packages` indexes of the selected user (= what you see on screen)
    filtered_packages: Vec<usize>,
    /// Vec of `(user_index, pkg_index)`
    selected_packages: Vec<(usize, usize)>,
    selected_removal: Option<Removal>,
    selected_package_state: Option<PackageState>,
    selected_list: Option<UadList>,
    pub selected_user: Option<User>,
    all_selected: bool,
    pub input_value: String,
    description_content: text_editor::Content,
    selection_modal: bool,
    error_modal: Option<String>,
    /// Result of the last "export selection", shown until dismissed
    export_result: Option<Result<PathBuf, String>>,
    /// Single-package action waiting for confirmation: `(user_index, pkg_index)`
    pending_action: Option<(usize, usize)>,
    /// Whether removals also remove related packages (opt-in, see `Relations`)
    include_related: bool,
    current_package_index: usize,
    is_adb_satisfied: bool,
    copy_confirmation: bool,
    fallback_notifications: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadUadList(bool),
    LoadPhonePackages((PackageHashMap, UadListState)),
    RestoringDevice(Result<PackageInfo, AdbError>),
    ApplyFilters(Vec<Vec<PackageRow>>),
    DismissFallbackNotifications,
    VerifyAndFallbackFinished(VerifyAndFallbackResult),
    SearchInputChanged(String),
    ToggleAllSelected(bool),
    ListSelected(UadList),
    UserSelected(User),
    RemovalSelected(Removal),
    PackageStateSelected(PackageState),
    ApplyActionOnSelection,
    List(usize, RowMessage),
    VerifyAndFallback(Result<PackageInfo, AdbError>),
    Nothing,
    ModalHide,
    ModalUserSelected(User),
    ModalValidate,
    ClearSelectedPackages,
    ADBSatisfied(bool),
    UpdateFailed,
    GoToUrl(PathBuf),
    ExportSelection,
    SelectionExported(Result<PathBuf, String>),
    IncludeRelatedToggled(bool),
    ConfirmPendingAction,
    DescriptionEdit(text_editor::Action),
    CopyError(String),
    HideCopyConfirmation,
}

pub struct SummaryEntry {
    category: Removal,
    discard: u8,
    restore: u8,
}

#[derive(Debug, Clone)]
pub struct VerifyAndFallbackResult {
    pub i_user: usize,
    pub index: usize,
    pub new_state: PackageState,
    pub notification: Option<String>,
    pub error_modal: Option<String>,
}

impl From<Removal> for SummaryEntry {
    fn from(category: Removal) -> Self {
        Self {
            category,
            discard: 0,
            restore: 0,
        }
    }
}

mod package_actions;
use package_actions::build_action_pkg_commands;
mod dialogs;
mod related_packages;
mod view;

impl List {
    pub fn update(
        &mut self,
        settings: &mut Settings,
        selected_device: &mut Phone,
        list_update_state: &mut UadListState,
        message: Message,
    ) -> Task<Message> {
        match message {
            Message::ModalHide => self.on_modal_hide(),
            Message::ModalValidate => self.on_modal_validate(settings, selected_device),
            Message::RestoringDevice(output) => self.on_restoring_device(output),
            Message::LoadUadList(remote) => self.on_load_uad_list(remote, selected_device),
            Message::LoadPhonePackages(payload) => {
                self.on_load_phone_packages(payload, selected_device, list_update_state)
            }
            Message::ApplyFilters(packages) => self.on_apply_filters(packages),
            Message::DismissFallbackNotifications => self.on_dismiss_fallback_notifications(),
            Message::ToggleAllSelected(selected) => {
                self.on_toggle_all_selected(selected, settings, selected_device, list_update_state)
            }
            Message::SearchInputChanged(letter) => self.on_search_input_changed(letter),
            Message::ListSelected(list) => self.on_list_selected(list),
            Message::RemovalSelected(removal) => self.on_removal_selected(removal),
            Message::PackageStateSelected(state) => self.on_package_state_selected(state),
            Message::List(i, row_msg) => self.on_list_row(i, &row_msg, settings, selected_device),
            Message::ApplyActionOnSelection => self.on_apply_action_on_selection(),
            Message::UserSelected(user) => self.on_user_selected(user),
            Message::VerifyAndFallback(res) => {
                self.on_verify_and_fallback(res, settings, selected_device)
            }
            Message::VerifyAndFallbackFinished(result) => {
                self.on_verify_and_fallback_finished(result)
            }
            Message::ModalUserSelected(user) => {
                self.on_modal_user_selected(user, settings, selected_device, list_update_state)
            }
            Message::ClearSelectedPackages => self.on_clear_selected_packages(),
            Message::ADBSatisfied(result) => self.on_adb_satisfied(result),
            Message::UpdateFailed => self.on_update_failed(),
            Message::GoToUrl(url) => Self::on_go_to_url(url),
            Message::ExportSelection => self.on_export_selection(&settings.general.backup_folder),
            Message::SelectionExported(res) => self.on_selection_exported(res),
            Message::IncludeRelatedToggled(include) => {
                self.include_related = include;
                Task::none()
            }
            Message::ConfirmPendingAction => {
                self.on_confirm_pending_action(settings, selected_device)
            }
            Message::Nothing => Task::none(),
            Message::DescriptionEdit(action) => self.on_description_edit(action),
            Message::CopyError(err) => self.on_copy_error(err),
            Message::HideCopyConfirmation => self.on_hide_copy_confirmation(),
        }
    }

    fn filter_package_lists(&mut self) {
        let list_filter: UadList = self.selected_list.expect("UAD-list type must be selected");
        let package_filter: PackageState = self
            .selected_package_state
            .expect("pack-state must be selected");
        let removal_filter: Removal = self
            .selected_removal
            .expect("removal recommendation must be selected");

        self.filtered_packages = self.phone_packages
            [self.selected_user.expect("User must be selected").index]
            .iter()
            // we must filter the indices associated with pack-rows,
            // that's why `enumerate` is before `filter`.
            .enumerate()
            .filter(|(_, p)| {
                (list_filter == UadList::All || p.list == list_filter)
                    && (package_filter == PackageState::All || p.state == package_filter)
                    && (removal_filter == Removal::All || p.removal == removal_filter)
                    && (self.input_value.is_empty()
                        || matches_search(&p.name, &self.input_value, Some(&p.description)))
            })
            .map(|(i, _)| i)
            .collect();
    }

    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "Async so the blocking ADB calls run in the Iced executor, not in `update`"
    )]
    async fn load_packages<S: AsRef<str>>(
        uad_list: PackageHashMap,
        device_serial: S,
        user_list: Vec<User>,
    ) -> Vec<Vec<PackageRow>> {
        let serial = device_serial.as_ref();
        if user_list.len() <= 1 {
            vec![
                fetch_packages(&uad_list, serial, None)
                    .into_iter()
                    .map(PackageRow::from)
                    .collect(),
            ]
        } else {
            user_list
                .iter()
                .map(|user| {
                    fetch_packages(&uad_list, serial, Some(user.id))
                        .into_iter()
                        .map(PackageRow::from)
                        .collect()
                })
                .collect()
        }
    }

    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "Async so the blocking ADB calls run in the Iced executor, not in `update`"
    )]
    async fn init_apps_view(remote: bool, phone: Phone) -> (PackageHashMap, UadListState) {
        let uad_lists = load_debloat_lists(remote);
        match uad_lists {
            Ok(list) => {
                if phone.adb_id.is_empty() {
                    warn!("AppsView ready but no phone found");
                }
                (list, UadListState::Done)
            }
            Err(local_list) => {
                error!(
                    "Error loading remote debloat list for the phone. Fallback to embedded (and outdated) list"
                );
                (local_list, UadListState::Failed)
            }
        }
    }

    // === Split handlers to keep update short ===
    fn on_modal_hide(&mut self) -> Task<Message> {
        self.selection_modal = false;
        self.error_modal = None;
        self.export_result = None;
        self.pending_action = None;
        Task::none()
    }

    fn on_modal_validate(
        &mut self,
        settings: &Settings,
        selected_device: &mut Phone,
    ) -> Task<Message> {
        self.fallback_notifications.clear();
        if self.include_related {
            let users: Vec<usize> = self.selected_packages.iter().map(|s| s.0).collect();
            for i_user in users {
                let selection: Vec<usize> = self
                    .selected_packages
                    .iter()
                    .filter(|s| s.0 == i_user)
                    .map(|s| s.1)
                    .collect();
                for index in self.related_indices(i_user, &selection) {
                    self.phone_packages[i_user][index].selected = true;
                    self.selected_packages.push((i_user, index));
                }
            }
        }
        let mut commands = vec![];
        self.selected_packages.sort_unstable();
        self.selected_packages.dedup();
        for selection in &self.selected_packages {
            commands.append(&mut build_action_pkg_commands(
                &self.phone_packages,
                selected_device,
                &settings.device,
                *selection,
            ));
        }
        self.selection_modal = false;
        Task::batch(commands)
    }

    fn on_restoring_device(&mut self, output: Result<PackageInfo, AdbError>) -> Task<Message> {
        match output {
            Ok(p) => {
                if let Some(package) = self
                    .phone_packages
                    .get(p.i_user)
                    .and_then(|packages| packages.get(p.index))
                {
                    self.loading_state = LoadingState::RestoringDevice(package.name.clone());
                }
            }
            // Shown once the package list reloads after the restore.
            Err(AdbError::Generic(err)) => self.error_modal = Some(err),
        }
        Task::none()
    }

    fn on_load_uad_list(&mut self, remote: bool, selected_device: &Phone) -> Task<Message> {
        info!("{:-^65}", "-");
        info!(
            "ANDROID_SDK: {} | DEVICE: {}",
            selected_device.android_sdk, selected_device.model
        );
        info!("{:-^65}", "-");
        self.loading_state = LoadingState::DownloadingList;
        Task::perform(
            Self::init_apps_view(remote, selected_device.clone()),
            Message::LoadPhonePackages,
        )
    }

    fn on_load_phone_packages(
        &mut self,
        payload: (PackageHashMap, UadListState),
        selected_device: &Phone,
        list_update_state: &mut UadListState,
    ) -> Task<Message> {
        let (uad_list, list_state) = payload;
        self.loading_state = LoadingState::LoadingPackages;
        self.relations = Relations::new(&uad_list);
        self.uad_lists.clone_from(&uad_list);
        *list_update_state = list_state;
        Task::perform(
            Self::load_packages(
                uad_list,
                selected_device.adb_id.clone(),
                selected_device.user_list.clone(),
            ),
            Message::ApplyFilters,
        )
    }

    fn on_apply_filters(&mut self, packages: Vec<Vec<PackageRow>>) -> Task<Message> {
        let i_user = self.selected_user.unwrap_or_default().index;
        self.phone_packages = packages;
        self.filtered_packages = (0..self.phone_packages[i_user].len()).collect();
        self.selected_removal = Some(Removal::Recommended);
        self.selected_package_state = Some(PackageState::Enabled);
        self.selected_list = Some(UadList::All);
        self.selected_user = Some(User::default());
        self.fallback_notifications.clear();
        Self::filter_package_lists(self);
        self.loading_state = LoadingState::Ready;
        Task::none()
    }

    fn on_toggle_all_selected(
        &mut self,
        selected: bool,
        settings: &mut Settings,
        selected_device: &mut Phone,
        list_update_state: &mut UadListState,
    ) -> Task<Message> {
        let i_user = self.selected_user.unwrap_or_default().index;
        for i in self.filtered_packages.clone() {
            if self.phone_packages[i_user][i].selected != selected {
                #[expect(unused_must_use, reason = "side-effect")]
                self.update(
                    settings,
                    selected_device,
                    list_update_state,
                    Message::List(i, RowMessage::ToggleSelection(selected)),
                );
            }
        }
        self.all_selected = selected;
        Task::none()
    }

    fn on_search_input_changed(&mut self, letter: String) -> Task<Message> {
        self.input_value = letter;
        Self::filter_package_lists(self);
        Task::none()
    }

    fn on_list_selected(&mut self, list: UadList) -> Task<Message> {
        self.selected_list = Some(list);
        Self::filter_package_lists(self);
        Task::none()
    }

    fn on_package_state_selected(&mut self, package_state: PackageState) -> Task<Message> {
        self.selected_package_state = Some(package_state);
        Self::filter_package_lists(self);
        Task::none()
    }

    fn on_removal_selected(&mut self, removal: Removal) -> Task<Message> {
        self.selected_removal = Some(removal);
        Self::filter_package_lists(self);
        Task::none()
    }

    fn on_list_row(
        &mut self,
        i_package: usize,
        row_message: &RowMessage,
        settings: &Settings,
        selected_device: &mut Phone,
    ) -> Task<Message> {
        let i_user = self.selected_user.unwrap_or_default().index;
        #[expect(unused_must_use, reason = "side-effect")]
        {
            self.phone_packages[i_user][i_package]
                .update(row_message)
                .map(move |row_message| Message::List(i_package, row_message));
        }

        let package = &mut self.phone_packages[i_user][i_package];

        match *row_message {
            RowMessage::ToggleSelection(toggle) => {
                if package.removal == Removal::Unsafe && !settings.general.expert_mode {
                    package.selected = false;
                    return Task::none();
                }

                if settings.device.multi_user_mode {
                    for u in selected_device.user_list.iter().filter(|&u| !u.protected) {
                        if let Some(pkg) = self
                            .phone_packages
                            .get_mut(u.index)
                            .and_then(|pkgs| pkgs.get_mut(i_package))
                        {
                            pkg.selected = toggle;
                            if toggle && !self.selected_packages.contains(&(u.index, i_package)) {
                                self.selected_packages.push((u.index, i_package));
                            }
                        }
                    }
                    if !toggle {
                        self.selected_packages.retain(|&x| x.1 != i_package);
                    }
                } else {
                    package.selected = toggle;
                    if toggle {
                        if !self.selected_packages.contains(&(i_user, i_package)) {
                            self.selected_packages.push((i_user, i_package));
                        }
                    } else {
                        self.selected_packages
                            .retain(|&x| x.1 != i_package || x.0 != i_user);
                    }
                }
                Task::none()
            }
            RowMessage::ActionPressed => {
                if self.needs_confirmation(i_user, i_package) {
                    self.pending_action = Some((i_user, i_package));
                    return Task::none();
                }
                self.run_single_action(settings, selected_device, (i_user, i_package), &[])
            }
            RowMessage::PackagePressed => {
                package.current = true;
                if self.current_package_index != i_package
                    && let Some(previous) =
                        self.phone_packages[i_user].get_mut(self.current_package_index)
                {
                    previous.current = false;
                }
                self.current_package_index = i_package;
                self.refresh_details();
                Task::none()
            }
        }
    }

    fn on_apply_action_on_selection(&mut self) -> Task<Message> {
        self.selection_modal = true;
        Task::none()
    }

    fn on_user_selected(&mut self, user: User) -> Task<Message> {
        self.selected_user = Some(user);
        self.fallback_notifications.clear();
        self.filtered_packages = (0..self.phone_packages[user.index].len()).collect();
        Self::filter_package_lists(self);
        Task::none()
    }

    fn on_modal_user_selected(
        &mut self,
        user: User,
        settings: &mut Settings,
        selected_device: &mut Phone,
        list_update_state: &mut UadListState,
    ) -> Task<Message> {
        self.selected_user = Some(user);
        self.update(
            settings,
            selected_device,
            list_update_state,
            Message::UserSelected(user),
        )
    }

    fn on_clear_selected_packages(&mut self) -> Task<Message> {
        self.selected_packages = Vec::new();
        Task::none()
    }

    fn on_adb_satisfied(&mut self, result: bool) -> Task<Message> {
        self.is_adb_satisfied = result;
        Task::none()
    }

    fn on_update_failed(&mut self) -> Task<Message> {
        self.loading_state = LoadingState::FailedToUpdate;
        Task::none()
    }

    fn on_go_to_url(url: PathBuf) -> Task<Message> {
        open_url(url);
        Task::none()
    }

    fn on_export_selection(&mut self, backup_folder: &std::path::Path) -> Task<Message> {
        let i_user = self.selected_user.unwrap_or_default().index;
        let package_names: Vec<String> = self.phone_packages[i_user]
            .iter()
            .filter(|p| p.selected)
            .map(|p| p.name.clone())
            .collect();
        let dir = backup_folder.to_path_buf();
        Task::perform(
            async move { export_selection(&dir, &package_names) },
            Message::SelectionExported,
        )
    }

    fn on_selection_exported(&mut self, export: Result<PathBuf, String>) -> Task<Message> {
        if let Err(err) = &export {
            error!("Failed to export current selection: {err}");
        }
        self.export_result = Some(export);
        Task::none()
    }

    fn on_description_edit(&mut self, action: text_editor::Action) -> Task<Message> {
        match action {
            text_editor::Action::Scroll { lines: _ } | text_editor::Action::Edit(_) => {}
            _ => {
                self.description_content.perform(action);
            }
        }
        Task::none()
    }

    fn on_copy_error(&mut self, err: String) -> Task<Message> {
        self.copy_confirmation = true;
        Task::batch(vec![
            iced::clipboard::write::<Message>(err),
            Task::perform(
                // intentional delay
                async { std::thread::sleep(std::time::Duration::from_secs(1)) },
                |()| Message::HideCopyConfirmation,
            ),
        ])
    }

    fn on_hide_copy_confirmation(&mut self) -> Task<Message> {
        self.copy_confirmation = false;
        Task::none()
    }

    fn on_dismiss_fallback_notifications(&mut self) -> Task<Message> {
        self.fallback_notifications.clear();
        Task::none()
    }

    /// Rebuilds the details text of the current package: what its rating
    /// means, its description, and the state of related packages.
    fn refresh_details(&mut self) {
        let i_user = self.selected_user.unwrap_or_default().index;
        let Some(package) = self
            .phone_packages
            .get(i_user)
            .and_then(|packages| packages.get(self.current_package_index))
        else {
            return;
        };
        let device = self.device_lookup(i_user);
        let describe = |names: Vec<&str>| {
            names
                .iter()
                .map(|name| match device.get(name) {
                    Some(p) => format!("{name} ({})", p.state),
                    None => format!("{name} (not installed for this user)"),
                })
                .collect::<Vec<_>>()
                .join(", ")
        };

        let mut details = vec![
            package.removal.explanation().to_string(),
            package.description.clone(),
        ];
        let needs = self.relations.requirements(&package.name);
        if !needs.is_empty() {
            details.push(format!("Needs: {}", describe(needs)));
        }
        let needed_by = self.relations.dependents(&package.name);
        if !needed_by.is_empty() {
            details.push(format!("Needed by: {}", describe(needed_by)));
        }
        self.description_content = text_editor::Content::with_text(&details.join("\n\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, removal: Removal, state: PackageState) -> PackageRow {
        PackageRow::new(name, "", removal, state, UadList::Oem, false, false)
    }

    /// `pay` needs `framework`. `framework` is needed by nothing else.
    fn list_with(rows: Vec<PackageRow>) -> List {
        let lists: PackageHashMap = serde_json::from_str(
            r#"{
                "pay": {"list":"Oem","description":"","dependencies":["framework"],"neededBy":[],"labels":[],"removal":"Recommended"},
                "framework": {"list":"Oem","description":"","dependencies":[],"neededBy":["pay"],"labels":[],"removal":"Recommended"}
            }"#,
        )
        .expect("test list must parse");
        List {
            relations: Relations::new(&lists),
            phone_packages: vec![rows],
            selected_user: Some(User::default()),
            selected_list: Some(UadList::All),
            selected_package_state: Some(PackageState::All),
            selected_removal: Some(Removal::All),
            ..List::default()
        }
    }

    #[test]
    fn removing_a_primary_package_offers_its_requirement() {
        let list = list_with(vec![
            row("framework", Removal::Recommended, PackageState::Enabled),
            row("pay", Removal::Recommended, PackageState::Enabled),
            row("unrelated", Removal::Recommended, PackageState::Enabled),
        ]);
        assert_eq!(list.related_indices(0, &[1]), vec![0]);
        assert_eq!(list.related_indices(0, &[2]), Vec::<usize>::new());
    }

    #[test]
    fn only_risky_or_related_actions_need_confirmation() {
        let list = list_with(vec![
            row("framework", Removal::Recommended, PackageState::Enabled),
            row("pay", Removal::Recommended, PackageState::Enabled),
            row("plain", Removal::Recommended, PackageState::Enabled),
            row("expert", Removal::Expert, PackageState::Enabled),
            row("restorable", Removal::Expert, PackageState::Uninstalled),
        ]);
        assert!(list.needs_confirmation(0, 1), "has a related package");
        assert!(list.needs_confirmation(0, 3), "Expert removal");
        assert!(!list.needs_confirmation(0, 2), "plain Recommended removal");
        assert!(!list.needs_confirmation(0, 4), "plain restore");
    }

    #[test]
    fn restoring_warns_about_a_missing_requirement() {
        let list = list_with(vec![
            row("framework", Removal::Recommended, PackageState::Uninstalled),
            row("pay", Removal::Recommended, PackageState::Uninstalled),
        ]);
        let (_, missing) = list.removal_plan(0, &[1]);
        assert_eq!(missing, vec![("pay".to_string(), "framework".to_string())]);
        assert!(list.needs_confirmation(0, 1));
    }

    #[test]
    fn finishing_one_action_keeps_the_other_selections() {
        let mut list = list_with(vec![
            row("a", Removal::Recommended, PackageState::Enabled),
            row("b", Removal::Recommended, PackageState::Enabled),
        ]);
        list.selected_packages = vec![(0, 0), (0, 1)];
        let _ = list.on_verify_and_fallback_finished(VerifyAndFallbackResult {
            i_user: 0,
            index: 0,
            new_state: PackageState::Uninstalled,
            notification: None,
            error_modal: None,
        });
        assert_eq!(list.selected_packages, vec![(0, 1)]);
    }

    #[test]
    fn mirror_selection_uses_package_indexes() {
        let rows = vec![
            row("a", Removal::Recommended, PackageState::Enabled),
            row("b", Removal::Recommended, PackageState::Enabled),
        ];
        let mut list = list_with(rows.clone());
        list.phone_packages.push(rows);
        list.phone_packages[0][1].selected = true;
        list.selected_packages = vec![(0, 1)];
        let users = [
            User::default(),
            User {
                id: 10,
                index: 1,
                protected: false,
            },
        ];
        list.mirror_selection(&users);
        assert!(list.phone_packages[1][1].selected);
        assert!(!list.phone_packages[1][0].selected);
        assert!(list.selected_packages.contains(&(1, 1)));
    }
}

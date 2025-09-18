use crate::core::config::DeviceSettings;
use crate::core::helpers::button_primary;
use crate::core::sync::{AdbError, Phone, User, adb_shell_command, apply_pkg_state_commands};
use crate::core::uad_lists::{
    Opposite, PackageHashMap, PackageState, Removal, UadList, UadListState, load_debloat_lists,
};
use crate::core::utils::{EXPORT_FILE_NAME, NAME, export_selection, fetch_packages, open_url};
use crate::gui::style;
use crate::gui::widgets::navigation_menu::ICONS;
use std::path::PathBuf;

use crate::gui::views::settings::Settings;
use crate::gui::widgets::modal::Modal;
use crate::gui::widgets::package_row::{Message as RowMessage, PackageRow};
use crate::gui::widgets::text;
use iced::widget::{
    Column, Space, button, checkbox, column, container, horizontal_space, pick_list, radio, row,
    scrollable, text_editor, text_input, tooltip, vertical_rule,
};
use iced::{Alignment, Color, Element, Length, Padding, Task, alignment};

#[derive(Debug, Default, Clone)]
pub struct PackageInfo {
    pub i_user: usize,
    pub index: usize,
    pub removal: String,
}

#[derive(Default, Debug, Clone)]
pub enum LoadingState {
    DownloadingList,
    #[default]
    FindingPhones,
    LoadingPackages,
    _UpdatingUad,
    Ready,
    RestoringDevice(String),
    FailedToUpdate,
}

#[derive(Default, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "Not a state-machine")]
pub struct List {
    pub loading_state: LoadingState,
    pub uad_lists: PackageHashMap,
    pub phone_packages: Vec<Vec<PackageRow>>,
    filtered_packages: Vec<usize>,
    selected_packages: Vec<(usize, usize)>,
    selected_package_state: Option<PackageState>,
    selected_removal: Option<Removal>,
    selected_list: Option<UadList>,
    pub selected_user: Option<User>,
    all_selected: bool,
    pub input_value: String,
    description: String,
    description_content: text_editor::Content,
    selection_modal: bool,
    error_modal: Option<String>,
    export_modal: bool,
    current_package_index: usize,
    is_adb_satisfied: bool,
    copy_confirmation: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadUadList(bool),
    LoadPhonePackages((PackageHashMap, UadListState)),
    RestoringDevice(Result<PackageInfo, AdbError>),
    ApplyFilters(Vec<Vec<PackageRow>>),
    SearchInputChanged(String),
    ToggleAllSelected(bool),
    ListSelected(UadList),
    UserSelected(User),
    PackageStateSelected(PackageState),
    RemovalSelected(Removal),
    ApplyActionOnSelection,
    List(usize, RowMessage),
    ChangePackageState(Result<PackageInfo, AdbError>),
    Nothing,
    ModalHide,
    ModalUserSelected(User),
    ModalValidate,
    ClearSelectedPackages,
    ADBSatisfied(bool),
    UpdateFailed,
    GoToUrl(PathBuf),
    ExportSelection,
    SelectionExported(Result<bool, String>),
    DescriptionEdit(text_editor::Action),
    CopyError(String),
    HideCopyConfirmation,
}

pub struct SummaryEntry {
    category: Removal,
    discard: u8,
    restore: u8,
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

impl List {
    #[inline]
    fn toggle_selection_at(
        &mut self,
        settings: &Settings,
        selected_device: &Phone,
        i_user: usize,
        i_package: usize,
        toggle: bool,
    ) {
        // Disallow selecting unsafe packages unless in expert mode
        if self.phone_packages[i_user][i_package].removal == Removal::Unsafe
            && !settings.general.expert_mode
        {
            self.phone_packages[i_user][i_package].selected = false;
            return;
        }

        if settings.device.multi_user_mode {
            // Apply to all non-protected users
            for u in selected_device.user_list.iter().filter(|u| !u.protected) {
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
                // Remove any entry of this package for any user
                self.selected_packages.retain(|&x| x.1 != i_package);
            }
        } else {
            let pkg = &mut self.phone_packages[i_user][i_package];
            pkg.selected = toggle;
            if toggle {
                if !self.selected_packages.contains(&(i_user, i_package)) {
                    self.selected_packages.push((i_user, i_package));
                }
            } else {
                self.selected_packages
                    .retain(|&x| x.1 != i_package || x.0 != i_user);
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn update(
        &mut self,
        settings: &mut Settings,
        selected_device: &mut Phone,
        list_update_state: &mut UadListState,
        message: Message,
    ) -> Task<Message> {
        let i_user = self.selected_user.unwrap_or_default().index;
        match message {
            Message::ModalHide => {
                self.selection_modal = false;
                self.error_modal = None;
                self.export_modal = false;
                Task::none()
            }
            Message::ModalValidate => {
                let mut commands = Vec::new();
                self.selected_packages.sort_unstable();
                self.selected_packages.dedup();
                for &selection in &self.selected_packages {
                    commands.extend(build_action_pkg_commands(
                        &self.phone_packages,
                        selected_device,
                        &settings.device,
                        selection,
                    ));
                }
                self.selection_modal = false;
                Task::batch(commands)
            }
            Message::RestoringDevice(output) => {
                if let Ok(p) = output {
                    self.loading_state = LoadingState::RestoringDevice(
                        self.phone_packages[i_user][p.index].name.clone(),
                    );
                } else {
                    self.loading_state = LoadingState::RestoringDevice("Error [TODO]".to_string());
                }
                Task::none()
            }
            Message::LoadUadList(remote) => {
                info!("{:-^65}", "-");
                info!(
                    "ANDROID_SDK: {} | DEVICE: {}",
                    selected_device.android_sdk, selected_device.model
                );
                info!("{:-^65}", "-");
                self.loading_state = LoadingState::DownloadingList;
                Task::perform(
                    Self::init_apps_view(remote, !selected_device.adb_id.is_empty()),
                    Message::LoadPhonePackages,
                )
            }
            Message::LoadPhonePackages((uad_list, list_state)) => {
                self.loading_state = LoadingState::LoadingPackages;
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
            Message::ApplyFilters(packages) => {
                self.phone_packages = packages;
                self.filtered_packages = (0..self.phone_packages[i_user].len()).collect();
                self.selected_package_state = Some(PackageState::Enabled);
                self.selected_removal = Some(Removal::Recommended);
                self.selected_list = Some(UadList::All);
                self.selected_user = Some(User::default());
                Self::filter_package_lists(self);
                self.loading_state = LoadingState::Ready;
                Task::none()
            }
            Message::ToggleAllSelected(selected) => {
                let count = self.filtered_packages.len();
                for idx in 0..count {
                    let i = self.filtered_packages[idx];
                    if self.phone_packages[i_user][i].selected != selected {
                        self.toggle_selection_at(settings, selected_device, i_user, i, selected);
                    }
                }
                self.all_selected = selected;
                Task::none()
            }
            Message::SearchInputChanged(letter) => {
                self.input_value = letter;
                Self::filter_package_lists(self);
                Task::none()
            }
            Message::ListSelected(list) => {
                self.selected_list = Some(list);
                Self::filter_package_lists(self);
                Task::none()
            }
            Message::PackageStateSelected(package_state) => {
                self.selected_package_state = Some(package_state);
                Self::filter_package_lists(self);
                Task::none()
            }
            Message::RemovalSelected(removal) => {
                self.selected_removal = Some(removal);
                Self::filter_package_lists(self);
                Task::none()
            }
            Message::List(i_package, row_message) => {
                let _ = self.phone_packages[i_user][i_package]
                    .update(&row_message)
                    .map(move |msg| Message::List(i_package, msg));

                let package = &mut self.phone_packages[i_user][i_package];

                match row_message {
                    RowMessage::ToggleSelection(toggle) => {
                        self.toggle_selection_at(
                            settings,
                            selected_device,
                            i_user,
                            i_package,
                            toggle,
                        );
                        Task::none()
                    }
                    RowMessage::ActionPressed => {
                        self.phone_packages[i_user][i_package].selected = true;
                        Task::batch(build_action_pkg_commands(
                            &self.phone_packages,
                            selected_device,
                            &settings.device,
                            (i_user, i_package),
                        ))
                    }
                    RowMessage::PackagePressed => {
                        self.description = package.description.clone();
                        self.description_content =
                            text_editor::Content::with_text(&package.description);
                        package.current = true;
                        if self.current_package_index != i_package {
                            self.phone_packages[i_user][self.current_package_index].current = false;
                        }
                        self.current_package_index = i_package;
                        Task::none()
                    }
                }
            }
            Message::ApplyActionOnSelection => {
                self.selection_modal = true;
                Task::none()
            }
            Message::UserSelected(user) => {
                self.selected_user = Some(user);
                self.filtered_packages = (0..self.phone_packages[user.index].len()).collect();
                Self::filter_package_lists(self);
                Task::none()
            }
            Message::ChangePackageState(res) => {
                match res {
                    Ok(p) => {
                        let package = &mut self.phone_packages[p.i_user][p.index];
                        package.state = package.state.opposite(settings.device.disable_mode);
                        package.selected = false;
                        self.selected_packages
                            .retain(|&x| x.1 != p.index || x.0 != p.i_user);
                        Self::filter_package_lists(self);
                    }
                    Err(AdbError::Generic(err)) => {
                        self.error_modal = Some(err);
                    }
                }
                Task::none()
            }
            Message::ModalUserSelected(user) => {
                self.selected_user = Some(user);
                self.update(
                    settings,
                    selected_device,
                    list_update_state,
                    Message::UserSelected(user),
                )
            }
            Message::ClearSelectedPackages => {
                self.selected_packages = Vec::new();
                Task::none()
            }
            Message::ADBSatisfied(result) => {
                self.is_adb_satisfied = result;
                Task::none()
            }
            Message::UpdateFailed => {
                self.loading_state = LoadingState::FailedToUpdate;
                Task::none()
            }
            Message::GoToUrl(url) => {
                open_url(url);
                Task::none()
            }
            Message::ExportSelection => Task::perform(
                export_selection(self.phone_packages[i_user].clone()),
                Message::SelectionExported,
            ),
            Message::SelectionExported(export) => {
                match export {
                    Ok(_) => self.export_modal = true,
                    Err(err) => error!("Failed to export current selection: {err:?}"),
                }
                Task::none()
            }
            Message::Nothing => Task::none(),
            Message::DescriptionEdit(action) => {
                self.description_content.perform(action);
                Task::none()
            }
            Message::CopyError(err) => {
                self.copy_confirmation = true;
                Task::batch([
                    iced::clipboard::write(err),
                    Task::perform(
                        async {
                            // Run on a background worker for Task::perform; UI thread is not blocked
                            std::thread::sleep(std::time::Duration::from_secs(1));
                        },
                        |_| Message::HideCopyConfirmation,
                    ),
                ])
            }
            Message::HideCopyConfirmation => {
                self.copy_confirmation = false;
                Task::none()
            }
        }
    }

    pub fn view(&self, settings: &Settings, selected_device: &Phone) -> Element<Message> {
        let palette = crate::core::theme::string_to_theme(&settings.general.theme).palette();
        let danger = palette.bright_error;

        match &self.loading_state {
            LoadingState::DownloadingList => waiting_view(
                &format!("Downloading latest {NAME} lists from GitHub. Please wait..."),
                Some(button("No internet?").on_press(Message::LoadUadList(false))),
                None,
            ),
            LoadingState::FindingPhones => {
                if self.is_adb_satisfied {
                    waiting_view("Finding connected devices...", None, None)
                } else {
                    waiting_view(
                        "ADB is not installed on your system, install ADB and relaunch application.",
                        Some(button("Read on how to get started.")
                            .on_press(Message::GoToUrl(PathBuf::from(
                                "https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Getting-started",
                            )))),
                        Some(danger),
                    )
                }
            }
            LoadingState::LoadingPackages => waiting_view(
                "Pulling packages from the device. Please wait...",
                None,
                None,
            ),
            LoadingState::_UpdatingUad => {
                waiting_view(&format!("Updating {NAME}. Please wait..."), None, None)
            }
            LoadingState::RestoringDevice(device) => {
                waiting_view(&format!("Restoring device: {device}"), None, None)
            }
            LoadingState::Ready => self.ready_view(settings, selected_device),
            LoadingState::FailedToUpdate => waiting_view(
                "Failed to download update",
                Some(button("Go back").on_press(Message::LoadUadList(false))),
                Some(danger),
            ),
        }
    }

    fn control_panel(&self, selected_device: &Phone) -> Element<Message> {
        let search_packages = text_input("Search packages...", &self.input_value)
            .width(Length::Fill)
            .on_input(Message::SearchInputChanged)
            .padding(10);

        let select_all_checkbox = checkbox("", self.all_selected)
            .on_toggle(Message::ToggleAllSelected)
            .spacing(0);

        let col_sel_all = row![
            tooltip(
                select_all_checkbox,
                if self.all_selected {
                    "Deselect all"
                } else {
                    "Select all"
                },
                tooltip::Position::Top,
            )
            .style(style::tooltip_container())
            .gap(4)
        ]
        .padding(8);

        let user_picklist = pick_list(
            selected_device.user_list.clone(),
            self.selected_user,
            Message::UserSelected,
        )
        .width(85);

        let list_picklist = pick_list(UadList::ALL, self.selected_list, Message::ListSelected);
        let package_state_picklist = pick_list(
            PackageState::ALL,
            self.selected_package_state,
            Message::PackageStateSelected,
        );

        let removal_picklist = pick_list(
            Removal::ALL,
            self.selected_removal,
            Message::RemovalSelected,
        );

        row![
            col_sel_all,
            search_packages,
            user_picklist,
            removal_picklist,
            package_state_picklist,
            list_picklist,
        ]
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .spacing(6)
        .padding(Padding {
            right: 16.0,
            ..Default::default()
        })
        .into()
    }

    #[allow(clippy::too_many_lines)]
    fn ready_view(&self, settings: &Settings, selected_device: &Phone) -> Element<Message> {
        let palette = crate::core::theme::string_to_theme(&settings.general.theme).palette();
        let commentary = palette.surface;
        let danger = palette.bright_error;

        let packages = self
            .filtered_packages
            .iter()
            .fold(column![].spacing(6), |col, &i| {
                col.push(
                    self.phone_packages[self.selected_user.unwrap_or_default().index][i]
                        .view(settings, selected_device)
                        .map(move |msg| Message::List(i, msg)),
                )
            });

        let packages_scrollable = scrollable(packages).height(Length::FillPortion(6));

        let description_scroll =
            scrollable(text_editor(&self.description_content).on_action(Message::DescriptionEdit));

        let description_panel = container(description_scroll)
            .padding(6)
            .height(Length::FillPortion(2))
            .width(Length::Fill)
            .style(style::frame_container());

        let review_selection = {
            let tmp_widget = text(format!(
                "Review selection ({})",
                self.selected_packages.len()
            ));
            if self.selected_packages.is_empty() {
                button(tmp_widget).padding(10)
            } else {
                button_primary(tmp_widget).on_press(Message::ApplyActionOnSelection)
            }
        };

        let mut export_selection = button(text(format!(
            "Export current selection ({})",
            self.selected_packages.len()
        )))
        .padding(10);
        if !self.selected_packages.is_empty() {
            export_selection = export_selection
                .on_press(Message::ExportSelection)
                .style(style::primary_button());
        }
        let export_selection = export_selection;

        let action_row = row![
            export_selection,
            Space::new(Length::Fill, Length::Shrink),
            review_selection
        ]
        .width(Length::Fill)
        .spacing(10)
        .align_y(Alignment::Center);

        let unavailable = container(
            column![
                text("ADB is not authorized to access this user!").size(20)
                    .color(danger),
                text("The most likely reason is that it is the user of your work profile (also called Secure Folder on Samsung devices). There's really no solution, other than completely disabling your work profile in your device settings.")
                    .color(commentary)
                    .width(Length::Fill)
                    .align_x(alignment::Horizontal::Center),
            ]
            .spacing(6)
            .align_x(Alignment::Center)
        )
        .padding(10)
        .align_x(Alignment::Center)
        .style(style::bordered_frame_container());

        let control_panel = self.control_panel(selected_device);
        let content = if selected_device.user_list.is_empty()
            || self
                .selected_user
                .map_or(true, |u| !self.phone_packages[u.index].is_empty())
        {
            column![
                control_panel,
                packages_scrollable,
                description_panel,
                action_row,
            ]
        } else {
            column![
                control_panel,
                container(unavailable)
                    .height(Length::Fill)
                    .align_y(Alignment::Center),
            ]
        }
        .width(Length::Fill)
        .spacing(10)
        .align_x(Alignment::Center);

        if self.selection_modal {
            return Modal::new(
                content.padding(10),
                self.apply_selection_modal(
                    selected_device,
                    settings,
                    &self.phone_packages[self.selected_user.unwrap_or_default().index],
                ),
            )
            .on_blur(Message::ModalHide)
            .into();
        }

        if self.export_modal {
            let title = container(row![text("Success").size(24)].align_y(Alignment::Center))
                .width(Length::Fill)
                .style(style::frame_container())
                .padding(Padding {
                    top: 10.0,
                    bottom: 10.0,
                    ..Default::default()
                })
                .align_y(Alignment::Center)
                .align_x(Alignment::Center);

            let text_box = row![
                text(format!("Exported current selection into file.\nFile is exported in same directory where {NAME} is located.")).width(Length::Fill),
            ].padding(20);

            let file_row = row![text(EXPORT_FILE_NAME).color(commentary)].padding(20);

            let modal_btn_row = row![
                Space::new(Length::Fill, Length::Shrink),
                button(text("Close").width(Length::Shrink))
                    .width(Length::Shrink)
                    .on_press(Message::ModalHide),
                Space::new(Length::Fill, Length::Shrink),
            ];

            let ctn = container(column![title, text_box, file_row, modal_btn_row])
                .height(Length::Shrink)
                .width(500)
                .padding(10)
                .style(style::frame_container());

            return Modal::new(content.padding(10), ctn)
                .on_blur(Message::ModalHide)
                .into();
        }

        if let Some(err) = &self.error_modal {
            error_view(err, content, self.copy_confirmation).into()
        } else {
            container(content).height(Length::Fill).padding(10).into()
        }
    }

    #[allow(clippy::too_many_lines)]
    fn apply_selection_modal(
        &self,
        device: &Phone,
        settings: &Settings,
        packages: &[PackageRow],
    ) -> Element<Message> {
        let palette = crate::core::theme::string_to_theme(&settings.general.theme).palette();
        let commentary = palette.surface;
        let danger = palette.bright_error;
        let ok_color = palette.bright_secondary;

        const PACK_NO_USER_MSG: &str = "`selected_packages` implies a user must be selected";

        let mut summaries = Removal::CATEGORIES.map(SummaryEntry::from);
        for p in packages.iter().filter(|p| p.selected) {
            let summary = &mut summaries[p.removal as usize];
            match p.state {
                PackageState::Uninstalled | PackageState::Disabled => summary.restore += 1,
                _ => summary.discard += 1,
            }
        }

        let radio_btn_users = device.user_list.iter().filter(|&u| !u.protected).fold(
            row![].spacing(10),
            |row, &user| {
                row.push(
                    radio(
                        user.to_string(),
                        user,
                        self.selected_user,
                        Message::ModalUserSelected,
                    )
                    .size(24),
                )
            },
        );

        let title_ctn =
            container(row![text("Review your selection").size(24)].align_y(Alignment::Center))
                .width(Length::Fill)
                .style(style::frame_container())
                .padding(Padding {
                    top: 10.0,
                    bottom: 10.0,
                    ..Default::default()
                })
                .align_y(Alignment::Center)
                .align_x(Alignment::Center);

        let users_ctn = container(radio_btn_users)
            .padding(10)
            .align_x(Alignment::Center)
            .style(style::frame_container());

        let explaination_ctn = container(
            row![
                text("The action for the selected user will be applied to all other users")
                    .color(danger),
                tooltip(
                    text("\u{EA0C}")
                        .font(ICONS)
                        .width(22)
                        .align_x(alignment::Horizontal::Center)
                        .color(commentary),
                    "Let's say you choose user 0. If a selected package on user 0\n\
                        is set to be uninstalled and if this same package is disabled on user 10,\n\
                        then the package on both users will be uninstalled.",
                    tooltip::Position::Top,
                )
                .gap(20)
                .padding(10)
                .style(style::tooltip_container())
            ]
            .spacing(10),
        )
        .align_x(Alignment::Center)
        .padding(10)
        .style(style::bordered_frame_container());

        let modal_btn_row = row![
            button(text("Cancel")).on_press(Message::ModalHide),
            horizontal_space(),
            button(text("Apply")).on_press(Message::ModalValidate),
        ]
        .padding(Padding {
            right: 15.0,
            bottom: 10.0,
            left: 10.0,
            ..Default::default()
        });

        let recap_view = summaries
            .iter()
            .fold(column![].spacing(6).width(Length::Fill), |col, r| {
                col.push(recap(settings, r))
            });

        let selected_idx = self.selected_user.expect(PACK_NO_USER_MSG).index;

        let selected_pkgs_ctn = container(
            container(scrollable(
                container(
                    if self.selected_packages.iter().any(|s| s.0 == selected_idx) {
                        self.selected_packages
                            .iter()
                            .filter(|s| s.0 == selected_idx)
                            .fold(
                                column![].spacing(6).width(Length::Fill),
                                |col, selection| {
                                    col.push(
                                        row![
                                            row![text(
                                                self.phone_packages[selection.0][selection.1]
                                                    .removal
                                                    .to_string()
                                            )]
                                            .width(120),
                                            row![text(
                                                self.phone_packages[selection.0][selection.1]
                                                    .uad_list
                                                    .to_string()
                                            )]
                                            .width(50),
                                            row![text(
                                                &self.phone_packages[selection.0][selection.1].name
                                            )]
                                            .width(540),
                                            horizontal_space(),
                                            row![
                                                match self.phone_packages[selection.0][selection.1]
                                                    .state
                                                {
                                                    PackageState::Enabled =>
                                                        if settings.device.disable_mode {
                                                            text("Disable").color(danger)
                                                        } else {
                                                            text("Uninstall").color(danger)
                                                        },
                                                    PackageState::Disabled =>
                                                        text("Enable").color(ok_color),
                                                    PackageState::Uninstalled =>
                                                        text("Restore").color(ok_color),
                                                    PackageState::All =>
                                                        text("Impossible").color(danger),
                                                }
                                            ]
                                            .width(70),
                                        ]
                                        .width(Length::Fill)
                                        .spacing(20),
                                    )
                                },
                            )
                    } else {
                        column![text("No packages selected for this user")]
                            .align_x(Alignment::Center)
                            .width(Length::Fill)
                    },
                )
                .padding(10)
                .width(Length::Fill),
            ))
            .width(Length::Fill)
            .style(style::frame_container()),
        )
        .width(Length::Fill)
        .max_height(150)
        .padding(Padding {
            right: 10.0,
            left: 10.0,
            ..Default::default()
        });

        container(
            if device.user_list.iter().filter(|&u| !u.protected).count() > 1
                && settings.device.multi_user_mode
            {
                column![
                    title_ctn,
                    users_ctn,
                    row![explaination_ctn].padding(Padding {
                        right: 10.0,
                        left: 10.0,
                        ..Default::default()
                    }),
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            } else if !settings.device.multi_user_mode {
                column![
                    title_ctn,
                    users_ctn,
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            } else {
                column![
                    title_ctn,
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            },
        )
        .width(900)
        .height(Length::Shrink)
        .max_height(700)
        .style(style::background_container())
        .into()
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
            .enumerate()
            .filter(|(_, p)| {
                (list_filter == UadList::All || p.uad_list == list_filter)
                    && (package_filter == PackageState::All || p.state == package_filter)
                    && (removal_filter == Removal::All || p.removal == removal_filter)
                    && (self.input_value.is_empty()
                        || p.name.contains(&self.input_value)
                        || p.description.contains(&self.input_value))
            })
            .map(|(i, _)| i)
            .collect();
    }

    async fn load_packages<S: AsRef<str>>(
        uad_list: PackageHashMap,
        device_serial: S,
        user_list: Vec<User>,
    ) -> Vec<Vec<PackageRow>> {
        let serial = device_serial.as_ref();
        if user_list.len() <= 1 {
            vec![fetch_packages(&uad_list, serial, None)]
        } else {
            user_list
                .iter()
                .map(|user| fetch_packages(&uad_list, serial, Some(user.id)))
                .collect()
        }
    }

    async fn init_apps_view(remote: bool, has_phone: bool) -> (PackageHashMap, UadListState) {
        match load_debloat_lists(remote) {
            Ok(list) => {
                if !has_phone {
                    warn!("AppsView ready but no phone found");
                }
                (list, UadListState::Done)
            }
            Err(local_list) => {
                error!("Error loading remote debloat list. Fallback to embedded list");
                (local_list, UadListState::Failed)
            }
        }
    }
}

fn error_view<'a>(
    error: &'a str,
    content: Column<'a, Message>,
    copy_confirmation: bool,
) -> Modal<'a, Message> {
    let title_ctn = container(
        row![text("Failed to perform ADB operation").size(24)].align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .style(style::frame_container())
    .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        ..Default::default()
    })
    .align_y(Alignment::Center)
    .align_x(Alignment::Center);

    let modal_btn_row = row![
        button(
            text(if copy_confirmation {
                "Copied!"
            } else {
                "Copy error"
            })
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .on_press_maybe(if copy_confirmation {
            None
        } else {
            Some(Message::CopyError(error.to_string()))
        })
        .style(move |theme: &iced::Theme, status: button::Status| {
            if copy_confirmation {
                style::primary_button()(theme, status)
            } else {
                style::secondary_button()(theme, status)
            }
        }),
        button(
            text("Close")
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .on_press(Message::ModalHide)
    ]
    .padding(Padding {
        top: 10.0,
        ..Default::default()
    });

    let text_box = scrollable(text(error).width(Length::Fill)).height(400);

    let ctn = container(column![title_ctn, text_box, modal_btn_row])
        .height(Length::Shrink)
        .max_height(700)
        .padding(10)
        .style(style::frame_container());

    Modal::new(content, ctn).on_blur(Message::ModalHide)
}

fn waiting_view<'a>(
    displayed_text: &(impl ToString + ?Sized),
    btn: Option<button::Button<'a, Message>>,
    text_color: Option<Color>,
) -> Element<'a, Message> {
    let mut text_widget = text(displayed_text.to_string()).size(20);
    if let Some(color) = text_color {
        text_widget = text_widget.color(color);
    }

    let mut col = column![]
        .spacing(10)
        .align_x(Alignment::Center)
        .push(text_widget);

    if let Some(btn) = btn {
        col = col.push(btn.style(style::primary_button()).padding(10));
    }

    container(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .align_x(Alignment::Center)
        .into()
}

fn build_action_pkg_commands(
    packages: &[Vec<PackageRow>],
    device: &Phone,
    settings: &DeviceSettings,
    selection: (usize, usize),
) -> Vec<Task<Message>> {
    let pkg = &packages[selection.0][selection.1];
    let wanted_state = pkg.state.opposite(settings.disable_mode);

    let mut commands = Vec::new();
    for u in device.user_list.iter().filter(|u| !u.protected) {
        if let Some(user_pkgs) = packages.get(u.index) {
            if let Some(u_pkg) = user_pkgs.get(selection.1) {
                if settings.multi_user_mode || u_pkg.selected {
                    let wanted_state_u = if settings.multi_user_mode {
                        wanted_state
                    } else {
                        u_pkg.state.opposite(settings.disable_mode)
                    };
                    let actions =
                        apply_pkg_state_commands(&u_pkg.into(), wanted_state_u, *u, device);
                    for (j, action) in actions.into_iter().enumerate() {
                        let p_info = PackageInfo {
                            i_user: u.index,
                            index: selection.1,
                            removal: pkg.removal.to_string(),
                        };
                        commands.push(Task::perform(
                            adb_shell_command(device.adb_id.clone(), action, p_info),
                            if j == 0 {
                                Message::ChangePackageState
                            } else {
                                |_| Message::Nothing
                            },
                        ));
                    }
                }
            }
        }
    }
    commands
}

fn recap<'a>(settings: &Settings, recap: &SummaryEntry) -> Element<'a, Message> {
    let palette = crate::core::theme::string_to_theme(&settings.general.theme).palette();
    let danger = palette.bright_error;
    let ok_color = palette.bright_secondary;

    container(
        row![
            text(recap.category.to_string())
                .size(19)
                .width(Length::FillPortion(1)),
            vertical_rule(5),
            row![
                if settings.device.disable_mode {
                    text("Disable").color(danger)
                } else {
                    text("Uninstall").color(danger)
                },
                horizontal_space(),
                text(recap.discard.to_string()).color(danger)
            ]
            .width(Length::FillPortion(1)),
            vertical_rule(5),
            row![
                if settings.device.disable_mode {
                    text("Enable").color(ok_color)
                } else {
                    text("Restore").color(ok_color)
                },
                horizontal_space(),
                text(recap.restore.to_string()).color(ok_color)
            ]
            .width(Length::FillPortion(1))
        ]
        .spacing(20)
        .padding(Padding {
            right: 10.0,
            ..Default::default()
        })
        .width(Length::Fill)
        .align_y(Alignment::Center),
    )
    .padding(10)
    .width(Length::Fill)
    .height(45)
    .style(style::frame_container())
    .into()
}

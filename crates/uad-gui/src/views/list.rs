use crate::helpers::{button_primary, export_result_dialog};
use crate::style;
use crate::theme::Theme;
use crate::widgets::navigation_menu::ICONS;
use log::{error, info, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use uad_core::config::DeviceSettings;
use uad_core::relations::{DevicePackage, Relations, RemovalPlan};
use uad_core::sync::{AdbError, Phone, User, apply_pkg_state_commands};
use uad_core::uad_lists::{
    Opposite, PackageHashMap, PackageState, Removal, UadList, UadListState, load_debloat_lists,
};
use uad_core::utils::{
    NAME, export_selection, fetch_packages, matches_search, open_url, truncate_description,
};

use crate::views::settings::Settings;
use crate::widgets::modal::Modal;
use crate::widgets::package_row::{Message as RowMessage, PackageRow, action_label};
use crate::widgets::text;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{
    Column, Space, button, checkbox, column, container, pick_list, radio, row, rule, scrollable,
    text_editor, text_input, tooltip,
};
use iced::{Alignment, Element, Length, Renderer, Task, alignment};

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

    // Handle verification completion on the UI thread after async work
    fn on_verify_and_fallback_finished(
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

    /// Builds the main view for the app list interface
    pub fn view(
        &self,
        settings: &Settings,
        selected_device: &Phone,
    ) -> Element<'_, Message, Theme, Renderer> {
        match &self.loading_state {
            LoadingState::DownloadingList => waiting_view(
                &format!("Downloading latest {NAME} lists from GitHub. Please wait..."),
                Some(button("No internet?").on_press(Message::LoadUadList(false))),
                style::Text::Default,
            ),
            LoadingState::FindingPhones => {
                if self.is_adb_satisfied {
                    waiting_view("Finding connected devices...", None, style::Text::Default)
                } else {
                    waiting_view(
                        "ADB is not installed on your system, install ADB and relaunch application.",
                        Some(button("Read on how to get started.")
                    .on_press(Message::GoToUrl(PathBuf::from(
                        "https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Getting-started",
                    )))),
                        style::Text::Danger,
                    )
                }
            }
            LoadingState::LoadingPackages => waiting_view(
                "Pulling packages from the device. Please wait...",
                None,
                style::Text::Default,
            ),
            LoadingState::_UpdatingUad => waiting_view(
                &format!("Updating {NAME}. Please wait..."),
                None,
                style::Text::Default,
            ),
            LoadingState::RestoringDevice(device) => waiting_view(
                &format!("Restoring device: {device}"),
                None,
                style::Text::Default,
            ),
            LoadingState::Ready => self.ready_view(settings, selected_device),
            LoadingState::FailedToUpdate => waiting_view(
                "Failed to download update",
                Some(button("Go back").on_press(Message::LoadUadList(false))),
                style::Text::Danger,
            ),
        }
    }

    fn control_panel(&self, selected_device: &Phone) -> Element<'_, Message, Theme, Renderer> {
        let search_packages = text_input("Search packages...", &self.input_value)
            .width(Length::Fill)
            .on_input(Message::SearchInputChanged)
            .padding([5, 10]);

        let select_all_checkbox = checkbox(self.all_selected)
            .on_toggle(Message::ToggleAllSelected)
            .size(20)
            .style(style::CheckBox::SettingsEnabled)
            .spacing(0); // no label, so remove space entirely

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
            .style(style::Container::Tooltip)
            .gap(4)
        ]
        .padding(8);

        let user_picklist = pick_list(
            selected_device.user_list.clone(),
            self.selected_user,
            Message::UserSelected,
        )
        .width(85);

        let list_picklist =
            pick_list(UadList::ALL, self.selected_list, Message::ListSelected).width(96);

        let package_state_picklist = pick_list(
            PackageState::ALL,
            self.selected_package_state,
            Message::PackageStateSelected,
        )
        .width(120);

        let removal_picklist = pick_list(
            Removal::ALL,
            self.selected_removal,
            Message::RemovalSelected,
        )
        .width(150);

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
        .padding(iced::Padding {
            top: 0.0,
            right: 16.0,
            bottom: 0.0,
            left: 0.0,
        })
        .into()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Complex layout; further refactor later"
    )]
    fn ready_view(
        &self,
        settings: &Settings,
        selected_device: &Phone,
    ) -> Element<'_, Message, Theme, Renderer> {
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

        let packages_scrollable = scrollable(packages)
            .height(Length::FillPortion(6))
            .style(style::Scrollable::Packages);

        let description_scroll =
            scrollable(text_editor(&self.description_content).on_action(Message::DescriptionEdit))
                .style(style::Scrollable::Description);

        let description_panel =
            container(column![self.details_header(), description_scroll].spacing(6))
                .padding(6)
                .height(Length::FillPortion(2))
                .width(Length::Fill)
                .style(style::Container::Frame);

        let review_selection = {
            let tmp_widget = text(format!(
                "Review selection ({})",
                self.selected_packages.len()
            ));
            if self.selected_packages.is_empty() {
                button(tmp_widget).padding([5, 10])
            } else {
                button_primary(tmp_widget).on_press(Message::ApplyActionOnSelection)
            }
        };

        let mut export_selection = button(text(format!(
            "Export current selection ({})",
            self.selected_packages.len()
        )))
        .padding([5, 10]);
        if !self.selected_packages.is_empty() {
            export_selection = export_selection
                .on_press(Message::ExportSelection)
                .style(style::Button::Primary);
        }
        // lock
        let export_selection = export_selection;

        let action_row = row![
            export_selection,
            Space::new().width(Length::Fill).height(Length::Shrink),
            review_selection
        ]
        .width(Length::Fill)
        .spacing(10)
        .align_y(Alignment::Center);

        let unavailable = container(
                    column![
                        text("ADB is not authorized to access this user!").size(20)
                            .style(style::Text::Danger),
                        text("The most likely reason is that it is the user of your work profile (also called Secure Folder on Samsung devices). There's really no solution, other than completely disabling your work profile in your device settings.")
                            .style(style::Text::Commentary)
                            .align_x(alignment::Horizontal::Center),
                    ]
                    .spacing(6)
                    .align_x(Alignment::Center)
                )
                .padding(10)
                .center_x(Length::Shrink)
                .style(style::Container::BorderedFrame);

        let control_panel = self.control_panel(selected_device);

        // Fallback notifications area
        let notifications_area: Element<'_, Message, Theme, Renderer> =
            if self.fallback_notifications.is_empty() {
                Space::new().into()
            } else {
                let notification_texts: Vec<_> = self
                    .fallback_notifications
                    .iter()
                    .map(|msg| text(msg).style(style::Text::Commentary).into())
                    .collect();

                container(
                    column![
                        text("Fallback Actions Performed:").style(style::Text::Default),
                        column(notification_texts).spacing(4),
                        row![
                            Space::new().width(Length::Fill).height(Length::Shrink),
                            button(text("Dismiss"))
                                .on_press(Message::DismissFallbackNotifications)
                                .style(style::Button::Primary)
                                .padding([4, 10]),
                        ]
                    ]
                    .spacing(6),
                )
                .padding(8)
                .style(style::Container::BorderedFrame)
                .into()
            };

        let content = if selected_device.user_list.is_empty()
            || match self.selected_user {
                Some(u) => !self.phone_packages[u.index].is_empty(),
                // If no user has been selected,
                // then it could be considered as "equivalent"
                // to the case where the `user_list` is empty?
                // However, this is inconsistent,
                // because other parts of the code simply use a `default` `User`.
                None => true,
            } {
            column![
                control_panel,
                notifications_area,
                packages_scrollable,
                description_panel,
                action_row,
            ]
        } else {
            column![
                control_panel,
                notifications_area,
                container(unavailable)
                    .height(Length::Fill)
                    .center_y(Length::Fill),
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

        if let Some(result) = &self.export_result {
            let dialog = export_result_dialog(
                "the selected package names",
                result,
                Message::ModalHide,
                Message::GoToUrl,
            );
            return Modal::new(content.padding(10), dialog)
                .on_blur(Message::ModalHide)
                .into();
        }

        if let Some(selection) = self.pending_action {
            return Modal::new(
                content.padding(10),
                self.confirm_action_modal(settings, selection),
            )
            .on_blur(Message::ModalHide)
            .into();
        }

        if let Some(err) = &self.error_modal {
            error_view(err, content, self.copy_confirmation).into()
        } else {
            container(content).height(Length::Fill).padding(10).into()
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Modal construction is verbose by nature"
    )]
    fn apply_selection_modal(
        &self,
        device: &Phone,
        settings: &Settings,
        packages: &[PackageRow],
    ) -> Element<'_, Message, Theme, Renderer> {
        const PACK_NO_USER_MSG: &str = "`selected_packages` implies a user must be selected";

        // 5 element slice is cheap
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
                        format!("{}", user.clone()),
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
                .style(style::Container::Frame)
                .padding([10, 0])
                .center_y(Length::Shrink)
                .center_x(Length::Fill);

        let users_ctn = container(radio_btn_users)
            .padding(10)
            .center_x(Length::Shrink)
            .style(style::Container::Frame);

        let explaination_ctn = container(
            row![
                text("The action for the selected user will be applied to all other users")
                    .style(style::Text::Danger),
                tooltip(
                    text("\u{EA0C}")
                        .font(ICONS)
                        .width(22)
                        .align_x(alignment::Horizontal::Center)
                        .style(style::Text::Commentary),
                    "Let's say you choose user 0. If a selected package on user 0\n\
                        is set to be uninstalled and if this same package is disabled on user 10,\n\
                        then the package on both users will be uninstalled.",
                    tooltip::Position::Top,
                )
                .gap(20)
                .padding(10)
                .style(style::Container::Tooltip)
            ]
            .spacing(10),
        )
        .center_x(Length::Shrink)
        .padding(10)
        .style(style::Container::BorderedFrame);

        let modal_btn_row = row![
            button(text("Cancel")).on_press(Message::ModalHide),
            Space::new().width(Length::Fill),
            button(text("Apply")).on_press(Message::ModalValidate),
        ]
        .padding(iced::Padding {
            top: 0.0,
            right: 15.0,
            bottom: 10.0,
            left: 10.0,
        });

        let recap_view = summaries
            .iter()
            .fold(column![].spacing(6).width(Length::Fill), |col, r| {
                col.push(recap(settings, r))
            });

        let selected_pkgs_ctn = container(
            container(
                scrollable(
                    container(
                        if self
                            .selected_packages
                            .iter()
                            .any(|s| s.0 == self.selected_user.expect(PACK_NO_USER_MSG).index)
                        {
                            self.selected_packages
                                .iter()
                                .filter(|s| {
                                    s.0 == self.selected_user.expect(PACK_NO_USER_MSG).index
                                })
                                .fold(
                                    column![].spacing(6).width(Length::Shrink),
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
                                                        .list
                                                        .to_string()
                                                )]
                                                .width(55),
                                                row![text(
                                                    self.phone_packages[selection.0][selection.1]
                                                        .name
                                                        .clone()
                                                ),]
                                                .width(540),
                                                row![match self.phone_packages[selection.0]
                                                    [selection.1]
                                                    .state
                                                {
                                                    PackageState::Enabled =>
                                                        if settings.device.disable_mode {
                                                            text("Disable")
                                                                .style(style::Text::Danger)
                                                        } else {
                                                            text("Uninstall")
                                                                .style(style::Text::Danger)
                                                        },
                                                    PackageState::Disabled =>
                                                        text("Enable").style(style::Text::Ok),
                                                    PackageState::Uninstalled =>
                                                        text("Restore").style(style::Text::Ok),
                                                    PackageState::All => text("Impossible")
                                                        .style(style::Text::Danger),
                                                },]
                                                .width(70),
                                            ]
                                            .width(Length::Shrink)
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
                    .width(Length::Shrink)
                    .style(style::Container::Invisible),
                )
                .direction(Direction::Both {
                    vertical: Scrollbar::default(),
                    horizontal: Scrollbar::default(),
                })
                .style(style::Scrollable::Description),
            )
            .width(Length::Fill)
            .style(style::Container::Frame),
        )
        .width(Length::Fill)
        .max_height(150)
        .padding([0, 10]);

        let i_user = self.selected_user.unwrap_or_default().index;
        let selection: Vec<usize> = self
            .selected_packages
            .iter()
            .filter(|s| s.0 == i_user)
            .map(|s| s.1)
            .collect();
        let relations_ctn =
            container(self.relations_panel(settings, i_user, &selection)).padding([0, 10]);

        container(
            if device.user_list.iter().filter(|&u| !u.protected).count() > 1
                && settings.device.multi_user_mode
            {
                column![
                    title_ctn,
                    users_ctn,
                    row![explaination_ctn].padding([0, 10]),
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    relations_ctn,
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
                    relations_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            } else {
                column![
                    title_ctn,
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    relations_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            },
        )
        .width(900)
        .height(Length::Shrink)
        .max_height(760)
        .style(style::Container::Background)
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

    #[expect(clippy::unused_async, reason = "1 call-site")]
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

    #[expect(clippy::unused_async, reason = "1 call-site")]
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

    #[allow(
        clippy::too_many_lines,
        reason = "Complex verification and fallback logic"
    )]
    //this functions matches the adb command to two types, when it is a soft issue and when it is a hard issue
    // soft issue: utilizes core::sync::attempt_fallback in the case the state that wanted to be changed wasn't in fact changed
    // hard issue: does basically nothing in most cases FIXME, besides handling enabling packages.
    // also this really should be refactored, these are gigantic code blocks
    fn on_verify_and_fallback(
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
                        let actual_state_opt = uad_core::sync::verify_package_state(
                            &pkg_name,
                            device.adb_id.as_str(),
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
                                            let actual_state =
                                                uad_core::sync::verify_package_state(
                                                    &pkg_name,
                                                    &device.adb_id,
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
}

impl List {
    fn on_dismiss_fallback_notifications(&mut self) -> Task<Message> {
        self.fallback_notifications.clear();
        Task::none()
    }

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
    fn device_lookup(&self, i_user: usize) -> HashMap<&str, DevicePackage> {
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
    fn removal_plan(
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
    fn related_indices(&self, i_user: usize, selection: &[usize]) -> Vec<usize> {
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
    fn needs_confirmation(&self, i_user: usize, i_package: usize) -> bool {
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

    /// Toggles one package, plus `related` packages of the same user.
    fn run_single_action(
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

    fn on_confirm_pending_action(
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

    /// Related packages, warnings about dependents and missing requirements
    /// for `selection` (package indexes of one user). Empty when nothing applies.
    fn relations_panel(
        &self,
        settings: &Settings,
        i_user: usize,
        selection: &[usize],
    ) -> Element<'_, Message, Theme, Renderer> {
        let (plan, missing) = self.removal_plan(i_user, selection);
        let rating = |name: &str| {
            self.phone_packages[i_user]
                .iter()
                .find(|p| p.name == name)
                .map_or(Removal::Unlisted, |p| p.removal)
        };
        let verb = if settings.device.disable_mode {
            "disable"
        } else {
            "remove"
        };

        let mut lines: Vec<Element<'_, Message, Theme, Renderer>> = vec![];
        if !plan.related.is_empty() {
            lines.push(
                checkbox(self.include_related)
                    .label(format!(
                        "Also {verb} {} related package(s) listed below",
                        plan.related.len()
                    ))
                    .on_toggle(Message::IncludeRelatedToggled)
                    .size(18)
                    .style(style::CheckBox::SettingsEnabled)
                    .into(),
            );
            for related in &plan.related {
                lines.push(
                    text(format!(
                        "    {} [{}]: {}",
                        related.name,
                        rating(&related.name),
                        related.reason
                    ))
                    .style(style::Text::Commentary)
                    .into(),
                );
            }
        }
        for skipped in &plan.excluded_unsafe {
            lines.push(
                text(format!(
                    "{} is rated Unsafe ({}). It is never added automatically.",
                    skipped.name, skipped.reason
                ))
                .style(style::Text::Danger)
                .into(),
            );
        }
        for risk in &plan.at_risk {
            lines.push(
                text(format!(
                    "Warning: {} may stop working, because it needs {}.",
                    risk.dependent, risk.requirement
                ))
                .style(style::Text::Danger)
                .into(),
            );
        }
        for (package, requirement) in &missing {
            lines.push(
                text(format!(
                    "Warning: {package} needs {requirement}, which is not enabled. \
                     Restore it too, or {package} may not work."
                ))
                .style(style::Text::Danger)
                .into(),
            );
        }

        if lines.is_empty() {
            return Space::new().into();
        }
        container(scrollable(column(lines).spacing(4).padding(4)))
            .width(Length::Fill)
            .max_height(170)
            .padding(8)
            .style(style::Container::BorderedFrame)
            .into()
    }

    /// Asks before acting on one package, showing its rating and relations.
    fn confirm_action_modal(
        &self,
        settings: &Settings,
        (i_user, i_package): (usize, usize),
    ) -> Element<'_, Message, Theme, Renderer> {
        let package = &self.phone_packages[i_user][i_package];
        let action = action_label(package.state, settings.device.disable_mode);
        let removing = package.state == PackageState::Enabled;

        let mut body = column![
            text(format!("{action} {}?", package.name)).size(22),
            row![
                text("Removal rating:"),
                text(package.removal.to_string()).style(rating_style(package.removal)),
            ]
            .spacing(6),
            text(package.removal.explanation()).style(if removing {
                rating_style(package.removal)
            } else {
                style::Text::Commentary
            }),
            text(truncate_description(&package.description, 240)),
            self.relations_panel(settings, i_user, &[i_package]),
        ]
        .spacing(12);

        if removing && settings.device.disable_mode {
            body = body.push(
                text("Disabling keeps the package installed. You can enable it again at any time.")
                    .style(style::Text::Commentary),
            );
        } else if removing {
            body = body.push(
                text(
                    "Uninstalling removes the package for this user only. \
                     You can restore it later from the list (filter by \"Uninstalled\").",
                )
                .style(style::Text::Commentary),
            );
        }

        let confirm = button(text(action))
            .padding([5, 10])
            .on_press(Message::ConfirmPendingAction)
            .style(if removing {
                style::Button::UninstallPackage
            } else {
                style::Button::RestorePackage
            });
        body = body.push(row![
            button(text("Cancel"))
                .padding([5, 10])
                .on_press(Message::ModalHide),
            Space::new().width(Length::Fill),
            confirm,
        ]);

        container(body)
            .width(680)
            .padding(20)
            .style(style::Container::Background)
            .into()
    }

    /// Name, list and rating of the package shown in the details panel.
    fn details_header(&self) -> Element<'_, Message, Theme, Renderer> {
        let i_user = self.selected_user.unwrap_or_default().index;
        match self
            .phone_packages
            .get(i_user)
            .and_then(|packages| packages.get(self.current_package_index))
            .filter(|package| package.current)
        {
            Some(package) => row![
                text(&package.name).size(16),
                text(format!("list: {}", package.list)).style(style::Text::Commentary),
                text(package.removal.to_string()).style(rating_style(package.removal)),
            ]
            .spacing(14)
            .align_y(Alignment::Center)
            .into(),
            None => text("Click a package to see what it does and what depends on it.")
                .style(style::Text::Commentary)
                .into(),
        }
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

/// Text colour for a removal rating: green for low risk, red for high risk.
fn rating_style(removal: Removal) -> fn(&Theme) -> iced::widget::text::Style {
    match removal {
        Removal::Recommended => style::Text::Ok,
        Removal::Expert | Removal::Unsafe => style::Text::Danger,
        Removal::Advanced | Removal::Unlisted | Removal::All => style::Text::Default,
    }
}
fn error_view<'a>(
    error: &'a str,
    content: Column<'a, Message, Theme, Renderer>,
    copy_confirmation: bool,
) -> Modal<'a, Message, Theme, Renderer> {
    let title_ctn = container(
        row![text("Failed to perform ADB operation").size(24)].align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .style(style::Container::Frame)
    .padding([10, 0])
    .center_y(Length::Shrink)
    .center_x(Length::Shrink);

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
        .style(style::Button::Primary),
        button(
            text("Close")
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .on_press(Message::ModalHide)
    ]
    .padding(iced::Padding {
        top: 10.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    });

    let text_box = scrollable(text(error).width(Length::Fill)).height(400);

    let ctn = container(column![title_ctn, text_box, modal_btn_row])
        .height(Length::Shrink)
        .max_height(700)
        .padding(10)
        .style(style::Container::Frame);

    Modal::new(content, ctn).on_blur(Message::ModalHide)
}

fn waiting_view<'a>(
    displayed_text: &(impl ToString + ?Sized),
    btn: Option<button::Button<'a, Message, Theme, Renderer>>,
    text_style: impl Fn(&Theme) -> iced::widget::text::Style + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    let col = column![]
        .spacing(10)
        .align_x(Alignment::Center)
        .push(text(displayed_text.to_string()).style(text_style).size(20));

    let col = match btn {
        Some(btn) => col.push(btn.style(style::Button::Primary).padding([5, 10])),
        None => col,
    };

    container(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .center_x(Length::Fill)
        .style(style::Container::Frame)
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

fn run_adb_action_with_before_states(
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

fn recap<'a>(settings: &Settings, recap: &SummaryEntry) -> Element<'a, Message, Theme, Renderer> {
    container(
        row![
            text(recap.category.to_string())
                .size(19)
                .width(Length::FillPortion(1)),
            rule::vertical(5),
            row![
                if settings.device.disable_mode {
                    text("Disable").style(style::Text::Danger)
                } else {
                    text("Uninstall").style(style::Text::Danger)
                },
                Space::new().width(Length::Fill),
                text(recap.discard.to_string()).style(style::Text::Danger)
            ]
            .width(Length::FillPortion(1)),
            rule::vertical(5),
            row![
                if settings.device.disable_mode {
                    text("Enable").style(style::Text::Ok)
                } else {
                    text("Restore").style(style::Text::Ok)
                },
                Space::new().width(Length::Fill),
                text(recap.restore.to_string()).style(style::Text::Ok)
            ]
            .width(Length::FillPortion(1))
        ]
        .spacing(20)
        .padding(iced::Padding {
            top: 0.0,
            right: 10.0,
            bottom: 0.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .align_y(Alignment::Center),
    )
    .padding(10)
    .width(Length::Fill)
    .height(45)
    .style(style::Container::Frame)
    .into()
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

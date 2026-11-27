use crate::helpers::button_primary;
use crate::style;
use crate::theme::Theme;
use crate::widgets::navigation_menu::ICONS;
use log::{error, info, warn};
use std::{collections::HashMap, path::PathBuf};
use uad_core::config::DeviceSettings;
use uad_core::sync::{AdbError, Phone, User, apply_pkg_state_commands};
use uad_core::uad_lists::{
    Opposite, PackageHashMap, PackageState, Removal, UadList, UadListState, load_debloat_lists,
};
use uad_core::utils::{NAME, export_selection, matches_search, open_url, try_fetch_packages};

use crate::views::settings::Settings;
use crate::widgets::modal::Modal;
use crate::widgets::package_row::{Message as RowMessage, PackageRow};
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
    pub allow_removal: bool,
    /// Device, user and package the action was planned for.
    /// The displayed lists can change before the action completes.
    /// The device does not list the other users targeted by the same action.
    pub device: Phone,
    pub user: User,
    pub name: String,
    /// State of the package before the action
    pub state: PackageState,
    /// State the action is meant to put the package in
    pub wanted_state: PackageState,
    pub before_cross_user_states: Vec<(u16, PackageState)>,
}

#[derive(Default, Debug, Clone)]
pub enum LoadingState {
    DownloadingList,
    #[default]
    FindingPhones,
    LoadingPackages,
    WaitingForActions,
    UpdatingUad,
    Ready,
    FailedToUpdate,
}

#[derive(Default, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "Not a state-machine")]
pub struct List {
    pub loading_state: LoadingState,
    pub(crate) load_id: u64,
    pending_definitions: Option<u64>,
    loaded_serial: Option<String>,
    pub(crate) inventory_errors: HashMap<usize, String>,
    pending_actions: HashMap<String, usize>,
    /// Keep filters and eligible selections during a same-phone reread.
    preserve_selection: bool,
    /// Actions or new definitions made the last inventory read stale.
    reread_packages: bool,
    pub uad_lists: PackageHashMap,
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
    description: String,
    description_content: text_editor::Content,
    selection_modal: bool,
    error_modal: Option<String>,
    /// Path of the last exported selection
    export_modal: Option<PathBuf>,
    current_package_index: usize,
    is_adb_satisfied: bool,
    copy_confirmation: bool,
    fallback_notifications: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadUadList(bool),
    LoadPhonePackages(u64, (PackageHashMap, UadListState)),
    /// Packages of the device with the given serial
    ApplyFilters(u64, String, Vec<Result<Vec<PackageRow>, String>>),
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
    Nothing,
    ModalHide,
    ModalUserSelected(User),
    ModalValidate,
    ADBSatisfied(bool),
    UpdateFailed,
    GoToUrl(PathBuf),
    ExportSelection,
    SelectionExported(Result<PathBuf, String>),
    DescriptionEdit(text_editor::Action),
    CopyError(String),
    HideCopyConfirmation,
}

pub struct SummaryEntry {
    category: Removal,
    discard: usize,
    restore: usize,
}

#[derive(Debug, Clone)]
pub struct VerifyAndFallbackResult {
    pub info: PackageInfo,
    pub new_state: Option<PackageState>,
    /// The command changing the package state ran, confirmed or not.
    pub ran: bool,
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
            Message::ModalValidate | Message::List(_, RowMessage::ActionPressed)
                if !self.inventory_current(selected_device) =>
            {
                Task::none()
            }
            Message::ModalHide => self.on_modal_hide(),
            Message::ModalValidate => self.on_modal_validate(settings, selected_device),
            Message::LoadUadList(remote) => {
                self.reset_inventory();
                self.load_uad_list(remote, selected_device)
            }
            Message::LoadPhonePackages(id, (uad_lists, list_state)) if id != self.load_id => {
                // A phone switch can finish loading cached definitions before this download.
                if self.pending_definitions == Some(id) {
                    self.pending_definitions = None;
                    self.uad_lists = uad_lists;
                    *list_update_state = list_state;
                    self.reread_packages = true;
                    return self.refresh_if_needed(selected_device);
                }
                Task::none()
            }
            Message::ApplyFilters(id, ..) if id != self.load_id => Task::none(),
            Message::LoadPhonePackages(id, payload) => {
                if self.pending_definitions == Some(id) {
                    self.pending_definitions = None;
                }
                self.on_load_phone_packages(payload, selected_device, list_update_state)
            }
            Message::ApplyFilters(_, serial, packages) => self.on_apply_filters(
                &serial,
                packages,
                selected_device,
                settings.general.expert_mode,
            ),
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
            Message::VerifyAndFallbackFinished(result) => {
                self.on_verify_and_fallback_finished(result, selected_device)
            }
            Message::ModalUserSelected(user) => {
                self.on_modal_user_selected(user, settings, selected_device, list_update_state)
            }
            Message::ADBSatisfied(result) => self.on_adb_satisfied(result),
            Message::UpdateFailed => self.on_update_failed(),
            Message::GoToUrl(url) => Self::on_go_to_url(url),
            Message::ExportSelection => self.on_export_selection(settings),
            Message::SelectionExported(res) => self.on_selection_exported(res),
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
        selected_device: &Phone,
    ) -> Task<Message> {
        let serial = &result.info.device.adb_id;
        if let Some(count) = self.pending_actions.get_mut(serial) {
            *count -= 1;
            if *count == 0 {
                self.pending_actions.remove(serial);
            }
        }
        // ADB can lose the reply after the phone has applied the command.
        if selected_device.adb_id == *serial {
            self.reread_packages = true;
        }
        if let Some(notification) = result.notification {
            self.fallback_notifications.push(notification);
        }
        if let Some(err) = result.error_modal {
            self.error_modal = Some(err);
        }
        if let Some((i_user, index)) = self.current_row(&result.info, selected_device) {
            let package = &mut self.phone_packages[i_user][index];
            if let Some(state) = result.new_state {
                package.state = state;
            }
            if result.ran {
                package.selected = false;
                self.selected_packages.retain(|&x| x != (i_user, index));
            }
            Self::filter_package_lists(self);
        }
        self.refresh_if_needed(selected_device)
    }

    /// Current row of the package an action was started for.
    ///
    /// `None` if the lists were replaced by those of another device,
    /// or no longer have the package, since the action started.
    fn current_row(&self, info: &PackageInfo, selected_device: &Phone) -> Option<(usize, usize)> {
        if selected_device.adb_id != info.device.adb_id {
            return None;
        }
        let user = selected_device
            .user_list
            .iter()
            .find(|u| u.id == info.user.id)?;
        let packages = self.phone_packages.get(user.index)?;
        Some((user.index, find_package(packages, &info.name, info.index)?))
    }

    /// Number of packages selected for the user being displayed.
    fn selection_count(&self) -> usize {
        let i_user = self.selected_user.unwrap_or_default().index;
        self.selected_packages
            .iter()
            .filter(|s| s.0 == i_user)
            .count()
    }

    /// Builds the main view for the app list interface
    pub fn view(
        &self,
        settings: &Settings,
        selected_device: &Phone,
    ) -> Element<'_, Message, Theme, Renderer> {
        // The packages of a device being restored are hidden: no action can be started.
        if let Some(package) = settings.restoring(selected_device) {
            let progress = if package.is_empty() {
                "Restoring device...".to_string()
            } else {
                format!("Restoring device: {package}")
            };
            return waiting_view(&progress, None, style::Text::Default);
        }
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
            LoadingState::WaitingForActions => waiting_view(
                "Waiting for package actions to finish...",
                None,
                style::Text::Default,
            ),
            LoadingState::UpdatingUad => waiting_view(
                &format!("Updating {NAME}. Please wait..."),
                None,
                style::Text::Default,
            ),
            LoadingState::Ready => match &selected_device.user_list_error {
                Some(error) => waiting_view(error, None, style::Text::Danger),
                None if selected_device.adb_id.is_empty() => {
                    waiting_view("No device selected.", None, style::Text::Default)
                }
                None => self.ready_view(settings, selected_device),
            },
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
        let actions_enabled = self.inventory_current(selected_device);
        let packages = self
            .filtered_packages
            .iter()
            .fold(column![].spacing(6), |col, &i| {
                col.push(
                    self.phone_packages[self.selected_user.unwrap_or_default().index][i]
                        .view(settings, actions_enabled)
                        .map(move |msg| Message::List(i, msg)),
                )
            });

        let packages_scrollable = scrollable(packages)
            .height(Length::FillPortion(6))
            .style(style::Scrollable::Packages);

        let description_scroll =
            scrollable(text_editor(&self.description_content).on_action(Message::DescriptionEdit))
                .style(style::Scrollable::Description);

        let description_panel = container(description_scroll)
            .padding(6)
            .height(Length::FillPortion(2))
            .width(Length::Fill)
            .style(style::Container::Frame);

        let review_selection = {
            let tmp_widget = text(format!("Review selection ({})", self.selection_count()));
            if self.selection_count() == 0 {
                button(tmp_widget).padding([5, 10])
            } else {
                button_primary(tmp_widget).on_press(Message::ApplyActionOnSelection)
            }
        };

        let mut export_selection = button(text(format!(
            "Export current selection ({})",
            self.selection_count()
        )))
        .padding([5, 10]);
        if self.selection_count() != 0 {
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
                        text("Package action details").style(style::Text::Default),
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
                container(self.unavailable_packages())
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

        if let Some(path) = &self.export_modal {
            let title = container(row![text("Success").size(24)].align_y(Alignment::Center))
                .width(Length::Fill)
                .style(style::Container::Frame)
                .padding([10, 0])
                .center_y(Length::Shrink)
                .center_x(Length::Shrink);

            let text_box =
                row![text("Exported current selection to:").width(Length::Fill),].padding(20);

            let file_row =
                row![text(path.display().to_string()).style(style::Text::Commentary)].padding(20);

            let modal_btn_row = row![
                Space::new().width(Length::Fill).height(Length::Shrink),
                button(text("Close").width(Length::Shrink))
                    .width(Length::Shrink)
                    .on_press(Message::ModalHide),
                Space::new().width(Length::Fill).height(Length::Shrink),
            ];

            let ctn = container(column![title, text_box, file_row, modal_btn_row])
                .height(Length::Shrink)
                .width(500)
                .padding(10)
                .style(style::Container::Frame);

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
        let eligible =
            |p: &PackageRow| p.selected && (settings.general.expert_mode || !needs_expert_mode(p));
        for p in packages.iter().filter(|p| eligible(p)) {
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
            button(text("Apply")).on_press_maybe(
                (self.inventory_current(device) && packages.iter().any(eligible))
                    .then_some(Message::ModalValidate)
            ),
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
                        if self.selected_packages.iter().any(|s| {
                            s.0 == self.selected_user.expect(PACK_NO_USER_MSG).index
                                && eligible(&packages[s.1])
                        }) {
                            self.selected_packages
                                .iter()
                                .filter(|s| {
                                    s.0 == self.selected_user.expect(PACK_NO_USER_MSG).index
                                        && eligible(&packages[s.1])
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
        .style(style::Container::Background)
        .into()
    }

    fn unavailable_packages(&self) -> Element<'_, Message, Theme, Renderer> {
        let user = self.selected_user.unwrap_or_default();
        let retry = Some("Check the device connection and press Refresh to try again.");
        let (title, detail) = match self.inventory_errors.get(&user.index) {
            Some(error) if uad_core::sync::profile_access_denied(error) => (
                format!("ADB is not authorized to access {user}."),
                Some("This may be a work profile or a Secure Folder."),
            ),
            Some(error) => (
                format!("Could not read packages for {user}: {error}"),
                retry,
            ),
            None => match user.packages(&self.phone_packages) {
                Err(error) => (error, retry),
                Ok(_) => (format!("No system packages found for {user}."), None),
            },
        };
        container(
            column![text(title).size(20)]
                .extend(
                    detail
                        .into_iter()
                        .map(|line| text(line).style(style::Text::Commentary).into()),
                )
                .spacing(6)
                .align_x(Alignment::Center),
        )
        .padding(10)
        .style(style::Container::BorderedFrame)
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

    fn load_packages(
        uad_list: &PackageHashMap,
        phone: &Phone,
    ) -> Vec<Result<Vec<PackageRow>, String>> {
        phone
            .user_list
            .iter()
            .map(|user| {
                try_fetch_packages(uad_list, &phone.adb_id, phone.package_user_id(user.id)?)
                    .map(|packages| packages.into_iter().map(PackageRow::from).collect())
            })
            .collect()
    }

    fn init_apps_view(remote: bool, phone: &Phone) -> (PackageHashMap, UadListState) {
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
        self.export_modal = None;
        Task::none()
    }

    fn on_modal_validate(
        &mut self,
        settings: &Settings,
        selected_device: &mut Phone,
    ) -> Task<Message> {
        self.fallback_notifications.clear();
        let commands = self.apply_actions(
            selected_device,
            &settings.device,
            settings.general.expert_mode,
            None,
        );
        self.selection_modal = false;
        Task::batch(commands)
    }

    /// Apply a row action or the selection under review.
    fn apply_actions(
        &mut self,
        device: &Phone,
        settings: &DeviceSettings,
        expert_mode: bool,
        single_package: Option<usize>,
    ) -> Vec<Task<Message>> {
        let i_user = self.selected_user.unwrap_or_default().index;
        for user in device
            .user_list
            .iter()
            .filter(|u| !u.protected && (settings.multi_user_mode || u.index == i_user))
        {
            let inventory = match self.inventory_errors.get(&user.index) {
                Some(error) => Err(format!("Could not read {user}: {error}")),
                None => user.packages(&self.phone_packages),
            };
            if let Err(error) = inventory {
                self.error_modal = Some(format!("{error}. Refresh the device and try again."));
                return Vec::new();
            }
        }
        // Only the packages shown in the review are applied.
        let selections = match single_package {
            Some(index) => vec![(i_user, index)],
            None => self
                .selected_packages
                .iter()
                .copied()
                .filter(|s| s.0 == i_user)
                .collect(),
        };
        let mut planned = Vec::new();
        for selection in selections {
            match plan_pkg_action(
                &self.phone_packages,
                device,
                settings,
                expert_mode,
                selection,
            ) {
                Ok(plan) => planned.extend(plan),
                Err(error) => {
                    self.error_modal = Some(error);
                    return Vec::new();
                }
            }
        }
        for (p_info, actions) in &planned {
            // Nothing reports back for another user already in the wanted state.
            if actions.is_empty() && p_info.i_user != i_user {
                self.phone_packages[p_info.i_user][p_info.index].selected = false;
                self.selected_packages
                    .retain(|&x| x != (p_info.i_user, p_info.index));
            }
        }
        let tasks = pkg_action_tasks(planned);
        if !tasks.is_empty() {
            *self
                .pending_actions
                .entry(device.adb_id.clone())
                .or_default() += tasks.len();
        }
        tasks
    }

    pub(crate) fn reset_inventory(&mut self) {
        *self = Self {
            load_id: self.load_id + 1,
            pending_definitions: self.pending_definitions,
            is_adb_satisfied: self.is_adb_satisfied,
            pending_actions: std::mem::take(&mut self.pending_actions),
            uad_lists: std::mem::take(&mut self.uad_lists),
            ..Self::default()
        };
    }

    pub(crate) fn inventory_ready(&self, phone: &Phone) -> bool {
        !phone.user_list.is_empty()
            && self.loaded_serial.as_ref() == Some(&phone.adb_id)
            && !phone.adb_id.is_empty()
    }

    pub(crate) fn inventory_current(&self, phone: &Phone) -> bool {
        self.inventory_ready(phone)
            && !self.reread_packages
            && !self.pending_actions.contains_key(&phone.adb_id)
    }

    /// Re-read stale rows once the phone's last pending action is done.
    fn refresh_if_needed(&mut self, phone: &Phone) -> Task<Message> {
        if self.reread_packages
            && (self.inventory_ready(phone)
                || matches!(self.loading_state, LoadingState::WaitingForActions))
            && !self.pending_actions.contains_key(&phone.adb_id)
        {
            self.preserve_selection = self.selected_user.is_some();
            self.loaded_serial = None;
            self.load_id += 1;
            self.loading_state = LoadingState::LoadingPackages;
            self.read_packages(phone)
        } else {
            Task::none()
        }
    }

    pub(crate) fn load_uad_list(&mut self, remote: bool, selected_device: &Phone) -> Task<Message> {
        info!("{:-^65}", "-");
        info!(
            "ANDROID_SDK: {} | DEVICE: {}",
            selected_device.android_sdk, selected_device.model
        );
        info!("{:-^65}", "-");
        self.loaded_serial = None;
        self.preserve_selection = false;
        self.loading_state = LoadingState::DownloadingList;
        self.load_id += 1;
        let id = self.load_id;
        self.pending_definitions = Some(id);
        let phone = selected_device.clone();
        Task::perform(
            async move { Self::init_apps_view(remote, &phone) },
            move |lists| Message::LoadPhonePackages(id, lists),
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
        self.uad_lists = uad_list;
        *list_update_state = list_state;
        self.read_packages(selected_device)
    }

    /// Read the packages of the device in the background.
    fn read_packages(&mut self, selected_device: &Phone) -> Task<Message> {
        self.reread_packages = false;
        if selected_device.user_list.is_empty() {
            self.loading_state = LoadingState::Ready;
            return Task::none();
        }
        let id = self.load_id;
        let uad_list = self.uad_lists.clone();
        let phone = selected_device.clone();
        Task::perform(
            async move {
                let packages = Self::load_packages(&uad_list, &phone);
                (phone.adb_id, packages)
            },
            move |(serial, packages)| Message::ApplyFilters(id, serial, packages),
        )
    }

    fn on_apply_filters(
        &mut self,
        device_serial: &str,
        packages: Vec<Result<Vec<PackageRow>, String>>,
        selected_device: &Phone,
        expert_mode: bool,
    ) -> Task<Message> {
        // Another device was selected while these packages were loading.
        if device_serial != selected_device.adb_id {
            return Task::none();
        }
        // An action or definitions update may have finished while these rows were loading.
        if self.reread_packages {
            if self.pending_actions.contains_key(device_serial) {
                // The last action will start a fresh read. Discard these obsolete rows.
                self.loading_state = LoadingState::WaitingForActions;
                return Task::none();
            }
            return self.read_packages(selected_device);
        }
        self.loaded_serial = Some(device_serial.to_owned());
        self.inventory_errors.clear();
        let old_packages = std::mem::take(&mut self.phone_packages);
        self.phone_packages = packages
            .into_iter()
            .enumerate()
            .map(|(index, result)| {
                result.unwrap_or_else(|error| {
                    error!(
                        "Could not read packages on {device_serial} for user index {index}: {error}"
                    );
                    self.inventory_errors.insert(index, error);
                    Vec::new()
                })
            })
            .collect();
        self.selected_packages.clear();
        let keep_filters = std::mem::take(&mut self.preserve_selection);
        if keep_filters {
            // A same-phone reread keeps the same user list. Take the latest selections
            // from each user's old rows, including changes made while the read ran.
            for user in selected_device
                .user_list
                .iter()
                .filter(|user| !user.protected)
            {
                if let (Some(old_rows), Some(rows)) = (
                    old_packages.get(user.index),
                    self.phone_packages.get_mut(user.index),
                ) {
                    let selected: HashMap<_, _> = old_rows
                        .iter()
                        .filter(|row| row.selected)
                        .map(|row| (row.name.as_str(), row.state))
                        .collect();
                    for (index, row) in rows.iter_mut().enumerate() {
                        if selected.get(row.name.as_str()) == Some(&row.state)
                            && (expert_mode || !needs_expert_mode(row))
                        {
                            row.selected = true;
                            self.selected_packages.push((user.index, index));
                        }
                    }
                }
            }
        } else {
            self.selected_removal = Some(Removal::Recommended);
            self.selected_package_state = Some(PackageState::Enabled);
            self.selected_list = Some(UadList::All);
        }
        self.selected_user = Some(
            self.selected_user
                .and_then(|previous| {
                    selected_device
                        .user_list
                        .iter()
                        .find(|u| u.id == previous.id && u.index < self.phone_packages.len())
                        .copied()
                })
                .unwrap_or_default(),
        );
        Self::filter_package_lists(self);
        self.all_selected = keep_filters
            && !self.filtered_packages.is_empty()
            && self.filtered_packages.iter().all(|&i| {
                self.phone_packages[self.selected_user.unwrap_or_default().index][i].selected
            });
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
        let Some(row) = self
            .phone_packages
            .get_mut(i_user)
            .and_then(|rows| rows.get_mut(i_package))
        else {
            return Task::none();
        };
        #[expect(unused_must_use, reason = "side-effect")]
        {
            row.update(row_message)
                .map(move |row_message| Message::List(i_package, row_message));
        }

        match *row_message {
            RowMessage::ToggleSelection(toggle) => {
                self.toggle_selection(
                    &selected_device.user_list,
                    i_package,
                    toggle,
                    settings.general.expert_mode,
                    settings.device.multi_user_mode,
                );
                Task::none()
            }
            RowMessage::ActionPressed => {
                self.fallback_notifications.clear();
                Task::batch(self.apply_actions(
                    selected_device,
                    &settings.device,
                    settings.general.expert_mode,
                    Some(i_package),
                ))
            }
            RowMessage::PackagePressed => {
                // The previous row can belong to a longer list of another user.
                if let Some(previous) =
                    self.phone_packages[i_user].get_mut(self.current_package_index)
                {
                    previous.current = false;
                }
                let package = &mut self.phone_packages[i_user][i_package];
                self.description.clone_from(&package.description);
                self.description_content = text_editor::Content::with_text(&package.description);
                package.current = true;
                self.current_package_index = i_package;
                Task::none()
            }
        }
    }

    /// Select or deselect a package of the user being displayed.
    fn toggle_selection(
        &mut self,
        users: &[User],
        i_package: usize,
        toggle: bool,
        expert_mode: bool,
        multi_user_mode: bool,
    ) {
        let user = self.selected_user.unwrap_or_default();
        let package = &mut self.phone_packages[user.index][i_package];
        if toggle && needs_expert_mode(package) && !expert_mode {
            package.selected = false;
            return;
        }
        let users = if multi_user_mode {
            users
        } else {
            std::slice::from_ref(&user)
        };
        self.toggle_for_users(users, (user.index, i_package), toggle, expert_mode);
    }

    /// Deselect the packages that can only be selected in expert mode.
    pub fn deselect_unsafe(&mut self) {
        let packages = &mut self.phone_packages;
        self.selected_packages.retain(|&(i_user, i_package)| {
            let package = &mut packages[i_user][i_package];
            if needs_expert_mode(package) {
                package.selected = false;
            }
            package.selected
        });
    }

    fn on_apply_action_on_selection(&mut self) -> Task<Message> {
        self.selection_modal = true;
        Task::none()
    }

    fn on_user_selected(&mut self, user: User) -> Task<Message> {
        if self.phone_packages.get(user.index).is_none() {
            return Task::none();
        }
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
        self.update(
            settings,
            selected_device,
            list_update_state,
            Message::UserSelected(user),
        )
    }

    /// Select the already selected packages for every user,
    /// as if they had been selected with multi-user mode enabled.
    pub fn select_for_all_users(&mut self, users: &[User], expert_mode: bool) {
        for selection in self.selected_packages.clone() {
            self.toggle_for_users(users, selection, true, expert_mode);
        }
    }

    /// Select or deselect the package of `selection` for `users`.
    fn toggle_for_users(
        &mut self,
        users: &[User],
        selection: (usize, usize),
        toggle: bool,
        expert_mode: bool,
    ) {
        for u in users.iter().filter(|u| !u.protected) {
            let Some(i_package) = same_package(&self.phone_packages, selection, u.index) else {
                continue;
            };
            let row = &mut self.phone_packages[u.index][i_package];
            let toggle = toggle && (expert_mode || !needs_expert_mode(row));
            row.selected = toggle;
            if !toggle {
                self.selected_packages
                    .retain(|&x| x != (u.index, i_package));
            } else if !self.selected_packages.contains(&(u.index, i_package)) {
                self.selected_packages.push((u.index, i_package));
            }
        }
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

    fn on_export_selection(&mut self, settings: &Settings) -> Task<Message> {
        let i_user = self.selected_user.unwrap_or_default().index;
        let package_names: Vec<String> = self.phone_packages[i_user]
            .iter()
            .filter(|p| p.selected)
            .map(|p| p.name.clone())
            .collect();
        let dir = settings.general.backup_folder.clone();
        Task::perform(
            async move { export_selection(&dir, &package_names) },
            Message::SelectionExported,
        )
    }

    fn on_selection_exported(&mut self, export: Result<PathBuf, String>) -> Task<Message> {
        match export {
            Ok(path) => self.export_modal = Some(path),
            Err(err) => error!("Failed to export current selection: {err:?}"),
        }
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

/// Removing an Unsafe package needs expert mode. Restoring one does not.
fn needs_expert_mode(package: &PackageRow) -> bool {
    package.removal == Removal::Unsafe && package.state == PackageState::Enabled
}

/// Find the package of `selection` in the list of another user.
///
/// Each user has its own list, and a package can be missing from some of them,
/// so the same package is not always on the same row.
fn same_package(
    packages: &[Vec<PackageRow>],
    selection: (usize, usize),
    i_user: usize,
) -> Option<usize> {
    let name = &packages.get(selection.0)?.get(selection.1)?.name;
    find_package(packages.get(i_user)?, name, selection.1)
}

/// Find a package by name, looking first at the row it is expected on.
fn find_package(packages: &[PackageRow], name: &str, expected: usize) -> Option<usize> {
    if packages.get(expected).is_some_and(|p| p.name == name) {
        return Some(expected);
    }
    packages.iter().position(|p| p.name == name)
}

/// The users the action of `selection` applies to, with the ADB commands to run for each.
fn plan_pkg_action(
    packages: &[Vec<PackageRow>],
    device: &Phone,
    settings: &DeviceSettings,
    expert_mode: bool,
    selection: (usize, usize),
) -> Result<Vec<(PackageInfo, Vec<String>)>, String> {
    let pkg = &packages[selection.0][selection.1];
    if needs_expert_mode(pkg) && !expert_mode {
        return Ok(Vec::new());
    }
    let wanted_state = pkg.state.opposite(settings.disable_mode);

    let mut plan = vec![];
    for u in device.user_list.iter().filter(|u| !u.protected) {
        // Only the initiating user, unless multi-user mode extends the action to every user.
        if u.index != selection.0 && !settings.multi_user_mode {
            continue;
        }
        let Some(index) = same_package(packages, selection, u.index) else {
            continue;
        };
        let u_pkg = &packages[u.index][index];
        if u_pkg.removal == Removal::Unsafe && wanted_state != PackageState::Enabled && !expert_mode
        {
            continue;
        }
        let p_info = PackageInfo {
            i_user: u.index,
            index,
            allow_removal: expert_mode || u_pkg.removal != Removal::Unsafe,
            device: device.clone(),
            user: *u,
            name: u_pkg.name.clone(),
            state: u_pkg.state,
            wanted_state,
            // Will be filled asynchronously before running the adb action
            before_cross_user_states: vec![],
        };
        let actions = apply_pkg_state_commands(&u_pkg.into(), wanted_state, *u, device)?;
        plan.push((p_info, actions));
    }
    // An action changes the package of its own user on purpose. To detect unexpected
    // changes, each one only watches the users that no action is planned for.
    let targeted: Vec<u16> = plan
        .iter()
        .filter(|(_, actions)| !actions.is_empty())
        .map(|(info, _)| info.user.id)
        .collect();
    for (info, _) in &mut plan {
        let own = info.user.id;
        info.device
            .user_list
            .retain(|u| u.id == own || !targeted.contains(&u.id));
    }
    Ok(plan)
}

/// Queue the ADB commands of a planned action.
fn pkg_action_tasks(plan: Vec<(PackageInfo, Vec<String>)>) -> Vec<Task<Message>> {
    plan.into_iter()
        .filter(|(_, actions)| !actions.is_empty())
        .map(|(p_info, actions)| {
            Task::perform(
                async move { run_package_action(&actions, p_info) },
                Message::VerifyAndFallbackFinished,
            )
        })
        .collect()
}

fn run_package_action(actions: &[String], mut info: PackageInfo) -> VerifyAndFallbackResult {
    use uad_core::sync::{capture_cross_user_states, run_adb_shell_action};

    info.before_cross_user_states =
        capture_cross_user_states(&info.name, &info.device.adb_id, info.user.id, &info.device);
    let mut result = VerifyAndFallbackResult {
        info,
        new_state: None,
        ran: false,
        notification: None,
        error_modal: None,
    };
    for (index, command) in actions.iter().enumerate() {
        if let Err(AdbError::Generic(error)) =
            run_adb_shell_action(&result.info.device.adb_id, command)
        {
            if index > 0 {
                add_note(
                    &mut result,
                    format!("Package cleanup incomplete. Failed command: {command}. {error}"),
                );
            } else if !(error.contains("Shell cannot change component state")
                && command.starts_with("pm enable --user "))
            {
                result.error_modal = Some(error);
                return result;
            }
            break;
        }
    }
    result.ran = true;
    if let Err(error) = verify_action(&mut result) {
        // Listed with the other notifications: several packages can fail in one batch.
        let note = format!(
            "Package '{}' (user {}) verification failed: {error}",
            result.info.name, result.info.user.id
        );
        add_note(&mut result, note);
    }
    result
}

fn add_note(result: &mut VerifyAndFallbackResult, note: String) {
    result.notification = Some(match result.notification.take() {
        Some(notes) => format!("{notes}\n{note}"),
        None => note,
    });
}

fn verify_action(result: &mut VerifyAndFallbackResult) -> Result<(), String> {
    use uad_core::sync::{
        CorePackage, detect_cross_user_behavior, query_package_state, run_fallback_commands,
        verify_fallback_state,
    };

    let info = &result.info;
    let read_state = || {
        query_package_state(
            &info.device.adb_id,
            &info.name,
            info.device.package_user_id(info.user.id)?,
        )
        .map(|state| state.unwrap_or(PackageState::Uninstalled))
    };
    let actual = read_state()?;
    result.new_state = Some(actual);
    if actual == info.wanted_state {
        result.error_modal = detect_cross_user_behavior(
            &info.name,
            &info.device.adb_id,
            info.user.id,
            info.wanted_state,
            actual,
            &info.device,
            &info.before_cross_user_states,
        )
        .map(|details| {
            format!(
                "Cross-User Behavior Detected:\n\n{details}\n\n\
                This is unusual behavior that may be specific to your device manufacturer (OEM). \
                The package state has been successfully changed on the target user."
            )
        });
        return Ok(());
    }
    // Recovery starts by removing or disabling the package, unless it is already uninstalled.
    if !info.allow_removal && actual != PackageState::Uninstalled {
        return Err("Recovery would require removing an Unsafe package.".into());
    }
    let pkg = CorePackage {
        name: info.name.clone(),
        description: String::new(),
        removal: Removal::All,
        state: actual,
        list: UadList::Unlisted,
    };
    let fallback = run_fallback_commands(&pkg, info.wanted_state, actual, info.user, &info.device);
    // Even a failed recovery can have changed the package.
    let confirmed = read_state()?;
    result.new_state = Some(confirmed);
    let (expected, action) = fallback?;
    verify_fallback_state(expected, confirmed)?;
    let note = format!(
        "Package '{}' was {} but {} instead. Fallback: {action}",
        info.name,
        match info.wanted_state {
            PackageState::Uninstalled => "uninstalled",
            PackageState::Disabled => "disabled",
            PackageState::Enabled => "enabled",
            PackageState::All => "modified",
        },
        match actual {
            PackageState::Uninstalled => "remains uninstalled",
            PackageState::Disabled => "was disabled",
            PackageState::Enabled => "was enabled",
            PackageState::All => "unexpected state (error determining state)",
        }
    );
    add_note(result, note);
    Ok(())
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

    /// Two users with the same three packages. The owner has the last two selected.
    fn view_with_selection() -> List {
        let packages: Vec<PackageRow> = ["com.example.a", "com.example.b", "com.example.c"]
            .iter()
            .map(|name| {
                PackageRow::new(
                    name,
                    "",
                    Removal::Recommended,
                    PackageState::Enabled,
                    UadList::Misc,
                    false,
                    false,
                )
            })
            .collect();
        let mut view = List {
            selected_user: Some(User::default()),
            selected_list: Some(UadList::All),
            selected_package_state: Some(PackageState::All),
            selected_removal: Some(Removal::All),
            phone_packages: vec![packages.clone(), packages],
            selected_packages: vec![(0, 1), (0, 2)],
            ..List::default()
        };
        view.phone_packages[0][1].selected = true;
        view.phone_packages[0][2].selected = true;
        view
    }

    /// The owner and a work profile.
    fn two_users() -> Phone {
        let work = User {
            id: 10,
            index: 1,
            protected: false,
        };
        Phone {
            android_sdk: 35,
            user_list: vec![User::default(), work],
            ..Phone::default()
        }
    }

    fn settings(multi_user_mode: bool) -> DeviceSettings {
        DeviceSettings {
            multi_user_mode,
            ..DeviceSettings::default()
        }
    }

    /// Result of the action of `selection`, for the user it was started from.
    fn finished(view: &List, selection: (usize, usize)) -> VerifyAndFallbackResult {
        let plan = plan_pkg_action(
            &view.phone_packages,
            &two_users(),
            &settings(false),
            false,
            selection,
        )
        .unwrap();
        VerifyAndFallbackResult {
            info: plan[0].0.clone(),
            new_state: Some(PackageState::Uninstalled),
            ran: true,
            notification: None,
            error_modal: None,
        }
    }

    #[test]
    fn finished_action_keeps_the_other_packages_selected() {
        let mut view = view_with_selection();
        let result = finished(&view, (0, 1));
        let _ = view.on_verify_and_fallback_finished(result, &two_users());
        assert!(!view.phone_packages[0][1].selected);
        assert!(view.phone_packages[0][2].selected);
        assert_eq!(view.selected_packages, [(0, 2)]);
    }

    #[test]
    fn finished_action_updates_the_package_it_was_started_for() {
        let mut view = view_with_selection();
        let result = finished(&view, (0, 1));

        // Another device was selected in the meantime: nothing is updated.
        let other = Phone {
            adb_id: "other".to_string(),
            ..two_users()
        };
        let _ = view.on_verify_and_fallback_finished(result.clone(), &other);
        assert_eq!(view.phone_packages[0][1].state, PackageState::Enabled);
        assert_eq!(view.selected_packages, [(0, 1), (0, 2)]);

        // The list was reloaded without its first package: the rows are shifted.
        view.phone_packages[0].remove(0);
        view.selected_packages = vec![(0, 0), (0, 1)];
        let _ = view.on_verify_and_fallback_finished(result, &two_users());
        let states: Vec<_> = view.phone_packages[0].iter().map(|p| p.state).collect();
        assert_eq!(states, [PackageState::Uninstalled, PackageState::Enabled]);
        assert_eq!(view.selected_packages, [(0, 1)]);
    }

    #[test]
    fn multi_user_mode_selects_the_same_packages_for_every_user() {
        let mut view = view_with_selection();
        view.select_for_all_users(&two_users().user_list, false);
        let selected: Vec<bool> = view.phone_packages[1].iter().map(|p| p.selected).collect();
        assert_eq!(selected, [false, true, true]);
        assert_eq!(view.selected_packages, [(0, 1), (0, 2), (1, 1), (1, 2)]);
    }

    #[test]
    fn unsafe_packages_need_expert_mode_to_be_selected() {
        let mut view = view_with_selection();
        let users = two_users().user_list;
        view.phone_packages[0][0].removal = Removal::Unsafe;
        view.phone_packages[0][1].removal = Removal::Unsafe;

        view.toggle_selection(&users, 0, true, false, false);
        assert_eq!(view.selected_packages, [(0, 1), (0, 2)]);
        view.toggle_selection(&users, 0, true, true, false);
        assert_eq!(view.selected_packages, [(0, 1), (0, 2), (0, 0)]);

        // Without expert mode, an Unsafe package can still be deselected.
        view.toggle_selection(&users, 0, false, false, false);
        assert_eq!(view.selected_packages, [(0, 1), (0, 2)]);
        assert!(!view.phone_packages[0][0].selected);

        // Restoring an Unsafe package does not need expert mode.
        view.phone_packages[0][0].state = PackageState::Uninstalled;
        view.toggle_selection(&users, 0, true, false, false);
        assert_eq!(view.selected_packages, [(0, 1), (0, 2), (0, 0)]);

        // Leaving expert mode deselects the Unsafe packages that would be removed.
        view.deselect_unsafe();
        assert_eq!(view.selected_packages, [(0, 2), (0, 0)]);
        assert!(!view.phone_packages[0][1].selected);
    }

    #[test]
    fn apply_runs_each_package_once_from_the_reviewed_user() {
        let mut view = view_with_selection();
        let phone = two_users();
        view.select_for_all_users(&phone.user_list, false);
        // The second package is already uninstalled for the work profile.
        view.phone_packages[1][1].state = PackageState::Uninstalled;

        // The owner is under review: uninstall the second package for the owner only,
        // and the third one for both users.
        let commands = view.apply_actions(&phone, &settings(true), false, None);
        assert_eq!(commands.len(), 3);
        // A package stays selected until its own action succeeds.
        assert_eq!(view.selected_packages, [(0, 1), (0, 2), (1, 2)]);
        let selected: Vec<bool> = view.phone_packages[1].iter().map(|p| p.selected).collect();
        assert_eq!(selected, [false, false, true]);
    }

    #[test]
    fn every_user_gets_the_action_of_the_user_under_review() {
        let mut view = view_with_selection();
        // The second package is disabled for the work profile.
        view.phone_packages[1][1].state = PackageState::Disabled;

        // Whether it comes from the review or from the button of a row,
        // multi-user mode extends the action to every user.
        let plan: Vec<_> = plan_pkg_action(
            &view.phone_packages,
            &two_users(),
            &settings(true),
            false,
            (0, 1),
        )
        .unwrap()
        .into_iter()
        .map(|(info, commands)| (info.user.id, info.state, info.wanted_state, commands))
        .collect();
        assert_eq!(
            plan,
            [
                (
                    0,
                    PackageState::Enabled,
                    PackageState::Uninstalled,
                    vec!["pm uninstall --user 0 com.example.b".to_string()]
                ),
                (
                    10,
                    PackageState::Disabled,
                    PackageState::Uninstalled,
                    vec!["pm uninstall --user 10 com.example.b".to_string()]
                ),
            ]
        );
    }

    #[test]
    fn packages_are_matched_by_name_across_users() {
        let mut view = view_with_selection();
        let phone = two_users();
        // The work profile does not list the first package: its rows are shifted.
        view.phone_packages[1].remove(0);

        view.select_for_all_users(&phone.user_list, false);
        assert_eq!(view.selected_packages, [(0, 1), (0, 2), (1, 0), (1, 1)]);

        // Deselecting the second package deselects it for the work profile too.
        view.toggle_for_users(&phone.user_list, (0, 1), false, false);
        assert_eq!(view.selected_packages, [(0, 2), (1, 1)]);
        assert!(!view.phone_packages[1][0].selected);

        // The third package is uninstalled for both users.
        let commands = view.apply_actions(&phone, &settings(true), false, None);
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn apply_only_runs_the_selection_under_review() {
        let apply = |multi_user_mode| {
            let mut view = view_with_selection();
            // The work profile has its own selection: the first and second packages.
            for i_package in [0, 1] {
                view.phone_packages[1][i_package].selected = true;
                view.selected_packages.push((1, i_package));
            }
            let commands =
                view.apply_actions(&two_users(), &settings(multi_user_mode), false, None);
            (commands.len(), view.selected_packages)
        };
        let selection = vec![(0, 1), (0, 2), (1, 0), (1, 1)];

        // The owner is under review, with the second and third packages selected.
        assert_eq!(apply(false), (2, selection.clone()));
        // In multi-user mode, they are also uninstalled for the work profile.
        assert_eq!(apply(true), (4, selection));
    }

    #[test]
    fn unreadable_target_blocks_row_and_bulk_actions() {
        let mut view = view_with_selection();
        let mut phone = two_users();
        view.phone_packages[1].clear();
        view.inventory_errors.insert(1, "device offline".into());
        for single in [Some(1), None] {
            assert!(
                view.apply_actions(&phone, &settings(true), false, single)
                    .is_empty()
            );
            let error = view.error_modal.take().unwrap();
            assert!(error.contains("user 10") && error.contains("device offline"));
        }
        assert!(view.pending_actions.is_empty());
        assert_eq!(
            view.apply_actions(&phone, &settings(false), false, None)
                .len(),
            2
        );
        phone.user_list[1].protected = true;
        assert_eq!(
            view.apply_actions(&phone, &settings(true), false, None)
                .len(),
            2
        );
    }

    #[test]
    fn reread_preserves_filters_and_eligible_selections() {
        let mut view = view_with_selection();
        let phone = Phone {
            adb_id: "A".into(),
            ..two_users()
        };
        view.loaded_serial = Some(phone.adb_id.clone());
        view.selected_list = Some(UadList::Misc);
        view.reread_packages = true;
        assert_eq!(view.refresh_if_needed(&phone).units(), 1);

        let mut fresh = view.phone_packages.clone();
        // Multi-user mode was enabled in Settings while the read was in flight.
        view.select_for_all_users(&phone.user_list, false);
        for row in &mut fresh[0] {
            row.selected = false;
        }
        // One selected package moved, and the other now requires expert mode.
        fresh[0].swap(0, 1);
        fresh[0][2].removal = Removal::Unsafe;
        let _ = view.on_apply_filters("A", fresh.into_iter().map(Ok).collect(), &phone, false);
        assert_eq!(view.selected_packages, [(0, 0), (1, 1), (1, 2)]);
        assert!(view.phone_packages[0][0].selected);
        assert!(!view.phone_packages[0][2].selected);
        assert_eq!(view.selected_removal, Some(Removal::All));
        assert_eq!(view.selected_package_state, Some(PackageState::All));
        assert_eq!(view.selected_list, Some(UadList::Misc));
    }

    #[test]
    fn reloading_the_packages_resets_the_selection_and_the_user() {
        let mut view = view_with_selection();
        // The work profile is displayed, and the new device has a single user.
        view.selected_user = Some(two_users().user_list[1]);
        let _ = view.on_apply_filters("", vec![Ok(vec![])], &two_users(), false);
        assert_eq!(view.selected_packages, []);
        assert_eq!(view.selected_user, Some(User::default()));
    }

    #[test]
    fn packages_of_another_device_are_not_displayed() {
        let mut view = view_with_selection();
        // These packages finished loading after another device was selected.
        let _ = view.on_apply_filters("previous-device", vec![Ok(vec![])], &two_users(), false);
        assert_eq!(view.phone_packages[0].len(), 3);
        assert_eq!(view.selected_packages, [(0, 1), (0, 2)]);
    }

    #[test]
    fn review_cannot_turn_an_unsafe_restore_into_a_removal() {
        let mut view = view_with_selection();
        let phone = two_users();
        for rows in &mut view.phone_packages {
            rows[0].removal = Removal::Unsafe;
        }
        view.phone_packages[0][0].state = PackageState::Disabled;
        view.toggle_selection(&phone.user_list, 0, true, false, true);
        view.select_for_all_users(&phone.user_list, false);
        assert!(view.phone_packages[0][0].selected);
        assert!(!view.phone_packages[1][0].selected);
        // Apply also checks the action in case the selection outlived a state change.
        assert!(
            plan_pkg_action(&view.phone_packages, &phone, &settings(true), false, (1, 0))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            view.apply_actions(&phone, &settings(true), false, Some(0))
                .len(),
            1
        );
        view.selected_user = Some(phone.user_list[1]);
        assert_eq!(
            view.apply_actions(&phone, &settings(true), true, Some(0))
                .len(),
            2
        );
    }

    #[test]
    fn failed_actions_are_reported_and_deselected_once_they_ran() {
        let mut view = view_with_selection();
        // The action ran but could not be confirmed.
        let mut unverified = finished(&view, (0, 1));
        unverified.new_state = None;
        unverified.notification = Some("unverified".to_string());
        // ADB refused the action.
        let mut refused = finished(&view, (0, 2));
        refused.new_state = None;
        refused.ran = false;
        refused.error_modal = Some("refused".to_string());

        // Messages are shown even if another device was selected in the meantime.
        let other = Phone {
            adb_id: "other".to_string(),
            ..two_users()
        };
        let _ = view.on_verify_and_fallback_finished(unverified.clone(), &other);
        assert!(!view.reread_packages);
        let _ = view.on_verify_and_fallback_finished(refused, &two_users());
        // A failed command may have changed the phone before ADB lost the reply.
        assert!(view.reread_packages);
        let _ = view.on_verify_and_fallback_finished(unverified, &two_users());

        assert_eq!(view.fallback_notifications, ["unverified", "unverified"]);
        assert_eq!(view.error_modal.as_deref(), Some("refused"));
        assert_eq!(view.phone_packages[0][1].state, PackageState::Enabled);
        assert_eq!(view.selected_packages, [(0, 2)]);

        // They are still listed once the packages of the other device are loaded.
        let _ = view.on_apply_filters("other", vec![Ok(vec![]), Ok(vec![])], &other, false);
        assert_eq!(view.fallback_notifications.len(), 2);
    }

    #[test]
    fn a_late_definitions_update_invalidates_rows_from_the_cache() {
        let mut view = view_with_selection();
        let mut state = UadListState::Downloading;
        let mut settings = Settings::default();
        let mut phone = Phone {
            adb_id: "B".into(),
            ..two_users()
        };
        let _ = view.load_uad_list(true, &phone);
        let id = view.load_id;
        // The selected phone has already loaded rows from the cached definitions.
        view.load_id += 1;
        view.loaded_serial = Some(phone.adb_id.clone());
        let late = Message::LoadPhonePackages(id, (PackageHashMap::new(), UadListState::Done));
        assert_eq!(
            view.update(&mut settings, &mut phone, &mut state, late)
                .units(),
            1
        );
        assert_eq!(state, UadListState::Done);
        assert_eq!(view.selected_packages, [(0, 1), (0, 2)]);
        assert!(!view.inventory_current(&phone));
        let action = Message::List(1, RowMessage::ActionPressed);
        assert_eq!(
            view.update(&mut settings, &mut phone, &mut state, action.clone())
                .units(),
            0
        );

        let mut fresh = view.phone_packages.clone();
        fresh[0][1].removal = Removal::Unsafe;
        for row in fresh.iter_mut().flatten() {
            row.selected = false;
        }
        let _ = view.on_apply_filters("B", fresh.into_iter().map(Ok).collect(), &phone, false);
        assert_eq!(view.selected_packages, [(0, 2)]);
        assert_eq!(
            view.update(&mut settings, &mut phone, &mut state, action)
                .units(),
            0
        );

        // An older download ending after it does not replace it.
        let older = Message::LoadPhonePackages(
            view.load_id + 1,
            (PackageHashMap::new(), UadListState::Failed),
        );
        let _ = view.update(&mut settings, &mut two_users(), &mut state, older);
        assert_eq!(state, UadListState::Done);
    }

    #[test]
    fn actions_only_watch_the_users_they_leave_alone() {
        let view = view_with_selection();
        let watched = |multi_user_mode| -> Vec<Vec<u16>> {
            plan_pkg_action(
                &view.phone_packages,
                &two_users(),
                &settings(multi_user_mode),
                false,
                (0, 1),
            )
            .unwrap()
            .iter()
            .map(|(info, _)| info.device.user_list.iter().map(|u| u.id).collect())
            .collect()
        };
        // The owner's action must notice a change for the work profile.
        assert_eq!(watched(false), [[0, 10]]);
        // Both are uninstalled on purpose: neither is a surprise for the other.
        assert_eq!(watched(true), [vec![0], vec![10]]);
    }

    #[test]
    fn discovery_keeps_action_messages_received_since_refresh_started() {
        let mut view = view_with_selection();
        let phone = two_users();
        let mut action = finished(&view, (0, 1));
        action.notification = Some("Retry failed".into());
        view.fallback_notifications.push("Old warning".into());

        view.reset_inventory();
        let _ = view.on_verify_and_fallback_finished(action, &phone);
        let _ = view.load_uad_list(true, &phone);
        let _ = view.on_apply_filters("", vec![Ok(vec![])], &phone, false);
        assert_eq!(view.fallback_notifications, ["Retry failed"]);
    }

    #[test]
    fn an_obsolete_read_waits_for_the_rest_of_the_batch() {
        let mut view = view_with_selection();
        let phone = Phone {
            adb_id: "A".into(),
            ..two_users()
        };
        let stale: Vec<_> = view.phone_packages.clone().into_iter().map(Ok).collect();
        view.pending_actions.insert(phone.adb_id.clone(), 2);
        for index in [1, 2] {
            let mut result = finished(&view, (0, index));
            result.info.device = phone.clone();
            let task = view.on_verify_and_fallback_finished(result, &phone);
            if index == 1 {
                assert_eq!(task.units(), 0);
                assert_eq!(
                    view.on_apply_filters("A", stale.clone(), &phone, false)
                        .units(),
                    0
                );
                assert_eq!(view.phone_packages[0][1].state, PackageState::Uninstalled);
                assert!(!view.inventory_current(&phone));
            } else {
                assert_eq!(task.units(), 1);
            }
        }
    }

    #[test]
    fn packages_are_read_again_when_an_action_finished_during_their_load() {
        let mut view = view_with_selection();
        let mut state = UadListState::Done;
        let mut settings = Settings::default();
        let mut phone = two_users();
        let action = finished(&view, (0, 1));
        let read_before_the_action: Vec<_> =
            view.phone_packages.clone().into_iter().map(Ok).collect();

        // The device is refreshed, and the action finishes while its packages are read.
        view.reset_inventory();
        let load =
            Message::LoadPhonePackages(view.load_id, (PackageHashMap::new(), UadListState::Done));
        let _ = view.update(&mut settings, &mut phone, &mut state, load);
        let _ = view.on_verify_and_fallback_finished(action, &phone);

        // What was read is dropped for a new read.
        let mut loaded = |packages| {
            let message = Message::ApplyFilters(view.load_id, String::new(), packages);
            let task = view.update(&mut settings, &mut phone, &mut state, message);
            (task.units(), view.phone_packages.len())
        };
        assert_eq!(loaded(read_before_the_action.clone()), (1, 0));
        assert_eq!(loaded(read_before_the_action), (0, 2));
    }
}

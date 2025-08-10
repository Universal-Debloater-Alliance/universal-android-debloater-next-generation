use crate::core::config::DeviceSettings;
use crate::core::helpers::button_primary;
use crate::core::sync::{AdbError, Phone, User, adb_shell_command, apply_pkg_state_commands};
use crate::core::theme::Theme;
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
use iced::{Alignment, Command, Element, Length, Renderer, alignment};

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
    /// packages of all users of the phone
    pub phone_packages: Vec<Vec<PackageRow>>,
    /// `phone_packages` indexes of the selected user (= what you see on screen)
    filtered_packages: Vec<usize>,
    /// Vec of `(user_index, pkg_index)`
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
    #[allow(clippy::too_many_lines)]
    pub fn update(
        &mut self,
        settings: &mut Settings,
        selected_device: &mut Phone,
        list_update_state: &mut UadListState,
        message: Message,
    ) -> Command<Message> {
        let i_user = self.selected_user.unwrap_or_default().index;
        match message {
            Message::ModalHide => {
                self.selection_modal = false;
                self.error_modal = None;
                self.export_modal = false;
                Command::none()
            }
            Message::ModalValidate => {
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
                Command::batch(commands)
            }
            Message::RestoringDevice(output) => {
                if let Ok(p) = output {
                    self.loading_state = LoadingState::RestoringDevice(
                        self.phone_packages[i_user][p.index].name.clone(),
                    );
                } else {
                    self.loading_state = LoadingState::RestoringDevice("Error [TODO]".to_string());
                }
                Command::none()
            }
            Message::LoadUadList(remote) => {
                info!("{:-^65}", "-");
                info!(
                    "ANDROID_SDK: {} | DEVICE: {}",
                    selected_device.android_sdk, selected_device.model
                );
                info!("{:-^65}", "-");
                self.loading_state = LoadingState::DownloadingList;
                Command::perform(
                    Self::init_apps_view(remote, selected_device.clone()),
                    Message::LoadPhonePackages,
                )
            }
            Message::LoadPhonePackages((uad_list, list_state)) => {
                self.loading_state = LoadingState::LoadingPackages;
                self.uad_lists.clone_from(&uad_list);
                *list_update_state = list_state;
                Command::perform(
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
                Command::none()
            }
            Message::ToggleAllSelected(selected) => {
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
                Command::none()
            }
            Message::SearchInputChanged(letter) => {
                self.input_value = letter;
                Self::filter_package_lists(self);
                Command::none()
            }
            Message::ListSelected(list) => {
                self.selected_list = Some(list);
                Self::filter_package_lists(self);
                Command::none()
            }
            Message::PackageStateSelected(package_state) => {
                self.selected_package_state = Some(package_state);
                Self::filter_package_lists(self);
                Command::none()
            }
            Message::RemovalSelected(removal) => {
                self.selected_removal = Some(removal);
                Self::filter_package_lists(self);
                Command::none()
            }
            Message::List(i_package, row_message) => {
                #[expect(unused_must_use, reason = "side-effect")]
                {
                    self.phone_packages[i_user][i_package]
                        .update(&row_message)
                        .map(move |row_message| Message::List(i_package, row_message));
                }

                let package = &mut self.phone_packages[i_user][i_package];

                match row_message {
                    RowMessage::ToggleSelection(toggle) => {
                        if package.removal == Removal::Unsafe && !settings.general.expert_mode {
                            package.selected = false;
                            return Command::none();
                        }

                        if settings.device.multi_user_mode {
                            for u in selected_device.user_list.iter().filter(|&u| !u.protected) {
                                if let Some(pkg) = self
                                    .phone_packages
                                    .get_mut(u.index)
                                    .and_then(|pkgs| pkgs.get_mut(i_package))
                                {
                                    pkg.selected = toggle;
                                    if toggle
                                        && !self.selected_packages.contains(&(u.index, i_package))
                                    {
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
                        Command::none()
                    }
                    RowMessage::ActionPressed => {
                        self.phone_packages[i_user][i_package].selected = true;
                        Command::batch(build_action_pkg_commands(
                            &self.phone_packages,
                            selected_device,
                            &settings.device,
                            (i_user, i_package),
                        ))
                    }
                    RowMessage::PackagePressed => {
                        self.description = package.clone().description;
                        self.description_content =
                            text_editor::Content::with_text(&package.description);
                        package.current = true;
                        if self.current_package_index != i_package {
                            self.phone_packages[i_user][self.current_package_index].current = false;
                        }
                        self.current_package_index = i_package;
                        Command::none()
                    }
                }
            }
            Message::ApplyActionOnSelection => {
                self.selection_modal = true;
                Command::none()
            }
            Message::UserSelected(user) => {
                self.selected_user = Some(user);
                self.filtered_packages = (0..self.phone_packages[user.index].len()).collect();
                Self::filter_package_lists(self);
                Command::none()
            }
            Message::ChangePackageState(res) => {
                match res {
                    Ok(p) => {
                        let package = &mut self.phone_packages[p.i_user][p.index];
                        package.state = package.state.opposite(settings.device.disable_mode);
                        package.selected = false;
                        self.selected_packages
                            .retain(|&x| x.1 != p.index && x.0 != p.i_user);
                        Self::filter_package_lists(self);
                    }
                    Err(AdbError::Generic(err)) => {
                        self.error_modal = Some(err);
                    }
                }
                Command::none()
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
                Command::none()
            }
            Message::ADBSatisfied(result) => {
                self.is_adb_satisfied = result;
                Command::none()
            }
            Message::UpdateFailed => {
                self.loading_state = LoadingState::FailedToUpdate;
                Command::none()
            }
            Message::GoToUrl(url) => {
                open_url(url);
                Command::none()
            }
            Message::ExportSelection => Command::perform(
                export_selection(self.phone_packages[i_user].clone()),
                Message::SelectionExported,
            ),
            Message::SelectionExported(export) => {
                match export {
                    Ok(_) => self.export_modal = true,
                    Err(err) => error!("Failed to export current selection: {err:?}"),
                }
                Command::none()
            }
            Message::Nothing => Command::none(),
            Message::DescriptionEdit(action) => {
                match action {
                    text_editor::Action::Edit(_) => {
                        // Do nothing - ignore all editing operations
                    }
                    text_editor::Action::Scroll { lines } => {}
                    // Allow all other actions (movement, selection, clicking, scrolling, etc.)
                    _ => {
                        self.description_content.perform(action);
                    }
                }
                Command::none()
            }
            Message::CopyError(err) => {
                self.copy_confirmation = true;
                Command::batch(vec![
                    iced::clipboard::write::<Message>(err),
                    Command::perform(Self::delay_hide_copy_confirmation(), |()| {
                        Message::HideCopyConfirmation
                    }),
                ])
            }
            Message::HideCopyConfirmation => {
                self.copy_confirmation = false;
                Command::none()
            }
        }
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

        let select_all_checkbox = checkbox("", self.all_selected)
            .on_toggle(Message::ToggleAllSelected)
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
        .align_items(Alignment::Center)
        .spacing(6)
        .padding([0, 16, 0, 0])
        .into()
    }

    #[allow(clippy::too_many_lines)]
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

        let description_panel = container(description_scroll)
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
            Space::new(Length::Fill, Length::Shrink),
            review_selection
        ]
        .width(Length::Fill)
        .spacing(10)
        .align_items(Alignment::Center);

        let unavailable = container(
                    column![
                        text("ADB is not authorized to access this user!").size(20)
                            .style(style::Text::Danger),
                        text("The most likely reason is that it is the user of your work profile (also called Secure Folder on Samsung devices). There's really no solution, other than completely disabling your work profile in your device settings.")
                            .style(style::Text::Commentary)
                            .horizontal_alignment(alignment::Horizontal::Center),
                    ]
                    .spacing(6)
                    .align_items(Alignment::Center)
                )
                .padding(10)
                .center_x()
                .style(style::Container::BorderedFrame);

        let control_panel = self.control_panel(selected_device);
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
                packages_scrollable,
                description_panel,
                action_row,
            ]
        } else {
            column![
                control_panel,
                container(unavailable).height(Length::Fill).center_y(),
            ]
        }
        .width(Length::Fill)
        .spacing(10)
        .align_items(Alignment::Center);

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
            let title = container(row![text("Success").size(24)].align_items(Alignment::Center))
                .width(Length::Fill)
                .style(style::Container::Frame)
                .padding([10, 0, 10, 0])
                .center_y()
                .center_x();

            let text_box = row![
                text(format!("Exported current selection into file.\nFile is exported in same directory where {NAME} is located.")).width(Length::Fill),
            ].padding(20);

            let file_row = row![text(EXPORT_FILE_NAME).style(style::Text::Commentary)].padding(20);

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

    #[allow(clippy::too_many_lines)]
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
            container(row![text("Review your selection").size(24)].align_items(Alignment::Center))
                .width(Length::Fill)
                .style(style::Container::Frame)
                .padding([10, 0, 10, 0])
                .center_y()
                .center_x();

        let users_ctn = container(radio_btn_users)
            .padding(10)
            .center_x()
            .style(style::Container::Frame);

        let explaination_ctn = container(
            row![
                text("The action for the selected user will be applied to all other users")
                    .style(style::Text::Danger),
                tooltip(
                    text("\u{EA0C}")
                        .font(ICONS)
                        .width(22)
                        .horizontal_alignment(alignment::Horizontal::Center)
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
        .center_x()
        .padding(10)
        .style(style::Container::BorderedFrame);

        let modal_btn_row = row![
            button(text("Cancel")).on_press(Message::ModalHide),
            horizontal_space(),
            button(text("Apply")).on_press(Message::ModalValidate),
        ]
        .padding([0, 15, 10, 10]);

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
                                    column![].spacing(6).width(Length::Fill),
                                    |col, selection| {
                                        col.push(
                                            row![
                                                row![text(
                                                    self.phone_packages[selection.0][selection.1]
                                                        .removal
                                                )]
                                                .width(120),
                                                row![text(
                                                    self.phone_packages[selection.0][selection.1]
                                                        .uad_list
                                                )]
                                                .width(50),
                                                row![text(
                                                    self.phone_packages[selection.0][selection.1]
                                                        .name
                                                        .clone()
                                                ),]
                                                .width(540),
                                                horizontal_space(),
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
                                            .width(Length::Fill)
                                            .spacing(20),
                                        )
                                    },
                                )
                        } else {
                            column![text("No packages selected for this user")]
                                .align_items(Alignment::Center)
                                .width(Length::Fill)
                        },
                    )
                    .padding(10)
                    .width(Length::Fill),
                )
                .style(style::Scrollable::Description),
            )
            .width(Length::Fill)
            .style(style::Container::Frame),
        )
        .width(Length::Fill)
        .max_height(150)
        .padding([0, 10, 0, 10]);

        container(
            if device.user_list.iter().filter(|&u| !u.protected).count() > 1
                && settings.device.multi_user_mode
            {
                column![
                    title_ctn,
                    users_ctn,
                    row![explaination_ctn].padding([0, 10, 0, 10]),
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_items(Alignment::Center)
            } else if !settings.device.multi_user_mode {
                column![
                    title_ctn,
                    users_ctn,
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_items(Alignment::Center)
            } else {
                column![
                    title_ctn,
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_items(Alignment::Center)
            },
        )
        .width(900)
        .height(Length::Shrink)
        .max_height(700)
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
    #[expect(clippy::unused_async, reason = "1 call-site")]
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

    async fn delay_hide_copy_confirmation() {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn error_view<'a>(
    error: &'a str,
    content: Column<'a, Message, Theme, Renderer>,
    copy_confirmation: bool,
) -> Modal<'a, Message, Theme, Renderer> {
    let title_ctn = container(
        row![text("Failed to perform ADB operation").size(24)].align_items(Alignment::Center),
    )
    .width(Length::Fill)
    .style(style::Container::Frame)
    .padding([10, 0, 10, 0])
    .center_y()
    .center_x();

    let modal_btn_row = row![
        button(
            text(if copy_confirmation {
                "Copied!"
            } else {
                "Copy error"
            })
            .width(Length::Fill)
            .horizontal_alignment(alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .on_press_maybe(if copy_confirmation {
            None
        } else {
            Some(Message::CopyError(error.to_string()))
        })
        .style(if copy_confirmation {
            style::Button::Primary
        } else {
            style::Button::default()
        }),
        button(
            text("Close")
                .width(Length::Fill)
                .horizontal_alignment(alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .on_press(Message::ModalHide)
    ]
    .padding([10, 0, 0, 0]);

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
    text_style: style::Text,
) -> Element<'a, Message, Theme, Renderer> {
    let col = column![]
        .spacing(10)
        .align_items(Alignment::Center)
        .push(text(displayed_text.to_string()).style(text_style).size(20));

    let col = match btn {
        Some(btn) => col.push(btn.style(style::Button::Primary).padding([5, 10])),
        None => col,
    };

    container(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y()
        .center_x()
        .style(style::Container::default())
        .into()
}

fn build_action_pkg_commands(
    packages: &[Vec<PackageRow>],
    device: &Phone,
    settings: &DeviceSettings,
    selection: (usize, usize),
) -> Vec<Command<Message>> {
    let pkg = &packages[selection.0][selection.1];
    let wanted_state = pkg.state.opposite(settings.disable_mode);

    let mut commands = vec![];
    for u in device.user_list.iter().filter(|&&u| {
        !u.protected
            && packages
                .get(u.index)
                .and_then(|user_pkgs| user_pkgs.get(selection.1))
                .is_some_and(|pkg| pkg.selected || settings.multi_user_mode)
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
            };
            // In the end there is only one package state change
            // even if we run multiple adb commands
            commands.push(Command::perform(
                adb_shell_command(
                    // this is typically small,
                    // so it's fine.
                    device.adb_id.clone(),
                    action,
                    p_info,
                ),
                if j == 0 {
                    Message::ChangePackageState
                } else {
                    |_| Message::Nothing
                },
            ));
        }
    }
    commands
}

fn recap<'a>(settings: &Settings, recap: &SummaryEntry) -> Element<'a, Message, Theme, Renderer> {
    container(
        row![
            text(recap.category).size(19).width(Length::FillPortion(1)),
            vertical_rule(5),
            row![
                if settings.device.disable_mode {
                    text("Disable").style(style::Text::Danger)
                } else {
                    text("Uninstall").style(style::Text::Danger)
                },
                horizontal_space(),
                text(recap.discard.to_string()).style(style::Text::Danger)
            ]
            .width(Length::FillPortion(1)),
            vertical_rule(5),
            row![
                if settings.device.disable_mode {
                    text("Enable").style(style::Text::Ok)
                } else {
                    text("Restore").style(style::Text::Ok)
                },
                horizontal_space(),
                text(recap.restore.to_string()).style(style::Text::Ok)
            ]
            .width(Length::FillPortion(1))
        ]
        .spacing(20)
        .padding([0, 10, 0, 0])
        .width(Length::Fill)
        .align_items(Alignment::Center),
    )
    .padding(10)
    .width(Length::Fill)
    .height(45)
    .style(style::Container::Frame)
    .into()
}

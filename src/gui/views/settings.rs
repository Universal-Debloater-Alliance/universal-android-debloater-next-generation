use crate::core::{
    config::{BackupSettings, Config, DeviceSettings, GeneralSettings},
    helpers::button_primary,
    save::{backup_phone, list_available_backup_user, list_available_backups, restore_backup},
    sync::{AdbError, Phone, User, get_android_sdk, run_adb_action, supports_multi_user},
    theme::Theme,
    utils::{
        DisplayablePath, Error, NAME, export_packages, generate_backup_name, open_folder, open_url,
        string_to_theme,
    },
};
use crate::gui::{
    style,
    views::list::{List as AppsView, PackageInfo},
    widgets::modal::Modal,
    widgets::navigation_menu::ICONS,
    widgets::package_row::PackageRow,
    widgets::text,
};
use iced::widget::{Space, button, checkbox, column, container, pick_list, radio, row, scrollable};
use iced::{Alignment, Element, Length, Renderer, Task, alignment};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum PopUpModal {
    ExportUninstalled,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub general: GeneralSettings,
    pub device: DeviceSettings,
    is_loading: bool,
    modal: Option<PopUpModal>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            general: Config::load_configuration_file().general,
            device: DeviceSettings::default(),
            is_loading: false,
            modal: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadDeviceSettings,
    ExpertMode(bool),
    DisableMode(bool),
    MultiUserMode(bool),
    ApplyTheme(Theme),
    UrlPressed(PathBuf),
    BackupSelected(DisplayablePath),
    BackupDevice,
    RestoreDevice,
    RestoringDevice(Result<PackageInfo, AdbError>),
    DeviceBackedUp(Result<bool, String>),
    ChooseBackUpFolder,
    FolderChosen(Result<PathBuf, Error>),
    ExportPackages,
    PackagesExported(Result<bool, String>),
    ModalHide,
}

impl Settings {
    pub fn update(
        &mut self,
        phone: &Phone,
        packages: &[Vec<PackageRow>],
        nb_running_async_adb_commands: &mut u32,
        msg: Message,
        selected_user: Option<User>,
    ) -> Task<Message> {
        match msg {
            Message::ModalHide => self.handle_modal_hide(),
            Message::ExpertMode(toggled) => self.handle_expert_mode(phone, toggled),
            Message::DisableMode(toggled) => self.handle_disable_mode(phone, toggled),
            Message::MultiUserMode(toggled) => self.handle_multi_user_mode(phone, toggled),
            Message::ApplyTheme(theme) => self.handle_apply_theme(phone, theme),
            Message::UrlPressed(url) => Self::handle_url_pressed(url),
            Message::LoadDeviceSettings => self.handle_load_device_settings(phone),
            Message::BackupSelected(d_path) => self.handle_backup_selected(d_path),
            Message::BackupDevice => self.handle_backup_device(phone, packages),
            Message::DeviceBackedUp(result) => self.handle_device_backed_up(phone, result),
            Message::RestoreDevice => {
                self.handle_restore_device(phone, packages, nb_running_async_adb_commands)
            }
            Message::RestoringDevice(_) => Task::none(),
            Message::FolderChosen(result) => self.handle_folder_chosen(phone, result),
            Message::ChooseBackUpFolder => self.handle_choose_backup_folder(),
            Message::ExportPackages => Self::handle_export_packages(selected_user, packages),
            Message::PackagesExported(result) => self.handle_packages_exported(result),
        }
    }

    fn handle_modal_hide(&mut self) -> Task<Message> {
        self.modal = None;
        Task::none()
    }

    fn handle_expert_mode(&mut self, phone: &Phone, toggled: bool) -> Task<Message> {
        self.general.expert_mode = toggled;
        debug!("Config change: {self:?}");
        Config::save_changes(self, &phone.adb_id);
        Task::none()
    }

    fn handle_disable_mode(&mut self, phone: &Phone, toggled: bool) -> Task<Message> {
        if phone.android_sdk >= 23 {
            self.device.disable_mode = toggled;
            debug!("Config change: {self:?}");
            Config::save_changes(self, &phone.adb_id);
        }
        Task::none()
    }

    fn handle_multi_user_mode(&mut self, phone: &Phone, toggled: bool) -> Task<Message> {
        self.device.multi_user_mode = toggled;
        debug!("Config change: {self:?}");
        Config::save_changes(self, &phone.adb_id);
        Task::none()
    }

    fn handle_apply_theme(&mut self, phone: &Phone, theme: Theme) -> Task<Message> {
        self.general.theme = theme.to_string();
        debug!("Config change: {self:?}");
        Config::save_changes(self, &phone.adb_id);
        Task::none()
    }

    fn handle_url_pressed(url: PathBuf) -> Task<Message> {
        open_url(url);
        Task::none()
    }

    fn handle_load_device_settings(&mut self, phone: &Phone) -> Task<Message> {
        self.load_device_settings(phone);
        Task::none()
    }

    fn load_device_settings(&mut self, phone: &Phone) {
        let backups = list_available_backups(&self.general.backup_folder.join(&phone.adb_id));
        let backup = BackupSettings {
            backups: backups.clone(),
            selected: backups.first().cloned(),
            users: phone.user_list.clone(),
            selected_user: phone.user_list.first().copied(),
            backup_state: String::default(),
        };

        match Config::load_configuration_file()
            .devices
            .iter()
            .find(|d| d.device_id == phone.adb_id)
        {
            Some(device) => {
                self.device.clone_from(device);
                self.device.backup = backup;
            }
            None => {
                self.device = DeviceSettings {
                    device_id: phone.adb_id.clone(),
                    multi_user_mode: supports_multi_user(phone),
                    disable_mode: false,
                    backup,
                };
            }
        }
    }

    fn handle_backup_selected(&mut self, d_path: DisplayablePath) -> Task<Message> {
        self.device.backup.selected = Some(d_path.clone());
        self.device.backup.users = list_available_backup_user(d_path);
        Task::none()
    }

    fn handle_backup_device(
        &mut self,
        phone: &Phone,
        packages: &[Vec<PackageRow>],
    ) -> Task<Message> {
        Task::perform(
            backup_phone(
                phone.user_list.clone(),
                self.device.device_id.clone(),
                packages.to_vec(),
            ),
            Message::DeviceBackedUp,
        )
    }

    fn handle_device_backed_up(
        &mut self,
        phone: &Phone,
        result: Result<bool, String>,
    ) -> Task<Message> {
        match result {
            Ok(_) => {
                info!("[BACKUP] Backup successfully created");
                self.device.backup.backups =
                    list_available_backups(&self.general.backup_folder.join(phone.adb_id.clone()));
                self.device.backup.selected = self.device.backup.backups.first().cloned();
            }
            Err(err) => {
                error!("[BACKUP FAILED] Backup creation failed: {err:?}");
            }
        }
        Task::none()
    }

    fn handle_restore_device(
        &mut self,
        phone: &Phone,
        packages: &[Vec<PackageRow>],
        nb_running_async_adb_commands: &mut u32,
    ) -> Task<Message> {
        match restore_backup(phone, packages, &self.device) {
            Ok(restore_result) => {
                let mut commands = vec![];
                *nb_running_async_adb_commands = 0;
                for p in &restore_result.packages {
                    let p_info = PackageInfo {
                        i_user: p.i_user,
                        index: p.index,
                        removal: "RESTORE".to_string(),
                        before_cross_user_states: vec![],
                    };
                    for command in p.commands.clone() {
                        *nb_running_async_adb_commands += 1;
                        commands.push(Task::perform(
                            run_adb_action(phone.adb_id.clone(), command, p_info.clone()),
                            Message::RestoringDevice,
                        ));
                    }
                }
                if restore_result.skipped_count > 0 {
                    self.device.backup.backup_state = format!(
                        "Restore completed with {} packages skipped (not found on device)",
                        restore_result.skipped_count
                    );
                } else if restore_result.packages.is_empty() {
                    if get_android_sdk(&phone.adb_id) == 0 {
                        self.device.backup.backup_state = "Device is not connected".to_string();
                    } else {
                        self.device.backup.backup_state =
                            "Device state is already restored".to_string();
                    }
                } else {
                    self.device.backup.backup_state = "Restore completed successfully".to_string();
                }
                info!(
                    "[RESTORE] Restoring backup {}",
                    self.device.backup.selected.as_ref().unwrap()
                );
                Task::batch(commands)
            }
            Err(e) => {
                self.device.backup.backup_state.clone_from(&e);
                error!("{} - {}", self.device.backup.selected.as_ref().unwrap(), e);
                Task::none()
            }
        }
    }

    fn handle_folder_chosen(
        &mut self,
        phone: &Phone,
        result: Result<PathBuf, Error>,
    ) -> Task<Message> {
        self.is_loading = false;

        if let Ok(path) = result {
            self.general.backup_folder = path;
            Config::save_changes(self, &phone.adb_id);
            self.load_device_settings(phone);
        }
        Task::none()
    }

    fn handle_choose_backup_folder(&mut self) -> Task<Message> {
        if self.is_loading {
            Task::none()
        } else {
            self.is_loading = true;
            Task::perform(open_folder(), Message::FolderChosen)
        }
    }

    fn handle_export_packages(
        selected_user: Option<User>,
        packages: &[Vec<PackageRow>],
    ) -> Task<Message> {
        Task::perform(
            export_packages(selected_user.unwrap_or_default(), packages.to_vec()),
            Message::PackagesExported,
        )
    }

    fn handle_packages_exported(&mut self, result: Result<bool, String>) -> Task<Message> {
        match result {
            Ok(_) => self.modal = Some(PopUpModal::ExportUninstalled),
            Err(err) => error!("Failed to export list of uninstalled packages: {err:?}"),
        }
        Task::none()
    }

    pub fn view(
        &self,
        phone: &Phone,
        apps_view: &AppsView,
    ) -> Element<'_, Message, Theme, Renderer> {
        let content = if phone.adb_id.is_empty() {
            self.build_no_device_content()
        } else {
            self.build_device_content(phone, apps_view)
        };

        if let Some(PopUpModal::ExportUninstalled) = self.modal {
            return Self::render_export_modal(content);
        }

        container(scrollable(content))
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn build_no_device_content(&self) -> Element<'_, Message, Theme, Renderer> {
        column![
            text("Theme").size(26),
            self.theme_container(),
            text("General").size(26),
            self.general_container(),
            text("Current device").size(26),
            Self::no_device_container(),
            text("Backup / Restore").size(26),
            Self::no_device_container(),
        ]
        .width(Length::Fill)
        .spacing(20)
        .into()
    }

    fn build_device_content(
        &self,
        phone: &Phone,
        apps_view: &AppsView,
    ) -> Element<'_, Message, Theme, Renderer> {
        column![
            text("Theme").size(26),
            self.theme_container(),
            text("General").size(26),
            self.general_container(),
            text("Current device").size(26),
            Self::warning_container(phone),
            self.device_specific_container(phone),
            text("Backup / Restore").size(26),
            self.backup_restore_container(phone, apps_view),
        ]
        .width(Length::Fill)
        .spacing(20)
        .into()
    }

    fn theme_container(&self) -> Element<'_, Message, Theme, Renderer> {
        let radio_btn_theme = Theme::ALL
            .iter()
            .fold(row![].spacing(10), |column, option| {
                column.push(
                    radio(
                        format!("{}", option.clone()),
                        *option,
                        Some(string_to_theme(&self.general.theme)),
                        Message::ApplyTheme,
                    )
                    .size(24),
                )
            });

        container(radio_btn_theme)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(style::Container::Frame)
            .into()
    }

    fn general_container(&self) -> Element<'_, Message, Theme, Renderer> {
        let expert_mode_checkbox = checkbox(
            "Allow to uninstall packages marked as \"unsafe\" (I KNOW WHAT I AM DOING)",
            self.general.expert_mode,
        )
        .on_toggle(Message::ExpertMode)
        .size(20)
        .style(style::CheckBox::SettingsEnabled);

        let expert_mode_descr =
            text("Most unsafe packages are known to bootloop the device if removed.")
                .style(style::Text::Commentary);

        let choose_backup_descr = text("Note: If you have previous backups, you will need to transfer them manually to newly changed backup folder to be able to use Restore functionality")
            .style(style::Text::Commentary);

        let choose_backup_btn = button(text("\u{E930}").font(ICONS))
            .padding([5, 10])
            .on_press(Message::ChooseBackUpFolder)
            .style(style::Button::Primary);

        let choose_backup_row = row![
            choose_backup_btn,
            "Choose backup folder",
            Space::new(Length::Fill, Length::Shrink),
            "Current folder: ",
            text(self.general.backup_folder.to_string_lossy())
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        container(
            column![
                expert_mode_checkbox,
                expert_mode_descr,
                choose_backup_row,
                choose_backup_descr,
            ]
            .spacing(10),
        )
        .padding(10)
        .width(Length::Fill)
        .height(Length::Shrink)
        .style(style::Container::Frame)
        .into()
    }

    fn warning_container(phone: &Phone) -> Element<'static, Message, Theme, Renderer> {
        container(
            row![
                text("The following settings only affect the currently selected device:")
                    .style(style::Text::Danger),
                text(phone.model.clone()),
                Space::new(Length::Fill, Length::Shrink),
                text(phone.adb_id.clone()).style(style::Text::Commentary)
            ]
            .spacing(7),
        )
        .padding(10)
        .width(Length::Fill)
        .style(style::Container::BorderedFrame)
        .into()
    }

    fn device_specific_container(&self, phone: &Phone) -> Element<'_, Message, Theme, Renderer> {
        let multi_user_mode_descr = row![
            text("This will not affect the following protected work profile users: ")
                .style(style::Text::Commentary),
            text(
                phone
                    .user_list
                    .iter()
                    .filter(|&u| u.protected)
                    .map(|u| u.id.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            )
            .style(style::Text::Danger)
        ];

        let multi_user_mode_checkbox = checkbox(
            "Affect all the users of the device (not only the selected user)",
            self.device.multi_user_mode,
        )
        .on_toggle(Message::MultiUserMode)
        .size(20)
        .style(style::CheckBox::SettingsEnabled);

        let disable_checkbox_style = if phone.android_sdk >= 23 {
            style::CheckBox::SettingsEnabled
        } else {
            style::CheckBox::SettingsDisabled
        };

        let disable_mode_descr =
            text("In some cases, it can be better to disable a package instead of uninstalling it")
                .style(style::Text::Commentary);

        let unavailable_btn = button(text("Unavailable").size(14))
            .on_press(Message::UrlPressed(PathBuf::from(
                "https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki/FAQ#\
                    why-is-the-disable-mode-setting-not-available-for-my-device",
            )))
            .height(22)
            .style(style::Button::Unavailable);

        let disable_mode_checkbox = checkbox(
            "Clear and disable packages instead of uninstalling them",
            self.device.disable_mode,
        )
        .on_toggle(Message::DisableMode)
        .size(20)
        .style(disable_checkbox_style);

        let disable_setting_row = if phone.android_sdk >= 23 {
            row![
                disable_mode_checkbox,
                Space::new(Length::Fill, Length::Shrink),
            ]
            .width(Length::Fill)
        } else {
            row![
                disable_mode_checkbox,
                Space::new(Length::Fill, Length::Shrink),
                unavailable_btn,
            ]
            .width(Length::Fill)
        };

        container(
            column![
                multi_user_mode_checkbox,
                multi_user_mode_descr,
                disable_setting_row,
                disable_mode_descr,
            ]
            .spacing(10),
        )
        .padding(10)
        .width(Length::Fill)
        .height(Length::Shrink)
        .style(style::Container::Frame)
        .into()
    }

    fn backup_restore_container(
        &self,
        phone: &Phone,
        apps_view: &AppsView,
    ) -> Element<'_, Message, Theme, Renderer> {
        let backup_pick_list = pick_list(
            self.device.backup.backups.clone(),
            self.device.backup.selected.clone(),
            Message::BackupSelected,
        )
        .padding(6);

        let backup_btn = button_primary(text("Backup").align_x(alignment::Horizontal::Center))
            .on_press(Message::BackupDevice)
            .width(77);

        let restore_btn = |enabled| {
            if enabled {
                button(text("Restore").align_x(alignment::Horizontal::Center))
                    .padding([5, 10])
                    .on_press(Message::RestoreDevice)
                    .width(77)
            } else {
                button(
                    text("No backup")
                        .align_x(alignment::Horizontal::Center)
                        .align_y(alignment::Vertical::Center),
                )
                .padding([5, 10])
                .width(77)
            }
        };

        let locate_backup_btn = if self.device.backup.backups.is_empty() {
            button_primary("Open backup directory")
        } else {
            button_primary("Open backup directory").on_press(Message::UrlPressed(
                self.general.backup_folder.join(phone.adb_id.clone()),
            ))
        };

        let export_btn = button_primary("Export").on_press(Message::ExportPackages);

        let backup_row = row![
            backup_btn,
            "Backup the current state of the phone",
            Space::new(Length::Fill, Length::Shrink),
            locate_backup_btn,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let restore_row = if self.device.backup.backups.is_empty() {
            row![]
        } else {
            row![
                restore_btn(true),
                "Restore the state of the device",
                Space::new(Length::Fill, Length::Shrink),
                text(self.device.backup.backup_state.clone()).style(style::Text::Danger),
                backup_pick_list,
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        };

        let export_row = row![
            export_btn,
            "Export uninstalled packages with their description",
            Space::new(Length::Fill, Length::Shrink),
            text(format!(
                "Selected: user {}",
                apps_view.selected_user.unwrap_or_default().id
            )),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        container(column![backup_row, restore_row, export_row].spacing(10))
            .padding(10)
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(style::Container::Frame)
            .into()
    }

    fn no_device_container() -> Element<'static, Message, Theme, Renderer> {
        container(text("No device detected").style(style::Text::Danger))
            .padding(10)
            .width(Length::Fill)
            .style(style::Container::BorderedFrame)
            .into()
    }

    fn render_export_modal<'a>(
        content: Element<'a, Message, Theme, Renderer>,
    ) -> Element<'a, Message, Theme, Renderer> {
        let title = container(row![text("Success").size(24)].align_y(Alignment::Center))
            .width(Length::Fill)
            .style(style::Container::Frame)
            .padding([10, 0])
            .center_y(Length::Shrink)
            .center_x(Length::Shrink);

        let text_box = row![
            text(format!(
                "Exported uninstalled packages into file.\nFile is exported in same directory where {NAME} is located."
            ))
            .width(Length::Fill),
        ]
        .padding(20);

        let file_row =
            row![text(generate_backup_name(chrono::Local::now())).style(style::Text::Commentary)]
                .padding(20);

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

        let padded_content: Element<'a, Message, Theme, Renderer> =
            container(content).padding(10).into();

        Modal::new(padded_content, ctn)
            .on_blur(Message::ModalHide)
            .into()
    }
}

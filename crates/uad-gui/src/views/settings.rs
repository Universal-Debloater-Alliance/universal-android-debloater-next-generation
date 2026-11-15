use crate::folder_picker::pick_backup_folder;
use crate::theme::Theme;
use crate::theme::string_to_theme;
use crate::widgets::export_result_dialog::export_result_dialog;
use crate::widgets::primary_button::button_primary;
use crate::{
    style,
    views::list::{List as AppsView, PackageInfo},
    widgets::modal::Modal,
    widgets::navigation_menu::ICONS,
    widgets::package_row::PackageRow,
    widgets::text,
};
use iced::widget::{Space, button, checkbox, column, container, pick_list, radio, row, scrollable};
use iced::{Alignment, Element, Length, Renderer, Task, alignment};
use log::{debug, error, info};
use std::path::PathBuf;
use uad_core::{
    config::{BackupSettings, Config, DeviceSettings, GeneralSettings},
    export::export_packages,
    opener::open_url,
    save::{
        DisplayablePath, backup_phone, device_backup_dir, list_available_backup_user,
        list_available_backups, restore_backup,
    },
    sync::{
        AdbError, CorePackage, Phone, User, get_android_sdk, run_adb_shell_action,
        supports_disabling, supports_multi_user,
    },
};

/// The package rows of every user, reduced to what backups and exports need.
fn to_core_packages(packages: &[Vec<PackageRow>]) -> Vec<Vec<CorePackage>> {
    packages
        .iter()
        .map(|user_packages| user_packages.iter().map(CorePackage::from).collect())
        .collect()
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub general: GeneralSettings,
    pub device: DeviceSettings,
    is_loading: bool,
    /// Result of the last "export uninstalled packages", shown until dismissed
    export_result: Option<Result<PathBuf, String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            general: Config::load_configuration_file().general,
            device: DeviceSettings::default(),
            is_loading: false,
            export_result: None,
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
    FolderChosen(Option<PathBuf>),
    ExportPackages,
    PackagesExported(Result<PathBuf, String>),
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
            Message::ExportPackages => self.handle_export_packages(selected_user, packages),
            Message::PackagesExported(result) => self.handle_packages_exported(result),
        }
    }

    fn handle_modal_hide(&mut self) -> Task<Message> {
        self.export_result = None;
        Task::none()
    }

    fn handle_expert_mode(&mut self, _phone: &Phone, toggled: bool) -> Task<Message> {
        self.general.expert_mode = toggled;
        debug!("Config change: {self:?}");
        let mut config = Config::load_configuration_file();
        config.save_device_settings(self.device.clone(), self.general.clone());
        Task::none()
    }

    fn handle_disable_mode(&mut self, phone: &Phone, toggled: bool) -> Task<Message> {
        if supports_disabling(phone) {
            self.device.disable_mode = toggled;
            debug!("Config change: {self:?}");
            let mut config = Config::load_configuration_file();
            config.save_device_settings(self.device.clone(), self.general.clone());
        }
        Task::none()
    }

    fn handle_multi_user_mode(&mut self, _phone: &Phone, toggled: bool) -> Task<Message> {
        self.device.multi_user_mode = toggled;
        debug!("Config change: {self:?}");
        let mut config = Config::load_configuration_file();
        config.save_device_settings(self.device.clone(), self.general.clone());
        Task::none()
    }

    fn handle_apply_theme(&mut self, _phone: &Phone, theme: Theme) -> Task<Message> {
        self.general.theme = theme.to_string();
        debug!("Config change: {self:?}");
        let mut config = Config::load_configuration_file();
        config.save_device_settings(self.device.clone(), self.general.clone());
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
        let backups = list_available_backups(&device_backup_dir(
            &self.general.backup_folder,
            &phone.adb_id,
        ));
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
                    // see FAQ, and GH-issue #1426
                    disable_mode: supports_disabling(phone),
                    multi_user_mode: supports_multi_user(phone),
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
        let core_packages = to_core_packages(packages);
        // Avoid capturing &self and &phone in the future; clone owned data first.
        let user_list = phone.user_list.clone();
        let device_id = self.device.device_id.clone();
        let backup_folder = self.general.backup_folder.clone();
        Task::perform(
            async move { backup_phone(&backup_folder, user_list, &device_id, &core_packages) },
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
                self.device.backup.backups = list_available_backups(&device_backup_dir(
                    &self.general.backup_folder,
                    &phone.adb_id,
                ));
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
        let core_packages = to_core_packages(packages);
        match restore_backup(phone, &core_packages, &self.device) {
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
                        let p_info_clone = p_info.clone();
                        let phone_id = phone.adb_id.clone();
                        commands.push(Task::perform(
                            async move {
                                run_adb_shell_action(phone_id, command.as_str())
                                    .map(|_| p_info_clone)
                            },
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
                if let Some(backup) = &self.device.backup.selected {
                    info!("[RESTORE] Restoring backup {backup}");
                }
                Task::batch(commands)
            }
            Err(e) => {
                self.device.backup.backup_state.clone_from(&e);
                error!("[RESTORE] {e}");
                Task::none()
            }
        }
    }

    fn handle_folder_chosen(
        &mut self,
        phone: &Phone,
        chosen_folder: Option<PathBuf>,
    ) -> Task<Message> {
        self.is_loading = false;

        if let Some(path) = chosen_folder {
            self.general.backup_folder = path;
            let mut config = Config::load_configuration_file();
            config.save_device_settings(self.device.clone(), self.general.clone());
            self.load_device_settings(phone);
        }
        Task::none()
    }

    fn handle_choose_backup_folder(&mut self) -> Task<Message> {
        if self.is_loading {
            Task::none()
        } else {
            self.is_loading = true;
            Task::perform(pick_backup_folder(), Message::FolderChosen)
        }
    }

    fn handle_export_packages(
        &self,
        selected_user: Option<User>,
        packages: &[Vec<PackageRow>],
    ) -> Task<Message> {
        let core_packages = to_core_packages(packages);
        let dir = self.general.backup_folder.clone();
        Task::perform(
            async move { export_packages(&dir, selected_user.unwrap_or_default(), &core_packages) },
            Message::PackagesExported,
        )
    }

    fn handle_packages_exported(&mut self, result: Result<PathBuf, String>) -> Task<Message> {
        if let Err(err) = &result {
            error!("Failed to export list of uninstalled packages: {err}");
        }
        self.export_result = Some(result);
        Task::none()
    }

    pub fn view(
        &self,
        phone: &Phone,
        apps_view: &AppsView,
    ) -> Element<'_, Message, Theme, Renderer> {
        let content = self.build_content(phone, apps_view);

        if let Some(result) = &self.export_result {
            let dialog = export_result_dialog(
                "the uninstalled packages",
                result,
                Message::ModalHide,
                Message::UrlPressed,
            );
            return Modal::new(container(content).padding(10), dialog)
                .on_blur(Message::ModalHide)
                .into();
        }

        container(scrollable(content))
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// The settings sections. The device sections show a notice when no device is connected.
    fn build_content(
        &self,
        phone: &Phone,
        apps_view: &AppsView,
    ) -> Element<'_, Message, Theme, Renderer> {
        let (device_section, backup_section): (Element<'_, Message, Theme, Renderer>, _) =
            if phone.adb_id.is_empty() {
                (Self::no_device_container(), Self::no_device_container())
            } else {
                (
                    column![
                        Self::warning_container(phone),
                        self.device_specific_container(phone)
                    ]
                    .spacing(20)
                    .into(),
                    self.backup_restore_container(phone, apps_view),
                )
            };
        column![
            text("Theme").size(26),
            self.theme_container(),
            text("General").size(26),
            self.general_container(),
            text("Current device").size(26),
            device_section,
            text("Backup / Restore").size(26),
            backup_section,
        ]
        .width(Length::Fill)
        // Keeps text clear of the vertical scrollbar.
        .padding(iced::Padding {
            right: 16.0,
            ..iced::Padding::ZERO
        })
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
        let expert_mode_checkbox = checkbox(self.general.expert_mode)
            .label("Allow removing packages rated Unsafe (I know what I am doing)")
            .on_toggle(Message::ExpertMode)
            .size(20)
            .style(style::CheckBox::SettingsEnabled);

        let expert_mode_descr =
            text("Most unsafe packages are known to bootloop the device if removed.")
                .style(style::Text::Commentary);

        let choose_backup_descr =
            text("Note: to restore earlier backups, move them into the new backup folder first.")
                .style(style::Text::Commentary);

        let choose_backup_btn = button(text("\u{E930}").font(ICONS))
            .padding([5, 10])
            .on_press(Message::ChooseBackUpFolder)
            .style(style::Button::Primary);

        let choose_backup_row = row![
            choose_backup_btn,
            "Choose backup folder",
            Space::new().width(Length::Fill).height(Length::Shrink),
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
                Space::new().width(Length::Fill).height(Length::Shrink),
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
        let protected_user_ids = phone
            .user_list
            .iter()
            .filter(|&u| u.protected)
            .map(|u| u.id.to_string())
            .collect::<Vec<String>>()
            .join(", ");

        let multi_user_mode_checkbox = checkbox(self.device.multi_user_mode)
            .label("Affect all the users of the device (not only the selected user)")
            .on_toggle(Message::MultiUserMode)
            .size(20)
            .style(style::CheckBox::SettingsEnabled);

        let disable_checkbox_style = if supports_disabling(phone) {
            style::CheckBox::SettingsEnabled
        } else {
            style::CheckBox::SettingsDisabled
        };

        let disable_mode_descr =
            text("In many cases, it is better to disable a package than to uninstall it.")
                .style(style::Text::Commentary);

        let unavailable_btn = button(text("Unavailable").size(14))
            .on_press(Message::UrlPressed(PathBuf::from(
                "https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/FAQ#\
                    why-is-the-disable-mode-setting-not-available-for-my-device",
            )))
            .height(22)
            .style(style::Button::UninstallPackage);

        let disable_mode_checkbox = checkbox(self.device.disable_mode)
            .label("Clear and disable packages instead of uninstalling them")
            .on_toggle(Message::DisableMode)
            .size(20)
            .style(disable_checkbox_style);

        let disable_setting_row = if supports_disabling(phone) {
            row![
                disable_mode_checkbox,
                Space::new().width(Length::Fill).height(Length::Shrink),
            ]
            .width(Length::Fill)
        } else {
            row![
                disable_mode_checkbox,
                Space::new().width(Length::Fill).height(Length::Shrink),
                unavailable_btn,
            ]
            .width(Length::Fill)
        };

        let mut options = column![multi_user_mode_checkbox].spacing(10);
        // Shown only when there is an exception to name.
        if !protected_user_ids.is_empty() {
            options = options.push(row![
                text("This does not affect these protected work profile users: ")
                    .style(style::Text::Commentary),
                text(protected_user_ids).style(style::Text::Danger),
            ]);
        }
        let options = options.push(disable_setting_row).push(disable_mode_descr);

        container(options)
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
                device_backup_dir(&self.general.backup_folder, &phone.adb_id),
            ))
        };

        let export_btn = button_primary("Export").on_press(Message::ExportPackages);

        let backup_row = row![
            backup_btn,
            "Backup the current state of the phone",
            Space::new().width(Length::Fill).height(Length::Shrink),
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
                Space::new().width(Length::Fill).height(Length::Shrink),
                text(self.device.backup.backup_state.clone()).style(style::Text::Danger),
                backup_pick_list,
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        };

        let export_row = row![
            export_btn,
            "Export uninstalled packages with their description",
            Space::new().width(Length::Fill).height(Length::Shrink),
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
}

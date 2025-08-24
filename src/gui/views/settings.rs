use crate::core::{
    config::{BackupSettings, Config, DeviceSettings, GeneralSettings},
    helpers::button_primary,
    save::{backup_phone, list_available_backup_user, list_available_backups, restore_backup},
    sync::{AdbError, Phone, User, adb_shell_command, get_android_sdk, supports_multi_user},
    utils::{
        DisplayablePath, Error, NAME, export_packages, generate_backup_name, open_folder, open_url,
        string_to_theme,
    },
};
use crate::gui::{
    styling,
    views::list::{List as AppsView, PackageInfo},
    widgets::modal::Modal,
    widgets::navigation_menu::ICONS,
    widgets::package_row::PackageRow,
    widgets::text,
};
use iced::widget::{Space, button, checkbox, column, container, pick_list, radio, row, scrollable};
use iced::{Alignment, Element, Length, alignment, Color};
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
    ApplyTheme(crate::core::theme::Theme),
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
    #[allow(clippy::too_many_lines)]
    pub fn update(
        &mut self,
        phone: &Phone,
        packages: &[Vec<PackageRow>],
        nb_running_async_adb_commands: &mut u32,
        msg: Message,
        selected_user: Option<User>,
    ) -> iced::Task<Message> {
        match msg {
            Message::ModalHide => {
                self.modal = None;
                iced::Task::none()
            }
            Message::ExpertMode(toggled) => {
                self.general.expert_mode = toggled;
                debug!("Config change: {self:?}");
                Config::save_changes(self, &phone.adb_id);
                iced::Task::none()
            }
            Message::DisableMode(toggled) => {
                if phone.android_sdk >= 23 {
                    self.device.disable_mode = toggled;
                    debug!("Config change: {self:?}");
                    Config::save_changes(self, &phone.adb_id);
                }
                iced::Task::none()
            }
            Message::MultiUserMode(toggled) => {
                self.device.multi_user_mode = toggled;
                debug!("Config change: {self:?}");
                Config::save_changes(self, &phone.adb_id);
                iced::Task::none()
            }
            Message::ApplyTheme(theme) => {
                self.general.theme = theme.to_string();
                debug!("Config change: {self:?}");
                Config::save_changes(self, &phone.adb_id);
                iced::Task::none()
            }
            Message::UrlPressed(url) => {
                open_url(url);
                iced::Task::none()
            }
            Message::LoadDeviceSettings => {
                let backups =
                    list_available_backups(&self.general.backup_folder.join(&phone.adb_id));
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
                        }
                    }
                }
                iced::Task::none()
            }
            Message::BackupSelected(d_path) => {
                self.device.backup.selected = Some(d_path.clone());
                self.device.backup.users = list_available_backup_user(d_path);
                iced::Task::none()
            }
            Message::BackupDevice => iced::Task::perform(
                backup_phone(
                    phone.user_list.clone(),
                    self.device.device_id.clone(),
                    packages.to_vec(),
                ),
                Message::DeviceBackedUp,
            ),
            Message::DeviceBackedUp(is_backed_up) => {
                match is_backed_up {
                    Ok(_) => {
                        info!("[BACKUP] Backup successfully created");
                        self.device.backup.backups = list_available_backups(
                            &self.general.backup_folder.join(phone.adb_id.clone()),
                        );
                        self.device.backup.selected = self.device.backup.backups.first().cloned();
                    }
                    Err(err) => {
                        error!("[BACKUP FAILED] Backup creation failed: {err:?}");
                    }
                }
                iced::Task::none()
            }
            Message::RestoreDevice => match restore_backup(phone, packages, &self.device) {
                Ok(r_packages) => {
                    let mut commands = vec![];
                    *nb_running_async_adb_commands = 0;
                    for p in &r_packages {
                        let p_info = PackageInfo {
                            i_user: 0,
                            index: p.index,
                            removal: "RESTORE".to_string(),
                        };
                        for command in p.commands.clone() {
                            *nb_running_async_adb_commands += 1;
                            commands.push(iced::Task::perform(
                                // This is "safe" thanks to serde:
                                // https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/issues/760
                                adb_shell_command(phone.adb_id.clone(), command, p_info.clone()),
                                Message::RestoringDevice,
                            ));
                        }
                    }
                    if r_packages.is_empty() {
                        if get_android_sdk(&phone.adb_id) == 0 {
                            self.device.backup.backup_state = "Device is not connected".to_string();
                        } else {
                            self.device.backup.backup_state =
                                "Device state is already restored".to_string();
                        }
                    }
                    info!(
                        "[RESTORE] Restoring backup {}",
                        self.device.backup.selected.as_ref().unwrap()
                    );
                    iced::Task::batch(commands)
                }
                Err(e) => {
                    self.device.backup.backup_state.clone_from(&e);
                    error!("{} - {}", self.device.backup.selected.as_ref().unwrap(), e);
                    iced::Task::none()
                }
            },
            // Trigger an action in mod.rs (Message::SettingsAction(msg))
            Message::RestoringDevice(_) => iced::Task::none(),
            Message::FolderChosen(result) => {
                self.is_loading = false;

                if let Ok(path) = result {
                    self.general.backup_folder = path;
                    Config::save_changes(self, &phone.adb_id);
                    #[expect(unused_must_use, reason = "side-effect")]
                    {
                        self.update(
                            phone,
                            packages,
                            nb_running_async_adb_commands,
                            Message::LoadDeviceSettings,
                            selected_user,
                        );
                    }
                }
                iced::Task::none()
            }
            Message::ChooseBackUpFolder => {
                if self.is_loading {
                    iced::Task::none()
                } else {
                    self.is_loading = true;
                    iced::Task::perform(open_folder(), Message::FolderChosen)
                }
            }
            Message::ExportPackages => iced::Task::perform(
                export_packages(selected_user.unwrap_or_default(), packages.to_vec()),
                Message::PackagesExported,
            ),
            Message::PackagesExported(exported) => {
                match exported {
                    Ok(_) => self.modal = Some(PopUpModal::ExportUninstalled),
                    Err(err) => error!("Failed to export list of uninstalled packages: {err:?}"),
                }
                iced::Task::none()
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn view(&self, phone: &Phone, apps_view: &AppsView) -> Element<Message> {
        let radio_btn_theme = crate::core::theme::Theme::ALL
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
        let theme_ctn = container(radio_btn_theme)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(styling::frame_container());

        let expert_mode_checkbox = checkbox(
            "Allow to uninstall packages marked as \"unsafe\" (I KNOW WHAT I AM DOING)",
            self.general.expert_mode,
        )
        .on_toggle(Message::ExpertMode);

        let expert_mode_descr =
            text("Most unsafe packages are known to bootloop the device if removed.")
                .color(Color::from_rgb(0.6, 0.6, 0.6));

        let choose_backup_descr = text("Note: If you have previous backups, you will need to transfer them manually to newly changed backup folder to be able to use Restore functionality")
            .color(Color::from_rgb(0.6, 0.6, 0.6));

        let choose_backup_btn = button(text("\u{E930}").font(ICONS))
            .padding([5, 10])
            .on_press(Message::ChooseBackUpFolder)
            .style(styling::primary_button());

        let choose_backup_row = row![
            choose_backup_btn,
            "Choose backup folder",
            Space::new(Length::Fill, Length::Shrink),
            "Current folder: ",
            text(self.general.backup_folder.to_string_lossy())
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let general_ctn = container(
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
        .style(styling::frame_container());

        let warning_ctn = container(
            row![
                text("The following settings only affect the currently selected device:")
                    .color(Color::from_rgb(0.8, 0.2, 0.2)),
                text(phone.model.clone()),
                Space::new(Length::Fill, Length::Shrink),
                text(phone.adb_id.clone()).color(Color::from_rgb(0.6, 0.6, 0.6))
            ]
            .spacing(7),
        )
        .padding(10)
        .width(Length::Fill)
        .style(styling::bordered_frame_container());

        let multi_user_mode_descr = row![
            text("This will not affect the following protected work profile users: ")
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            text(
                phone
                    .user_list
                    .iter()
                    .filter(|&u| u.protected)
                    .map(|u| u.id.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            )
            .color(Color::from_rgb(0.8, 0.2, 0.2))
        ];

        let multi_user_mode_checkbox = checkbox(
            "Affect all the users of the device (not only the selected user)",
            self.device.multi_user_mode,
        )
        .on_toggle(Message::MultiUserMode);

        let disable_mode_descr =
            text("In some cases, it can be better to disable a package instead of uninstalling it")
                .color(Color::from_rgb(0.6, 0.6, 0.6));

        let unavailable_btn = button(text("Unavailable").size(14))
            .on_press(Message::UrlPressed(PathBuf::from(
                "https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki/FAQ#\
                    why-is-the-disable-mode-setting-not-available-for-my-device",
            )))
            .height(22)
            .style(styling::danger_button());

        // Disabling package without root isn't really possible before Android Oreo (8.0)
        // see https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki/ADB-reference
        let disable_mode_checkbox = checkbox(
            "Clear and disable packages instead of uninstalling them",
            self.device.disable_mode,
        )
        .on_toggle(if phone.android_sdk >= 23 {
            Message::DisableMode
        } else {
            |_| Message::ModalHide // Dummy message that does nothing
        });

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

        let device_specific_ctn = container(
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
        .style(styling::frame_container());

        let backup_pick_list = pick_list(
            self.device.backup.backups.clone(),
            self.device.backup.selected.clone(),
            Message::BackupSelected,
        )
        .padding(6);

        let backup_btn =
            button_primary(text("Backup").align_x(alignment::Horizontal::Center))
                .on_press(Message::BackupDevice)
                .width(77);

        let restore_btn = |enabled| {
            if enabled {
                button(text("Restore").align_x(alignment::Horizontal::Center))
                    .padding([5, 10])
                    .on_press(Message::RestoreDevice)
                    .style(styling::restore_button())
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
                text(self.device.backup.backup_state.clone()).color(Color::from_rgb(0.8, 0.2, 0.2)),
                backup_pick_list,
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        };

        let no_device_ctn = || {
            container(text("No device detected").color(Color::from_rgb(0.8, 0.2, 0.2)))
                .padding(10)
                .width(Length::Fill)
                .style(styling::bordered_frame_container())
        };

        let content = if phone.adb_id.is_empty() {
            column![
                text("Theme").size(26),
                theme_ctn,
                text("General").size(26),
                general_ctn,
                text("Current device").size(26),
                no_device_ctn(),
                text("Backup / Restore").size(26),
                no_device_ctn(),
            ]
            .width(Length::Fill)
            .spacing(20)
        } else {
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

            let backup_restore_ctn =
                container(column![backup_row, restore_row, export_row].spacing(10))
                    .padding(10)
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .style(styling::frame_container());

            column![
                text("Theme").size(26),
                theme_ctn,
                text("General").size(26),
                general_ctn,
                text("Current device").size(26),
                warning_ctn,
                device_specific_ctn,
                text("Backup / Restore").size(26),
                backup_restore_ctn,
            ]
            .width(Length::Fill)
            .spacing(20)
        };

        if let Some(PopUpModal::ExportUninstalled) = self.modal {
            let title = container(row![text("Success").size(24)].align_y(Alignment::Center))
                .width(Length::Fill)
                .style(styling::frame_container())
                .padding(iced::Padding::from([10.0, 0.0]))
                .center_y(Length::Shrink)
                .center_x(Length::Shrink);

            let text_box = row![
                text(format!("Exported uninstalled packages into file.\nFile is exported in same directory where {NAME} is located.")).width(Length::Fill),
            ].padding(20);

            let file_row = row![
                text(generate_backup_name(chrono::Local::now())).color(Color::from_rgb(0.6, 0.6, 0.6))
            ]
            .padding(20);

            let modal_btn_row = row![
                Space::new(Length::Fill, Length::Shrink),
                button(text("Close").width(Length::Shrink))
                    .width(Length::Shrink)
                    .on_press(Message::ModalHide)
                    .style(styling::primary_button()),
                Space::new(Length::Fill, Length::Shrink),
            ];

            let ctn = container(column![title, text_box, file_row, modal_btn_row])
                .height(Length::Shrink)
                .width(500)
                .padding(10)
                .style(styling::frame_container());

            return Modal::new(content.padding(10), ctn)
                .on_blur(Message::ModalHide)
                .into();
        }

        container(scrollable(content))
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
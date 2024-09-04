use crate::core::helpers::button_primary;
use crate::core::sync::AdbError;

use crate::core::config::{BackupSettings, Config, DeviceSettings, GeneralSettings};
use crate::core::save::{
    backup_phone, list_available_backup_user, list_available_backups, restore_backup,
};
use crate::core::sync::{get_android_sdk, perform_adb_commands, CommandType, Phone, User};
use crate::core::theme::Theme;
use crate::core::utils::{
    export_packages, open_folder, open_url, string_to_theme, DisplayablePath,
    UNINSTALLED_PACKAGES_FILE_NAME,
};
use crate::gui::style;
use crate::gui::views::list::{List as AppsView, PackageInfo};
use crate::gui::widgets::modal::Modal;
use crate::gui::widgets::navigation_menu::ICONS;
use crate::gui::widgets::package_row::PackageRow;

use iced::widget::{
    button, checkbox, column, container, pick_list, radio, row, scrollable, text, Space, Text,
};
use iced::{alignment, Alignment, Command, Element, Length, Renderer};
use std::path::PathBuf;

use crate::core::utils::{Error, NAME};

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
    RestoringDevice(Result<CommandType, AdbError>),
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
    ) -> Command<Message> {
        match msg {
            Message::ModalHide => {
                self.modal = None;
                Command::none()
            }
            Message::ExpertMode(toggled) => {
                self.general.expert_mode = toggled;
                debug!("Config change: {:?}", self);
                Config::save_changes(self, &phone.adb_id);
                Command::none()
            }
            Message::DisableMode(toggled) => {
                if phone.android_sdk >= 23 {
                    self.device.disable_mode = toggled;
                    debug!("Config change: {:?}", self);
                    Config::save_changes(self, &phone.adb_id);
                }
                Command::none()
            }
            Message::MultiUserMode(toggled) => {
                self.device.multi_user_mode = toggled;
                debug!("Config change: {:?}", self);
                Config::save_changes(self, &phone.adb_id);
                Command::none()
            }
            Message::ApplyTheme(theme) => {
                self.general.theme = theme.to_string();
                debug!("Config change: {:?}", self);
                Config::save_changes(self, &phone.adb_id);
                Command::none()
            }
            Message::UrlPressed(url) => {
                open_url(url);
                Command::none()
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
                            multi_user_mode: phone.android_sdk > 21,
                            disable_mode: false,
                            backup,
                        }
                    }
                };
                Command::none()
            }
            Message::BackupSelected(d_path) => {
                self.device.backup.selected = Some(d_path.clone());
                self.device.backup.users = list_available_backup_user(d_path);
                Command::none()
            }
            Message::BackupDevice => Command::perform(
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
                        error!("[BACKUP FAILED] Backup creation failed: {:?}", err);
                    }
                }
                Command::none()
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
                            commands.push(Command::perform(
                                perform_adb_commands(
                                    command,
                                    CommandType::PackageManager(p_info.clone()),
                                ),
                                Message::RestoringDevice,
                            ));
                        }
                    }
                    if r_packages.is_empty() {
                        if get_android_sdk() == 0 {
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
                    Command::batch(commands)
                }
                Err(e) => {
                    self.device.backup.backup_state = e.clone();
                    error!("{} - {}", self.device.backup.selected.as_ref().unwrap(), e);
                    Command::none()
                }
            },
            // Trigger an action in mod.rs (Message::SettingsAction(msg))
            Message::RestoringDevice(_) => Command::none(),
            Message::FolderChosen(result) => {
                self.is_loading = false;

                if let Ok(path) = result {
                    self.general.backup_folder = path;
                    Config::save_changes(self, &phone.adb_id);
                    #[allow(unused_must_use)]
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
                Command::none()
            }
            Message::ChooseBackUpFolder => {
                if self.is_loading {
                    Command::none()
                } else {
                    self.is_loading = true;
                    Command::perform(open_folder(), Message::FolderChosen)
                }
            }
            Message::ExportPackages => Command::perform(
                export_packages(selected_user.unwrap_or_default(), packages.to_vec()),
                Message::PackagesExported,
            ),
            Message::PackagesExported(exported) => {
                match exported {
                    Ok(_) => self.modal = Some(PopUpModal::ExportUninstalled),
                    Err(err) => error!("Failed to export list of uninstalled packages: {:?}", err),
                }
                Command::none()
            }
        }
    }

    pub fn view(&self, phone: &Phone, apps_view: &AppsView) -> Element<Message, Theme, Renderer> {
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
        let theme_ctn = container(radio_btn_theme)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(style::Container::Frame);

        let expert_mode_checkbox = checkbox(
            "Allow to uninstall packages marked as \"unsafe\" (I KNOW WHAT I AM DOING)",
            self.general.expert_mode,
        )
        .on_toggle(Message::ExpertMode)
        .style(style::CheckBox::SettingsEnabled);

        let expert_mode_descr =
            text("Most unsafe packages are known to bootloop the device if removed.")
                .style(style::Text::Commentary);

        let choose_backup_descr = text("Note: If you have previous backups, you will need to transfer them manually to newly changed backup folder to be able to use Restore functionality")
            .style(style::Text::Commentary);

        let choose_backup_btn = button(Text::new("\u{E930}").font(ICONS))
            .padding([5, 10])
            .on_press(Message::ChooseBackUpFolder)
            .style(style::Button::Primary);

        let choose_backup_row = row![
            choose_backup_btn,
            "Choose backup folder",
            Space::new(Length::Fill, Length::Shrink),
            "Current folder: ",
            Text::new(self.general.backup_folder.to_string_lossy())
        ]
        .spacing(10)
        .align_items(Alignment::Center);

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
        .style(style::Container::Frame);

        let warning_ctn = container(
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
        .style(style::Container::BorderedFrame);

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

        // Disabling package without root isn't really possible before Android Oreo (8.0)
        // see https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki/ADB-reference
        let disable_mode_checkbox = checkbox(
            "Clear and disable packages instead of uninstalling them",
            self.device.disable_mode,
        )
        .on_toggle(Message::DisableMode)
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
        .style(style::Container::Frame);

        let backup_pick_list = pick_list(
            self.device.backup.backups.clone(),
            self.device.backup.selected.clone(),
            Message::BackupSelected,
        )
        .padding(6);

        let backup_btn =
            button_primary(text("Backup").horizontal_alignment(alignment::Horizontal::Center))
                .on_press(Message::BackupDevice)
                .width(77);

        let restore_btn = |enabled| {
            if enabled {
                button(text("Restore").horizontal_alignment(alignment::Horizontal::Center))
                    .padding([5, 10])
                    .on_press(Message::RestoreDevice)
                    .width(77)
            } else {
                button(
                    text("No backup")
                        .horizontal_alignment(alignment::Horizontal::Center)
                        .vertical_alignment(alignment::Vertical::Center),
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
        .align_items(Alignment::Center);

        let restore_row = if !self.device.backup.backups.is_empty() {
            row![
                restore_btn(true),
                "Restore the state of the device",
                Space::new(Length::Fill, Length::Shrink),
                text(self.device.backup.backup_state.clone()).style(style::Text::Danger),
                backup_pick_list,
            ]
            .spacing(10)
            .align_items(Alignment::Center)
        } else {
            row![]
        };

        let no_device_ctn = || {
            container(text("No device detected").style(style::Text::Danger))
                .padding(10)
                .width(Length::Fill)
                .style(style::Container::BorderedFrame)
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
            .align_items(Alignment::Center);

            let backup_restore_ctn =
                container(column![backup_row, restore_row, export_row].spacing(10))
                    .padding(10)
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .style(style::Container::Frame);

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
            let title = container(row![text("Success").size(24)].align_items(Alignment::Center))
                .width(Length::Fill)
                .style(style::Container::Frame)
                .padding([10, 0, 10, 0])
                .center_y()
                .center_x();

            let text_box = row![
                text(format!("Exported uninstalled packages into file.\nFile is exported in same directory where {NAME} is located.")).width(Length::Fill),
            ].padding(20);

            let file_row = row![text(format!(
                "{}_{}.txt",
                UNINSTALLED_PACKAGES_FILE_NAME,
                chrono::Local::now().format("%Y%m%d")
            ))
            .style(style::Text::Commentary)]
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

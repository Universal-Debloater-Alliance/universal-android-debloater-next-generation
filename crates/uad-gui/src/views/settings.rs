use crate::helpers::{button_primary, open_folder};
use crate::theme::Theme;
use crate::theme::string_to_theme;
use crate::{
    style, views::list::List as AppsView, widgets::modal::Modal, widgets::navigation_menu::ICONS,
    widgets::package_row::PackageRow, widgets::text,
};
use iced::widget::{Space, button, checkbox, column, container, pick_list, radio, row, scrollable};
use iced::{Alignment, Element, Length, Renderer, Task, alignment};
use log::{debug, error, info};
use std::{collections::HashMap, path::PathBuf};
use uad_core::{
    config::{BackupSettings, Config, DeviceSettings, GeneralSettings},
    save::{backup_phone, device_backup_dir, list_available_backups, restore_backup},
    sync::{
        AdbError, CorePackage, Phone, User, get_android_sdk, run_adb_shell_action,
        supports_disabling, supports_multi_user,
    },
    utils::{DisplayablePath, Error, export_packages, open_url},
};

#[derive(Debug, Clone)]
pub enum PopUpModal {
    ExportUninstalled(PathBuf),
}

#[derive(Debug, Clone)]
struct RestoreBatch {
    serial: String,
    remaining: usize,
    skipped: usize,
    /// Name of the package restored last
    restored: String,
    failures: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub general: GeneralSettings,
    pub device: DeviceSettings,
    active_restore: Option<RestoreBatch>,
    restore_reports: HashMap<String, String>,
    /// Why the last backup could not be made
    backup_error: String,
    is_loading: bool,
    modal: Option<PopUpModal>,
}

impl Default for Settings {
    /// Default settings, without reading the configuration file.
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            device: DeviceSettings::default(),
            active_restore: None,
            restore_reports: HashMap::new(),
            backup_error: String::new(),
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
    /// Name of the restored package, or why it failed
    RestorePackageFinished(Result<String, String>),
    DeviceBackedUp(Result<bool, String>),
    ChooseBackUpFolder,
    FolderChosen(Result<PathBuf, Error>),
    ExportPackages,
    PackagesExported(Result<PathBuf, String>),
    ModalHide,
}

impl Settings {
    /// Settings saved in the configuration file.
    pub(crate) fn load() -> Self {
        Self {
            general: Config::load_configuration_file().general,
            ..Self::default()
        }
    }

    pub fn update(&mut self, phone: &Phone, apps: &AppsView, msg: Message) -> Task<Message> {
        match msg {
            Message::ModalHide => self.handle_modal_hide(),
            Message::ExpertMode(toggled) => self.handle_expert_mode(phone, toggled),
            Message::DisableMode(toggled) => self.handle_disable_mode(phone, toggled),
            Message::MultiUserMode(toggled) => self.handle_multi_user_mode(phone, toggled),
            Message::ApplyTheme(theme) => self.handle_apply_theme(phone, theme),
            Message::UrlPressed(url) => Self::handle_url_pressed(url),
            Message::LoadDeviceSettings => self.handle_load_device_settings(phone),
            Message::BackupSelected(d_path) => self.handle_backup_selected(d_path),
            Message::BackupDevice | Message::RestoreDevice | Message::ExportPackages
                if !apps.inventory_ready(phone) =>
            {
                Task::none()
            }
            // It would save a half-restored device.
            Message::BackupDevice if self.restoring(phone).is_some() => Task::none(),
            Message::BackupDevice => self.handle_backup_device(phone, &apps.phone_packages),
            Message::DeviceBackedUp(result) => self.handle_device_backed_up(phone, result),
            Message::RestoreDevice if self.active_restore.is_some() => Task::none(),
            Message::RestoreDevice => self.handle_restore_device(phone, &apps.phone_packages),
            Message::RestorePackageFinished(_) => Task::none(),
            Message::FolderChosen(result) => self.handle_folder_chosen(phone, result),
            Message::ChooseBackUpFolder => self.handle_choose_backup_folder(),
            Message::ExportPackages => {
                self.handle_export_packages(apps.selected_user, &apps.phone_packages)
            }
            Message::PackagesExported(result) => self.handle_packages_exported(result),
        }
    }

    fn handle_modal_hide(&mut self) -> Task<Message> {
        self.modal = None;
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
        self.load_device_settings(phone, &Config::load_configuration_file());
        Task::none()
    }

    fn load_device_settings(&mut self, phone: &Phone, config: &Config) {
        self.backup_error.clear();
        let backups = list_available_backups(&device_backup_dir(
            &self.general.backup_folder,
            &phone.adb_id,
        ));
        // The status of a running restore stays. A report waiting to be seen is shown once.
        let backup_state = if self.restoring(phone).is_some() {
            self.restore_reports.get(&phone.adb_id).cloned()
        } else {
            self.restore_reports.remove(&phone.adb_id)
        };
        let backup = BackupSettings {
            backups: backups.clone(),
            selected: backups.first().cloned(),
            backup_state: backup_state.unwrap_or_default(),
        };

        match config.devices.iter().find(|d| d.device_id == phone.adb_id) {
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
        self.device.backup.selected = Some(d_path);
        Task::none()
    }

    fn handle_backup_device(
        &mut self,
        phone: &Phone,
        packages: &[Vec<PackageRow>],
    ) -> Task<Message> {
        let core_packages: Vec<Vec<_>> = packages
            .iter()
            .map(|user_packages| user_packages.iter().map(CorePackage::from).collect())
            .collect();
        // Avoid capturing &self and &phone in the future; clone owned data first.
        let user_list = phone.user_list.clone();
        let device_id = self.device.device_id.clone();
        let packages_owned = core_packages;
        let backup_dir = self.general.backup_folder.clone();
        Task::perform(
            async move { backup_phone(&backup_dir, user_list, device_id, &packages_owned) },
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
                self.backup_error.clear();
                self.device.backup.backups = list_available_backups(&device_backup_dir(
                    &self.general.backup_folder,
                    &phone.adb_id,
                ));
                self.device.backup.selected = self.device.backup.backups.first().cloned();
            }
            Err(err) => {
                error!("[BACKUP FAILED] Backup creation failed: {err:?}");
                self.backup_error = err;
            }
        }
        Task::none()
    }

    fn handle_restore_device(
        &mut self,
        phone: &Phone,
        packages: &[Vec<PackageRow>],
    ) -> Task<Message> {
        let core_packages: Vec<Vec<_>> = packages
            .iter()
            .map(|user_packages| user_packages.iter().map(CorePackage::from).collect())
            .collect();
        match restore_backup(phone, &core_packages, &self.device) {
            Ok(restore_result) => {
                let mut commands = vec![];
                for package in restore_result
                    .packages
                    .into_iter()
                    .filter(|p| !p.commands.is_empty())
                {
                    let name = packages[package.i_user][package.index].name.clone();
                    let user_id = phone.user_list[package.i_user].id;
                    let serial = phone.adb_id.clone();
                    commands.push(Task::perform(
                        async move {
                            for command in package.commands {
                                if let Err(AdbError::Generic(error)) =
                                    run_adb_shell_action(&serial, &command)
                                {
                                    return Err(format!("{name} (user {user_id}): {error}"));
                                }
                            }
                            Ok(name)
                        },
                        Message::RestorePackageFinished,
                    ));
                }
                let skipped = restore_result.skipped_count;
                let status = if !commands.is_empty() {
                    self.active_restore = Some(RestoreBatch {
                        serial: phone.adb_id.clone(),
                        remaining: commands.len(),
                        skipped,
                        restored: String::new(),
                        failures: Vec::new(),
                    });
                    format!("Restoring {} package entries…", commands.len())
                } else if skipped > 0 {
                    format!(
                        "Restore completed with {skipped} packages skipped (not found on device)"
                    )
                } else if get_android_sdk(&phone.adb_id) == 0 {
                    "Device is not connected".to_string()
                } else {
                    "Device state is already restored".to_string()
                };
                // Every result replaces the one kept for the next reload.
                self.restore_reports
                    .insert(phone.adb_id.clone(), status.clone());
                self.device.backup.backup_state = status;
                info!(
                    "[RESTORE] Restoring backup {}",
                    self.device.backup.selected.as_ref().unwrap()
                );
                Task::batch(commands)
            }
            Err(e) => {
                self.restore_reports.insert(phone.adb_id.clone(), e.clone());
                self.device.backup.backup_state.clone_from(&e);
                if let Some(backup) = &self.device.backup.selected {
                    error!("{backup} - {e}");
                } else {
                    error!("{e}");
                }
                Task::none()
            }
        }
    }

    /// Name of the package restored last, while a restore is running on this phone.
    pub(crate) fn restoring(&self, phone: &Phone) -> Option<&str> {
        self.active_restore
            .as_ref()
            .filter(|restore| restore.serial == phone.adb_id)
            .map(|restore| restore.restored.as_str())
    }

    /// Return true when the displayed phone needs a fresh inventory.
    pub(crate) fn finish_restore_package(&mut self, result: &Result<String, String>) -> bool {
        let Some(active) = self.active_restore.as_mut() else {
            return false;
        };
        match result {
            Ok(name) => active.restored.clone_from(name),
            Err(error) => active.failures.push(error.clone()),
        }
        active.remaining -= 1;
        if active.remaining > 0 {
            return false;
        }

        let batch = self.active_restore.take().expect("active restore");
        let mut report = if batch.failures.is_empty() {
            "Restore completed successfully".to_string()
        } else {
            // The report only has room for a few, the log gets them all.
            for line in &batch.failures {
                error!("[RESTORE] {line}");
            }
            let shown = batch.failures.len().min(3);
            let mut report = format!(
                "Restore finished with errors:\n{}",
                batch.failures[..shown].join("\n")
            );
            if shown < batch.failures.len() {
                report = format!(
                    "{report}\n… and {} more, see the log.",
                    batch.failures.len() - shown
                );
            }
            report
        };
        if batch.skipped > 0 {
            report = format!(
                "{report}\nSkipped {} packages missing from the device.",
                batch.skipped
            );
        }
        // Also kept for the next reload of its device: it may not have been seen yet.
        let displayed = self.device.device_id == batch.serial;
        if displayed {
            self.device.backup.backup_state.clone_from(&report);
        }
        self.restore_reports.insert(batch.serial, report);
        displayed
    }

    fn handle_folder_chosen(
        &mut self,
        phone: &Phone,
        result: Result<PathBuf, Error>,
    ) -> Task<Message> {
        self.is_loading = false;

        if let Ok(path) = result {
            self.general.backup_folder = path;
            let mut config = Config::load_configuration_file();
            config.save_device_settings(self.device.clone(), self.general.clone());
            self.load_device_settings(phone, &config);
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
        &self,
        selected_user: Option<User>,
        packages: &[Vec<PackageRow>],
    ) -> Task<Message> {
        let core_packages: Vec<Vec<_>> = packages
            .iter()
            .map(|user_packages| user_packages.iter().map(CorePackage::from).collect())
            .collect();
        let dir = self.general.backup_folder.clone();
        Task::perform(
            async move { export_packages(&dir, selected_user.unwrap_or_default(), &core_packages) },
            Message::PackagesExported,
        )
    }

    fn handle_packages_exported(&mut self, result: Result<PathBuf, String>) -> Task<Message> {
        match result {
            Ok(path) => self.modal = Some(PopUpModal::ExportUninstalled(path)),
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

        if let Some(PopUpModal::ExportUninstalled(path)) = &self.modal {
            return Self::render_export_modal(content, path);
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
        let expert_mode_checkbox = checkbox(self.general.expert_mode)
            .label("Allow to uninstall packages marked as \"unsafe\" (I KNOW WHAT I AM DOING)")
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
            text("In many cases, it's better to disable a package instead of uninstalling it")
                .style(style::Text::Commentary);

        let unavailable_btn = button(text("Unavailable").size(14))
            .on_press(Message::UrlPressed(PathBuf::from(
                "https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki/FAQ#\
                    why-is-the-disable-mode-setting-not-available-for-my-device",
            )))
            .height(22)
            .style(style::Button::Unavailable);

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

        let ready = apps_view.inventory_ready(phone);
        let backup_btn = button_primary(text("Backup").align_x(alignment::Horizontal::Center))
            .on_press_maybe(
                (ready && self.restoring(phone).is_none()).then_some(Message::BackupDevice),
            )
            .width(77);

        let restore_btn = button(text("Restore").align_x(alignment::Horizontal::Center))
            .padding([5, 10])
            .on_press_maybe(
                (ready && self.active_restore.is_none()).then_some(Message::RestoreDevice),
            )
            .width(77);

        let locate_backup_btn = if self.device.backup.backups.is_empty() {
            button_primary("Open backup directory")
        } else {
            button_primary("Open backup directory").on_press(Message::UrlPressed(
                device_backup_dir(&self.general.backup_folder, &phone.adb_id),
            ))
        };

        let export_btn =
            button_primary("Export").on_press_maybe(ready.then_some(Message::ExportPackages));

        let backup_row = row![
            backup_btn,
            "Backup the current state of the phone",
            Space::new().width(Length::Fill).height(Length::Shrink),
            text(self.backup_error.clone()).style(style::Text::Danger),
            locate_backup_btn,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let restore_row = if self.device.backup.backups.is_empty() {
            row![]
        } else {
            row![
                restore_btn,
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

    fn render_export_modal<'a>(
        content: Element<'a, Message, Theme, Renderer>,
        path: &std::path::Path,
    ) -> Element<'a, Message, Theme, Renderer> {
        let title = container(row![text("Success").size(24)].align_y(Alignment::Center))
            .width(Length::Fill)
            .style(style::Container::Frame)
            .padding([10, 0])
            .center_y(Length::Shrink)
            .center_x(Length::Shrink);

        let text_box =
            row![text("Exported uninstalled packages to:").width(Length::Fill),].padding(20);

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

        let padded_content: Element<'a, Message, Theme, Renderer> =
            container(content).padding(10).into();

        Modal::new(padded_content, ctn)
            .on_blur(Message::ModalHide)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phone(serial: &str) -> Phone {
        Phone {
            adb_id: serial.to_string(),
            ..Phone::default()
        }
    }

    fn start_restore(settings: &mut Settings, serial: &str, packages: usize) {
        settings.active_restore = Some(RestoreBatch {
            serial: serial.to_string(),
            remaining: packages,
            skipped: 0,
            restored: String::new(),
            failures: Vec::new(),
        });
    }

    fn restore_with_five_failures(settings: &mut Settings, serial: &str) {
        start_restore(settings, serial, 5);
        for i in 0..5 {
            settings.finish_restore_package(&Err(format!("failure {i}")));
        }
    }

    #[test]
    fn restore_is_in_progress_for_its_phone_until_the_last_package() {
        let mut settings = Settings::default();
        start_restore(&mut settings, "A", 2);
        assert_eq!(settings.restoring(&phone("A")), Some(""));
        assert_eq!(settings.restoring(&phone("B")), None);

        assert!(!settings.finish_restore_package(&Ok("com.example.a".to_string())));
        assert_eq!(settings.restoring(&phone("A")), Some("com.example.a"));
        settings.finish_restore_package(&Err("failure".to_string()));
        assert_eq!(settings.restoring(&phone("A")), None);
    }

    #[test]
    fn restore_report_is_short_and_kept_for_one_reload() {
        const REPORT: &str = "Restore finished with errors:\n\
            failure 0\nfailure 1\nfailure 2\n… and 2 more, see the log.";
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let mut settings = Settings::default();
        settings.general.backup_folder = dir.path().to_path_buf();

        // The device can be refreshed, or reconnect after a reboot,
        // before the report has been seen.
        settings.load_device_settings(&phone("A"), &config);
        restore_with_five_failures(&mut settings, "A");
        assert_eq!(settings.device.backup.backup_state, REPORT);
        settings.load_device_settings(&phone("A"), &config);
        assert_eq!(settings.device.backup.backup_state, REPORT);
        settings.load_device_settings(&phone("A"), &config);
        assert_eq!(settings.device.backup.backup_state, "");

        // Finished while another device is displayed.
        restore_with_five_failures(&mut settings, "B");
        assert_eq!(settings.device.backup.backup_state, "");
        settings.load_device_settings(&phone("B"), &config);
        assert_eq!(settings.device.backup.backup_state, REPORT);
        settings.load_device_settings(&phone("B"), &config);
        assert_eq!(settings.device.backup.backup_state, "");
    }

    #[test]
    fn a_restore_that_cannot_start_replaces_the_previous_report() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let mut settings = Settings::default();
        settings.general.backup_folder = dir.path().to_path_buf();
        settings.load_device_settings(&phone("A"), &config);
        restore_with_five_failures(&mut settings, "A");

        settings.device.backup.selected = Some(DisplayablePath {
            path: dir.path().join("missing.json"),
        });
        let _ = settings.handle_restore_device(&phone("A"), &[]);
        let error = settings.device.backup.backup_state.clone();
        assert!(!error.starts_with("Restore finished"));
        settings.load_device_settings(&phone("A"), &config);
        assert_eq!(settings.device.backup.backup_state, error);
    }

    #[test]
    fn a_failed_backup_is_shown_until_the_next_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        settings.general.backup_folder = dir.path().to_path_buf();
        let error = "The packages of user 10 could not be read".to_string();
        let _ = settings.handle_device_backed_up(&phone("A"), Err(error.clone()));
        assert_eq!(settings.backup_error, error);
        let _ = settings.handle_device_backed_up(&phone("A"), Ok(true));
        assert_eq!(settings.backup_error, "");
    }
}

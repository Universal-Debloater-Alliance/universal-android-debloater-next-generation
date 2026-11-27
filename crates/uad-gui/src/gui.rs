#[cfg(feature = "img")]
use crate::theme::OS_COLOR_SCHEME;
use crate::theme::Theme;
use crate::theme::string_to_theme;
use crate::views::about::{About as AboutView, Message as AboutMessage};
use crate::views::list::{
    List as AppsView, LoadingState as ListLoadingState, Message as AppsMessage,
};
use crate::views::settings::{Message as SettingsMessage, Settings as SettingsView};
use crate::widgets::navigation_menu::nav_menu;
use iced::font;
use iced::widget::column;
#[cfg(feature = "img")]
use iced::window::icon;
use iced::{Alignment, Element, Length, Settings, Task, window::Settings as Window};
use iced::{Subscription, event, keyboard};
#[cfg(feature = "img")]
use image::ImageFormat;
#[cfg(feature = "self-update")]
use log::debug;
use log::{error, info};
#[cfg(feature = "self-update")]
use std::path::PathBuf;
use uad_core::adb;
use uad_core::sync::{Phone, get_devices_list, initial_load};
use uad_core::uad_lists::UadListState;
use uad_core::update::{Release, SelfUpdateState, SelfUpdateStatus, get_latest_release};
use uad_core::utils::{FULL_NAME, NAME};

#[cfg(feature = "self-update")]
use uad_core::update::{BIN_NAME, download_update_to_temp_file, remove_file, rename};

#[derive(Default, Debug, Clone)]
enum View {
    #[default]
    List,
    About,
    Settings,
}

#[derive(Default, Clone)]
pub struct UpdateState {
    pub self_update: SelfUpdateState,
    pub uad_list: UadListState,
}

#[derive(Default)]
pub struct UadGui {
    view: View,
    apps_view: AppsView,
    about_view: AboutView,
    settings_view: SettingsView,
    devices_list: Vec<Phone>,
    discovery_id: u64,
    /// index of `devices_list`
    selected_device: Option<Phone>,
    update_state: UpdateState,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Navigation Panel
    AboutPressed,
    SettingsPressed,
    AppsPress,
    DeviceSelected(Phone),
    AboutAction(AboutMessage),
    AppsAction(AppsMessage),
    SettingsAction(SettingsMessage),
    RefreshButtonPressed,
    RebootButtonPressed,
    LoadDevices(u64, Vec<Phone>),
    #[cfg(feature = "self-update")]
    NewReleaseDownloaded(Result<(PathBuf, PathBuf), ()>),
    GetLatestRelease(Result<Option<Release>, ()>),
    FontLoaded(Result<(), iced::font::Error>),
    Nothing,
    ADBSatisfied(bool),
}

impl UadGui {
    #[allow(
        clippy::unused_self,
        reason = "required by iced's Application trait interface"
    )]
    fn title(&self) -> String {
        self.selected_device.as_ref().map_or_else(
            || FULL_NAME.to_string(),
            |device| format!("{FULL_NAME} - {}", device.model),
        )
    }

    fn new() -> (Self, Task<Message>) {
        (
            Self {
                settings_view: SettingsView::load(),
                ..Self::default()
            },
            Task::batch([
                // Used in crate::widgets::navigation_menu::ICONS. Name is `icomoon`.
                font::load(include_bytes!("../../../resources/assets/icons.ttf").as_slice())
                    .map(Message::FontLoaded),
                Task::perform(async { initial_load() }, Message::ADBSatisfied),
                Task::perform(async { get_devices_list() }, |devices| {
                    Message::LoadDevices(0, devices)
                }),
                Task::perform(
                    async move { get_latest_release() },
                    Message::GetLatestRelease,
                ),
            ]),
        )
    }

    #[allow(
        clippy::unused_self,
        reason = "required by iced's Application trait interface"
    )]
    fn subscription(&self) -> Subscription<Message> {
        if self.selected_device.is_some() {
            event::listen_with(Self::keyboard_shortcuts_with_device)
        } else {
            event::listen_with(Self::keyboard_shortcuts_without_device)
        }
    }

    fn keyboard_shortcuts_with_device(
        event: iced::Event,
        _status: iced::event::Status,
        _window: iced::window::Id,
    ) -> Option<Message> {
        Self::keyboard_shortcuts(event, true)
    }

    fn keyboard_shortcuts_without_device(
        event: iced::Event,
        _status: iced::event::Status,
        _window: iced::window::Id,
    ) -> Option<Message> {
        Self::keyboard_shortcuts(event, false)
    }

    fn keyboard_shortcuts(event: iced::Event, can_control_device: bool) -> Option<Message> {
        match event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character(c),
                modifiers,
                ..
            }) if modifiers.control() && modifiers.shift() => match c.as_str() {
                "r" if can_control_device => Some(Message::RebootButtonPressed),
                "5" if can_control_device => Some(Message::RefreshButtonPressed),
                "a" => Some(Message::AppsPress),
                "i" => Some(Message::AboutPressed),
                "s" => Some(Message::SettingsPressed),
                _ => None,
            },
            _ => None,
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Root GUI update orchestrates many flows"
    )]
    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::LoadDevices(id, _) if id != self.discovery_id => Task::none(),
            Message::LoadDevices(_, devices_list) => {
                let previous = self.selected_device.take();
                self.selected_device = match &previous {
                    Some(s_device) => {
                        // Try to reload last selected phone
                        devices_list
                            .iter()
                            .find(|phone| phone.adb_id == s_device.adb_id)
                            .cloned()
                    }
                    None => devices_list.first().cloned(),
                };
                self.devices_list = devices_list;

                #[expect(unused_must_use, reason = "side-effect")]
                {
                    self.update(Message::SettingsAction(SettingsMessage::LoadDeviceSettings));
                }

                self.apps_view
                    .load_uad_list(true, &self.selected_device.clone().unwrap_or_default())
                    .map(Message::AppsAction)
            }
            Message::AppsPress => {
                self.view = View::List;
                Task::none()
            }
            Message::AboutPressed => {
                self.view = View::About;
                if self.update_state.self_update.status == SelfUpdateStatus::Updating {
                    return Task::none();
                }
                self.update_state.self_update = SelfUpdateState::default();
                Task::perform(
                    async move { get_latest_release() },
                    Message::GetLatestRelease,
                )
            }
            Message::SettingsPressed => {
                self.view = View::Settings;
                Task::none()
            }
            Message::RefreshButtonPressed => {
                self.apps_view.reset_inventory();
                self.discovery_id += 1;
                let id = self.discovery_id;
                Task::perform(async { get_devices_list() }, move |devices| {
                    Message::LoadDevices(id, devices)
                })
            }
            Message::RebootButtonPressed => {
                let Some(phone) = self.selected_device.take() else {
                    return Task::none();
                };
                self.discovery_id += 1;
                self.apps_view.reset_package_rows();
                self.devices_list.clear();
                Task::perform(
                    async move { adb::ACommand::new().shell(phone.adb_id).reboot() },
                    |_| Message::Nothing,
                )
            }
            Message::AppsAction(msg) => self
                .apps_view
                .update(
                    &mut self.settings_view,
                    &mut self.selected_device.clone().unwrap_or_default(),
                    &mut self.update_state.uad_list,
                    msg,
                )
                .map(Message::AppsAction),
            Message::SettingsAction(SettingsMessage::DeviceBackedUp(id, origin, result)) => {
                let displayed = self.selected_device.clone().unwrap_or_default();
                self.settings_view
                    .finish_backup(&origin.adb_id, &displayed, result);
                if displayed.adb_id != origin.adb_id {
                    return Task::none();
                }
                // A later device refresh owns the profile list if it has already started.
                if id == self.apps_view.load_id && origin.user_list != displayed.user_list {
                    if let Some(entry) = self
                        .devices_list
                        .iter_mut()
                        .find(|p| p.adb_id == origin.adb_id)
                    {
                        entry.clone_from(&origin);
                    }
                    self.selected_device = Some(origin);
                    return self.reload_packages();
                }
                self.apps_view
                    .invalidate_inventory(&displayed)
                    .map(Message::AppsAction)
            }
            Message::SettingsAction(msg) => {
                match msg {
                    SettingsMessage::RestorePackageFinished(ref result) => {
                        if self.settings_view.finish_restore_package(result)
                            && self.selected_device.is_some()
                        {
                            return self.reload_packages();
                        }
                        return Task::none();
                    }
                    SettingsMessage::MultiUserMode(toggled) if toggled => {
                        if let Some(device) = &self.selected_device {
                            self.apps_view.select_for_all_users(
                                &device.user_list,
                                self.settings_view.general.expert_mode,
                            );
                        }
                    }
                    SettingsMessage::ExpertMode(false) => self.apps_view.deselect_unsafe(),
                    _ => (),
                }
                let device = self.selected_device.clone().unwrap_or_default();
                let restore = matches!(msg, SettingsMessage::RestoreDevice);
                let previous_mode = self.settings_view.device.disable_mode;
                let plan_for_displayed = matches!(&msg, SettingsMessage::RestorePlanned(phone, _) if phone.adb_id == device.adb_id);
                let task = self.settings_view.update(&device, &self.apps_view, msg);
                if previous_mode != self.settings_view.device.disable_mode {
                    self.apps_view
                        .deselect_changed_retries(self.settings_view.device.disable_mode);
                }
                // The package list shows the progress, then the restored packages.
                if restore && self.settings_view.restoring(&device).is_some() {
                    self.view = View::List;
                } else if plan_for_displayed && self.settings_view.restoring(&device).is_none() {
                    if matches!(self.view, View::List) {
                        self.view = View::Settings;
                    }
                    return self.reload_packages();
                }
                task.map(Message::SettingsAction)
            }
            Message::AboutAction(msg) => {
                self.about_view.update(msg.clone());

                match msg {
                    AboutMessage::UpdateUadLists => {
                        self.update_state.uad_list = UadListState::Downloading;
                        self.apps_view.loading_state = ListLoadingState::DownloadingList;
                        self.update(Message::AppsAction(AppsMessage::LoadUadList(true)))
                    }
                    AboutMessage::DoSelfUpdate => {
                        // A second update would replace the backup of the first one.
                        #[cfg(feature = "self-update")]
                        if let Some(release) =
                            self.update_state.self_update.available_release().cloned()
                        {
                            self.update_state.self_update.status = SelfUpdateStatus::Updating;
                            self.apps_view.loading_state = ListLoadingState::UpdatingUad;
                            Task::perform(
                                download_update_to_temp_file(BIN_NAME, release),
                                Message::NewReleaseDownloaded,
                            )
                        } else {
                            Task::none()
                        }
                        #[cfg(not(feature = "self-update"))]
                        Task::none()
                    }
                    AboutMessage::UrlPressed(_) => Task::none(),
                }
            }
            Message::DeviceSelected(s_device) => {
                self.discovery_id += 1;
                self.selected_device = Some(s_device.clone());
                self.view = View::List;
                info!("{:-^65}", "-");
                info!(
                    "ANDROID_SDK: {} | DEVICE: {}",
                    s_device.android_sdk, s_device.model
                );
                info!("{:-^65}", "-");
                #[expect(unused_must_use, reason = "side-effects")]
                {
                    self.update(Message::SettingsAction(SettingsMessage::LoadDeviceSettings));
                }
                self.reload_packages()
            }
            #[cfg(feature = "self-update")]
            Message::NewReleaseDownloaded(res) => {
                debug!("{NAME} update has been downloaded!");

                if let Ok((relaunch_path, cleanup_path)) = res {
                    let mut args: Vec<_> = std::env::args().skip(1).collect();

                    // Remove the `--self-update-temp` arg from args if it exists,
                    // since we need to pass it cleanly. Otherwise new process will
                    // fail during arg parsing.
                    if let Some(idx) = args.iter().position(|a| a == "--self-update-temp") {
                        args.remove(idx);
                        // Remove path passed after this arg
                        if idx < args.len() {
                            args.remove(idx);
                        }
                    }

                    match std::process::Command::new(&relaunch_path)
                        .args(args)
                        .arg("--self-update-temp")
                        .arg(&cleanup_path)
                        .spawn()
                    {
                        Ok(_) => {
                            // Windows cannot delete the executable of a running process:
                            // the next update replaces this backup.
                            if !cfg!(target_os = "windows")
                                && let Err(e) = remove_file(cleanup_path)
                            {
                                error!("Could not remove temp update file: {e}");
                            }
                            std::process::exit(0)
                        }
                        Err(error) => {
                            if let Err(e) = rename(&cleanup_path, &relaunch_path) {
                                error!(
                                    "Could not restore executable: {e}. Original saved at {}",
                                    cleanup_path.display()
                                );
                            }
                            error!("Failed to update {NAME}: {error}");
                            self.update_state.self_update.status = SelfUpdateStatus::Failed;
                            return self.update(Message::AppsAction(AppsMessage::UpdateFailed));
                        }
                    }
                }
                error!("Failed to update {NAME}!");
                self.update_state.self_update.status = SelfUpdateStatus::Failed;
                self.update(Message::AppsAction(AppsMessage::UpdateFailed))
            }
            Message::GetLatestRelease(_)
                if self.update_state.self_update.status == SelfUpdateStatus::Updating =>
            {
                Task::none()
            }
            Message::GetLatestRelease(release) => {
                match release {
                    Ok(r) => {
                        self.update_state.self_update.status = SelfUpdateStatus::Done;
                        self.update_state.self_update.latest_release = r;
                    }
                    Err(()) => self.update_state.self_update.status = SelfUpdateStatus::Failed,
                }
                Task::none()
            }
            Message::FontLoaded(result) => {
                if let Err(error) = result {
                    error!("Couldn't load font: {error:?}");
                }

                Task::none()
            }
            Message::ADBSatisfied(result) => {
                self.update(Message::AppsAction(AppsMessage::ADBSatisfied(result)))
            }
            Message::Nothing => Task::none(),
        }
    }

    fn reload_packages(&mut self) -> Task<Message> {
        self.apps_view.reset_package_rows();
        if self.apps_view.uad_lists.is_empty() {
            return self
                .apps_view
                .load_uad_list(true, &self.selected_device.clone().unwrap_or_default())
                .map(Message::AppsAction);
        }
        self.update(Message::AppsAction(AppsMessage::LoadPhonePackages(
            self.apps_view.load_id,
            (self.apps_view.uad_lists.clone(), self.update_state.uad_list),
        )))
    }

    fn view(&self) -> Element<'_, Message, Theme> {
        let navigation_container = nav_menu(
            &self.devices_list,
            self.selected_device.clone(),
            &self.apps_view,
            &self.update_state.self_update,
        );

        let selected_device = self.selected_device.clone().unwrap_or_default();
        let main_container = match self.view {
            View::List => self
                .apps_view
                .view(&self.settings_view, &selected_device)
                .map(Message::AppsAction),
            View::About => self
                .about_view
                .view(&self.update_state)
                .map(Message::AboutAction),
            View::Settings => self
                .settings_view
                .view(&selected_device, &self.apps_view)
                .map(Message::SettingsAction),
        };

        column![navigation_container, main_container]
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .into()
    }

    fn theme(&self) -> Theme {
        string_to_theme(&self.settings_view.general.theme)
    }
}

impl UadGui {
    pub fn start() -> iced::Result {
        #[cfg(feature = "img")]
        let logo: &[u8] = match *OS_COLOR_SCHEME {
            // remember to keep `Unspecified` in sync with `src/theme`
            dark_light::Mode::Dark | dark_light::Mode::Unspecified => {
                include_bytes!("../../../resources/assets/logo-dark.png")
            }
            dark_light::Mode::Light => {
                include_bytes!("../../../resources/assets/logo-light.png")
            }
        };

        iced::application(UadGui::new, UadGui::update, UadGui::view)
            .title(UadGui::title)
            .theme(UadGui::theme)
            .subscription(UadGui::subscription)
            .settings(Settings {
                id: Some(String::from(NAME)),
                default_text_size: iced::Pixels(16.0),
                // HACK: avoid glyph clipping on some systems. Details:
                // https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/pull/1337
                #[cfg(target_os = "linux")]
                default_font: iced::Font {
                    family: iced::font::Family::Name("Adwaita Sans"),
                    ..Default::default()
                },
                ..Settings::default()
            })
            .window(Window {
                size: iced::Size {
                    width: 950.0,
                    height: 700.0,
                },
                resizable: true,
                decorations: true,
                #[cfg(feature = "img")]
                icon: icon::from_file_data(logo, Some(ImageFormat::Png)).ok(),
                ..iced::window::Settings::default()
            })
            .run()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::package_row::PackageRow;
    use uad_core::{
        sync::User,
        uad_lists::{PackageState, Removal, UadList},
    };

    fn phone(serial: &str) -> Phone {
        Phone {
            adb_id: serial.to_owned(),
            android_sdk: 35,
            user_list: vec![User::default()],
            ..Phone::default()
        }
    }

    fn inventory(id: u64, serial: &str, state: PackageState) -> Message {
        let row = PackageRow::new(
            "com.example.app",
            "",
            Removal::Recommended,
            state,
            UadList::Misc,
            false,
            false,
        );
        Message::AppsAction(AppsMessage::ApplyFilters(
            id,
            serial.into(),
            vec![Ok(vec![row])],
        ))
    }

    /// Select a phone as `DeviceSelected` does, without reading the saved settings.
    fn select(gui: &mut UadGui, serial: &str) {
        gui.selected_device = Some(phone(serial));
        gui.settings_view.device.device_id = serial.to_owned();
        let _ = gui.reload_packages();
    }

    #[test]
    fn settings_waits_for_the_current_inventory_after_switching_devices() {
        let mut gui = UadGui::default();
        select(&mut gui, "A");
        let old = gui.apps_view.load_id;
        let _ = gui.update(inventory(old, "A", PackageState::Enabled));
        assert!(gui.apps_view.inventory_ready(&phone("A")));

        select(&mut gui, "B");
        assert!(gui.apps_view.phone_packages.is_empty());
        for action in [
            SettingsMessage::BackupDevice,
            SettingsMessage::RestoreDevice,
            SettingsMessage::ExportPackages,
        ] {
            assert_eq!(gui.update(Message::SettingsAction(action)).units(), 0);
        }
        select(&mut gui, "A");
        let latest = gui.apps_view.load_id;
        let _ = gui.update(inventory(latest, "A", PackageState::Disabled));
        let _ = gui.update(inventory(old, "A", PackageState::Enabled));
        assert_eq!(
            gui.apps_view.phone_packages[0][0].state,
            PackageState::Disabled
        );
        assert_eq!(
            gui.update(Message::SettingsAction(SettingsMessage::BackupDevice))
                .units(),
            1
        );
    }

    #[test]
    fn backup_completion_waits_for_fresh_rows_before_unlocking_actions() {
        use crate::widgets::package_row::Message::ActionPressed;
        let mut gui = UadGui::default();
        select(&mut gui, "A");
        let id = gui.apps_view.load_id;
        let _ = gui.update(inventory(id, "A", PackageState::Enabled));
        let _ = gui.update(Message::SettingsAction(SettingsMessage::BackupDevice));
        let task = gui.update(Message::SettingsAction(SettingsMessage::DeviceBackedUp(
            id,
            phone("A"),
            Ok("saved.json".into()),
        )));
        assert_eq!(task.units(), 1);
        assert!(!gui.apps_view.inventory_current(&phone("A")));
        assert_eq!(
            gui.update(Message::AppsAction(AppsMessage::List(0, ActionPressed)))
                .units(),
            0
        );
        let _ = gui.update(inventory(id, "A", PackageState::Enabled));
        assert!(!gui.apps_view.inventory_current(&phone("A")));
        let _ = gui.update(inventory(
            gui.apps_view.load_id,
            "A",
            PackageState::Disabled,
        ));
        assert_eq!(
            gui.apps_view.phone_packages[0][0].state,
            PackageState::Disabled
        );
        assert!(gui.apps_view.inventory_current(&phone("A")));
    }

    #[test]
    fn reboot_requires_a_selected_phone() {
        let mut gui = UadGui::default();
        gui.devices_list.push(phone("B"));
        assert_eq!(gui.update(Message::RebootButtonPressed).units(), 0);
        assert_eq!(gui.devices_list[0].adb_id, "B");
        gui.selected_device = Some(phone("B"));
        assert_eq!(gui.update(Message::RebootButtonPressed).units(), 1);
    }

    #[test]
    fn restore_waits_for_actions_on_its_phone_and_the_next_inventory() {
        use crate::views::list::{PackageInfo, VerifyAndFallbackResult};
        use crate::widgets::package_row::Message::{ActionPressed, ToggleSelection};
        let mut gui = UadGui::default();
        select(&mut gui, "A");
        let _ = gui.update(inventory(gui.apps_view.load_id, "A", PackageState::Enabled));
        let mut second = gui.apps_view.phone_packages[0][0].clone();
        second.name = "com.example.second".into();
        gui.apps_view.phone_packages[0].push(second);
        for index in 0..2 {
            let _ = gui.update(Message::AppsAction(AppsMessage::List(
                index,
                ToggleSelection(true),
            )));
        }
        assert_eq!(
            gui.update(Message::AppsAction(AppsMessage::ModalValidate))
                .units(),
            2
        );
        assert_eq!(
            gui.update(Message::AppsAction(AppsMessage::List(0, ActionPressed)))
                .units(),
            0
        );

        select(&mut gui, "B");
        let _ = gui.update(inventory(gui.apps_view.load_id, "B", PackageState::Enabled));
        assert!(gui.apps_view.inventory_current(&phone("B")));
        select(&mut gui, "A");
        let _ = gui.update(inventory(gui.apps_view.load_id, "A", PackageState::Enabled));
        let _ = gui.update(Message::SettingsPressed);
        let done = Message::AppsAction(AppsMessage::VerifyAndFallbackFinished(
            VerifyAndFallbackResult {
                info: PackageInfo {
                    device: phone("A"),
                    name: "com.example.app".into(),
                    ..PackageInfo::default()
                },
                new_state: Some(PackageState::Disabled),
                ran: true,
                notification: None,
                error_modal: None,
            },
        ));
        for settings_action in [
            SettingsMessage::BackupDevice,
            SettingsMessage::ExportPackages,
        ] {
            assert_eq!(
                gui.update(Message::SettingsAction(settings_action)).units(),
                0
            );
        }
        assert_eq!(gui.update(done.clone()).units(), 0);
        assert!(!gui.apps_view.inventory_current(&phone("A")));
        assert_eq!(gui.update(done).units(), 1);
        assert!(!gui.apps_view.inventory_current(&phone("A")));
        let _ = gui.update(inventory(
            gui.apps_view.load_id,
            "A",
            PackageState::Disabled,
        ));
        assert!(gui.apps_view.inventory_current(&phone("A")));
    }

    #[cfg(feature = "self-update")]
    #[test]
    fn an_install_survives_about_and_late_release_checks() {
        let mut gui = UadGui::default();
        let release = Release {
            tag_name: "v2.0.0".into(),
            assets: vec![],
        };
        let check = Message::GetLatestRelease(Ok(Some(release)));
        let install = Message::AboutAction(AboutMessage::DoSelfUpdate);
        let _ = gui.update(check.clone());
        assert_eq!(gui.update(install.clone()).units(), 1);
        assert_eq!(gui.update(Message::AboutPressed).units(), 0);
        let _ = gui.update(check);
        assert_eq!(gui.update(install.clone()).units(), 0);
        let _ = gui.update(Message::NewReleaseDownloaded(Err(())));
        assert_eq!(gui.update(install).units(), 1);
    }

    #[test]
    fn restore_blocks_backup_and_restore_then_reports_failures() {
        use uad_core::utils::DisplayablePath;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("backup.json");
        std::fs::write(
            &file,
            r#"{"device_id":"A","users":[{"id":0,"packages":[
            {"name":"com.example.app","description":"","state":"Disabled"},
            {"name":"com.example.missing","description":"","state":"Enabled"}
        ]}]}"#,
        )
        .unwrap();
        let mut gui = UadGui::default();
        select(&mut gui, "A");
        let _ = gui.update(inventory(
            gui.apps_view.load_id,
            "A",
            PackageState::Disabled,
        ));
        gui.settings_view.device.backup.selected = Some(DisplayablePath { path: file });
        let restore = Message::SettingsAction(SettingsMessage::RestoreDevice);
        gui.view = View::Settings;
        assert_eq!(gui.update(restore.clone()).units(), 1);
        assert!(matches!(gui.view, View::List));
        assert_eq!(gui.update(restore).units(), 0);
        let backup = Message::SettingsAction(SettingsMessage::BackupDevice);
        assert_eq!(gui.update(backup.clone()).units(), 0);
        assert_eq!(
            gui.update(Message::SettingsAction(SettingsMessage::ExportPackages))
                .units(),
            0
        );
        assert!(
            gui.settings_view
                .device
                .backup
                .backup_state
                .starts_with("Restoring")
        );
        // The cache matched the backup, but the fresh inventory needs a disable.
        let mut fresh = uad_core::sync::CorePackage::from(&gui.apps_view.phone_packages[0][0]);
        fresh.state = PackageState::Enabled;
        let plan =
            uad_core::save::restore_backup(&phone("A"), &[vec![fresh]], &gui.settings_view.device)
                .unwrap();
        assert_eq!(
            gui.update(Message::SettingsAction(SettingsMessage::RestorePlanned(
                phone("A"),
                Ok(plan)
            )))
            .units(),
            1
        );
        let error = "com.example.app (user 0): device offline";
        let done =
            Message::SettingsAction(SettingsMessage::RestorePackageFinished(Err(error.into())));
        assert_eq!(gui.update(done).units(), 1);
        let report = &gui.settings_view.device.backup.backup_state;
        assert!(report.contains(error));
        assert!(report.contains("Skipped 1"));

        // The restored packages can be backed up once they are loaded.
        let _ = gui.update(inventory(gui.apps_view.load_id, "A", PackageState::Enabled));
        assert_eq!(gui.update(backup).units(), 1);
    }
}

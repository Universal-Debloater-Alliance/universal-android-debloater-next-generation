pub mod style;
pub mod views;
pub mod widgets;

use crate::core::adb;
use crate::core::sync::{Phone, get_devices_list, initial_load};
use crate::core::theme::{OS_COLOR_SCHEME, string_to_theme, to_iced_theme};
use crate::core::uad_lists::UadListState;
use crate::core::update::{Release, SelfUpdateState, SelfUpdateStatus, get_latest_release};
use crate::core::utils::NAME;

use iced::font;
use views::about::{About as AboutView, Message as AboutMessage};
use views::list::{List as AppsView, LoadingState as ListLoadingState, Message as AppsMessage};
use views::settings::{Message as SettingsMessage, Settings as SettingsView};
use widgets::navigation_menu::nav_menu;

use iced::widget::column;
use iced::{Alignment, Element, Length, Task};
#[cfg(feature = "self-update")]
use std::path::PathBuf;

#[cfg(feature = "self-update")]
use crate::core::update::{BIN_NAME, download_update_to_temp_file, remove_file};

#[derive(Default, Debug, Clone)]
enum View {
    #[default]
    List,
    About,
    Settings,
}

#[derive(Default, Clone)]
pub struct UpdateState {
    self_update: SelfUpdateState,
    uad_list: UadListState,
}

#[derive(Default)]
pub struct UadGui {
    view: View,
    apps_view: AppsView,
    about_view: AboutView,
    settings_view: SettingsView,
    devices_list: Vec<Phone>,
    selected_device: Option<Phone>,
    fallback_phone: Phone,
    update_state: UpdateState,
    nb_running_async_adb_commands: u32,
    adb_satisfied: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    AboutPressed,
    SettingsPressed,
    AppsPress,
    DeviceSelected(Phone),
    AboutAction(AboutMessage),
    AppsAction(AppsMessage),
    SettingsAction(SettingsMessage),
    RefreshButtonPressed,
    RebootButtonPressed,
    LoadDevices(Vec<Phone>),
    #[cfg(feature = "self-update")]
    _NewReleaseDownloaded(Result<(PathBuf, PathBuf), ()>),
    GetLatestRelease(Result<Option<Release>, ()>),
    FontLoaded(Result<(), iced::font::Error>),
    Nothing,
    ADBSatisfied(bool),
}

impl UadGui {
    fn update_apps(&mut self, msg: AppsMessage) -> Task<Message> {
        let mut selected = self.selected_device.clone().unwrap_or_default();
        self.apps_view
            .update(
                &mut self.settings_view,
                &mut selected,
                &mut self.update_state.uad_list,
                msg,
            )
            .map(Message::AppsAction)
    }

    fn update_settings(&mut self, msg: SettingsMessage) -> Task<Message> {
        self.settings_view
            .update(
                &self.selected_device.clone().unwrap_or_default(),
                &self.apps_view.phone_packages,
                &mut self.nb_running_async_adb_commands,
                msg,
                self.apps_view.selected_user,
            )
            .map(Message::SettingsAction)
    }

    fn refresh(&mut self) -> Task<Message> {
        self.apps_view = AppsView::default();
        let adb_task = self.update_apps(AppsMessage::ADBSatisfied(self.adb_satisfied));
        let devices_task = Task::perform(get_devices_list(), Message::LoadDevices);
        Task::batch([adb_task, devices_task])
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::LoadDevices(devices_list) => {
                // Keep current selection if still present; else pick first
                let prev_id = self.selected_device.as_ref().map(|d| d.adb_id.clone());
                self.selected_device = prev_id
                    .and_then(|id| devices_list.iter().find(|p| p.adb_id == id).cloned())
                    .or_else(|| devices_list.first().cloned());
                self.devices_list = devices_list;

                let settings_task = self.update_settings(SettingsMessage::LoadDeviceSettings);
                let apps_task = self.update_apps(AppsMessage::LoadUadList(true));

                Task::batch([settings_task, apps_task])
            }
            Message::AppsPress => {
                self.view = View::List;
                Task::none()
            }
            Message::AboutPressed => {
                self.view = View::About;
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
            Message::RefreshButtonPressed => self.refresh(),
            Message::RebootButtonPressed => {
                self.apps_view = AppsView::default();
                let serial = self
                    .selected_device
                    .as_ref()
                    .map(|d| d.adb_id.clone())
                    .unwrap_or_default();
                self.selected_device = None;
                self.devices_list.clear();
                Task::perform(
                    async { adb::ACommand::new().shell(serial).reboot() },
                    |_| Message::Nothing,
                )
            }
            Message::AppsAction(msg) => self.update_apps(msg),
            Message::SettingsAction(msg) => {
                match msg {
                    SettingsMessage::RestoringDevice(ref output) => {
                        self.nb_running_async_adb_commands -= 1;
                        self.view = View::List;

                        let restore_task =
                            self.update_apps(AppsMessage::RestoringDevice(output.clone()));

                        if self.nb_running_async_adb_commands == 0 {
                            let refresh_task = self.refresh();
                            return Task::batch([restore_task, refresh_task]);
                        }
                    }
                    SettingsMessage::MultiUserMode(toggled) if toggled => {
                        // Collect indices selected in any user, then propagate
                        let pkg_len = self
                            .apps_view
                            .phone_packages
                            .iter()
                            .map(|v| v.len())
                            .min()
                            .unwrap_or(0);
                        if pkg_len > 0 {
                            let mut selected_any = vec![false; pkg_len];
                            for user_pkgs in &self.apps_view.phone_packages {
                                for (i, pkg) in user_pkgs.iter().take(pkg_len).enumerate() {
                                    if pkg.selected {
                                        selected_any[i] = true;
                                    }
                                }
                            }
                            for u in self
                                .selected_device
                                .as_ref()
                                .expect("Device should be selected")
                                .user_list
                                .iter()
                                .filter(|u| !u.protected)
                            {
                                if let Some(pkgs) = self.apps_view.phone_packages.get_mut(u.index) {
                                    for (i, pkg) in pkgs.iter_mut().take(pkg_len).enumerate() {
                                        if selected_any[i] {
                                            pkg.selected = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => (),
                }
                self.update_settings(msg)
            }
            Message::AboutAction(msg) => {
                self.about_view.update(msg.clone());

                match msg {
                    AboutMessage::UpdateUadLists => {
                        self.update_state.uad_list = UadListState::Downloading;
                        self.apps_view.loading_state = ListLoadingState::DownloadingList;
                        self.update_apps(AppsMessage::LoadUadList(true))
                    }
                    AboutMessage::DoSelfUpdate => {
                        #[cfg(feature = "self-update")]
                        if let Some(release) = self.update_state.self_update.latest_release.as_ref()
                        {
                            self.update_state.self_update.status = SelfUpdateStatus::Updating;
                            self.apps_view.loading_state = ListLoadingState::_UpdatingUad;
                            Task::perform(
                                download_update_to_temp_file(BIN_NAME, release.clone()),
                                Message::_NewReleaseDownloaded,
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
                info!("{:-^65}", "-");
                info!(
                    "ANDROID_SDK: {} | DEVICE: {}",
                    s_device.android_sdk, s_device.model
                );
                info!("{:-^65}", "-");
                self.selected_device = Some(s_device);
                self.view = View::List;
                self.apps_view.loading_state = ListLoadingState::FindingPhones;

                let settings_task = self.update_settings(SettingsMessage::LoadDeviceSettings);
                let toggle_task = self.update_apps(AppsMessage::ToggleAllSelected(false));
                let clear_task = self.update_apps(AppsMessage::ClearSelectedPackages);
                let load_task = self.update_apps(AppsMessage::LoadPhonePackages((
                    self.apps_view.uad_lists.clone(),
                    UadListState::Done,
                )));

                Task::batch([settings_task, toggle_task, clear_task, load_task])
            }
            #[cfg(feature = "self-update")]
            Message::_NewReleaseDownloaded(res) => {
                debug!("{NAME} update has been downloaded!");

                if let Ok((relaunch_path, cleanup_path)) = res {
                    let mut args: Vec<_> = std::env::args().skip(1).collect();

                    if let Some(idx) = args.iter().position(|a| a == "--self-update-temp") {
                        args.remove(idx);
                        args.remove(idx);
                    }

                    match std::process::Command::new(relaunch_path)
                        .args(args)
                        .arg("--self-update-temp")
                        .arg(&cleanup_path)
                        .spawn()
                    {
                        Ok(_) => {
                            if let Err(e) = remove_file(cleanup_path) {
                                error!("Could not remove temp update file: {e}");
                            }
                            std::process::exit(0)
                        }
                        Err(error) => {
                            if let Err(e) = remove_file(cleanup_path) {
                                error!("Could not remove temp update file: {e}");
                            }
                            error!("Failed to update {NAME}: {error}");
                            Task::none()
                        }
                    }
                } else {
                    error!("Failed to update {NAME}!");
                    let update_task = self.update(Message::AppsAction(AppsMessage::UpdateFailed));
                    self.update_state.self_update.status = SelfUpdateStatus::Failed;
                    update_task
                }
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
                self.adb_satisfied = result;
                self.update_apps(AppsMessage::ADBSatisfied(self.adb_satisfied))
            }
            Message::Nothing => Task::none(),
        }
    }

    fn view(&self) -> Element<Message> {
        let navigation_container = nav_menu(
            &self.devices_list,
            self.selected_device.as_ref(),
            &self.apps_view,
            &self.update_state.self_update,
        );

        // Borrow selected device or a fallback owned by `self`
        let selected_device = self
            .selected_device
            .as_ref()
            .unwrap_or(&self.fallback_phone);
        let main_container = match self.view {
            View::List => self
                .apps_view
                .view(&self.settings_view, selected_device)
                .map(Message::AppsAction),
            View::About => self
                .about_view
                .view(&self.update_state)
                .map(Message::AboutAction),
            View::Settings => self
                .settings_view
                .view(selected_device, &self.apps_view)
                .map(Message::SettingsAction),
        };

        column![navigation_container, main_container]
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .into()
    }

    pub fn start() -> iced::Result {
        let logo: &[u8] = match *OS_COLOR_SCHEME {
            dark_light::Mode::Dark | dark_light::Mode::Unspecified => {
                include_bytes!("../../resources/assets/logo-dark.png")
            }
            dark_light::Mode::Light => {
                include_bytes!("../../resources/assets/logo-light.png")
            }
        };

        iced::application(
            "Universal Android Debloater Next Generation",
            UadGui::update,
            UadGui::view,
        )
        .theme(|state: &UadGui| to_iced_theme(string_to_theme(&state.settings_view.general.theme)))
        .window(iced::window::Settings {
            size: iced::Size {
                width: 950.0,
                height: 700.0,
            },
            resizable: true,
            decorations: true,
            icon: iced::window::icon::from_file_data(
                logo,
                Some(iced::advanced::graphics::image::image_rs::ImageFormat::Png),
            )
            .ok(),
            ..Default::default()
        })
        .run_with(|| {
            (
                Self::default(),
                Task::batch([
                    font::load(include_bytes!("../../resources/assets/icons.ttf").as_slice())
                        .map(Message::FontLoaded),
                    Task::perform(initial_load(), Message::ADBSatisfied),
                    Task::perform(get_devices_list(), Message::LoadDevices),
                    Task::perform(
                        async move { get_latest_release() },
                        Message::GetLatestRelease,
                    ),
                ]),
            )
        })
    }
}

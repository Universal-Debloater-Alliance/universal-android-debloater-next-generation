pub mod style;
pub mod views;
pub mod widgets;

use crate::core::sync::{get_devices_list, initial_load, perform_adb_commands, CommandType, Phone};
use crate::core::theme::Theme;
use crate::core::uad_lists::UadListState;
use crate::core::update::{get_latest_release, Release, SelfUpdateState, SelfUpdateStatus};
use crate::core::utils::{set_adb_serial, string_to_theme, ANDROID_SERIAL, NAME};

use iced::advanced::graphics::image::image_rs::ImageFormat;
use iced::font;
use iced::window::icon;
use views::about::{About as AboutView, Message as AboutMessage};
use views::list::{List as AppsView, LoadingState as ListLoadingState, Message as AppsMessage};
use views::settings::{Message as SettingsMessage, Settings as SettingsView};
use widgets::navigation_menu::nav_menu;

use iced::widget::column;
use iced::{
    window::Settings as Window, Alignment, Application, Command, Element, Length, Renderer,
    Settings,
};
use std::env;
#[cfg(feature = "self-update")]
use std::path::PathBuf;

#[cfg(feature = "self-update")]
use crate::core::update::{bin_name, download_update_to_temp_file, remove_file};

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

#[derive(Default, Clone)]
pub struct UadGui {
    view: View,
    apps_view: AppsView,
    about_view: AboutView,
    settings_view: SettingsView,
    devices_list: Vec<Phone>,
    /// index of `devices_list`
    selected_device: Option<Phone>,
    update_state: UpdateState,
    nb_running_async_adb_commands: u32,
    adb_satisfied: bool,
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
    LoadDevices(Vec<Phone>),
    #[cfg(feature = "self-update")]
    _NewReleaseDownloaded(Result<(PathBuf, PathBuf), ()>),
    GetLatestRelease(Result<Option<Release>, ()>),
    FontLoaded(Result<(), iced::font::Error>),
    Nothing,
    ADBSatisfied(bool),
}

impl Application for UadGui {
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self::default(),
            Command::batch([
                // Used in crate::gui::widgets::navigation_menu::ICONS. Name is `icomoon`.
                font::load(include_bytes!("../../resources/assets/icons.ttf").as_slice())
                    .map(Message::FontLoaded),
                Command::perform(initial_load(), Message::ADBSatisfied),
                Command::perform(get_devices_list(), Message::LoadDevices),
                Command::perform(
                    async move { get_latest_release() },
                    Message::GetLatestRelease,
                ),
            ]),
        )
    }

    fn theme(&self) -> Theme {
        string_to_theme(&self.settings_view.general.theme)
    }

    fn title(&self) -> String {
        String::from("Universal Android Debloater Next Generation")
    }
    // TODO: refactor later
    #[allow(clippy::too_many_lines)]
    fn update(&mut self, msg: Message) -> Command<Message> {
        match msg {
            #[allow(clippy::option_if_let_else)]
            Message::LoadDevices(devices_list) => {
                self.selected_device = match &self.selected_device {
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

                #[allow(unused_must_use)]
                {
                    self.update(Message::SettingsAction(SettingsMessage::LoadDeviceSettings));
                }

                self.update(Message::AppsAction(AppsMessage::LoadUadList(true)))
            }
            Message::AppsPress => {
                self.view = View::List;
                Command::none()
            }
            Message::AboutPressed => {
                self.view = View::About;
                self.update_state.self_update = SelfUpdateState::default();
                Command::perform(
                    async move { get_latest_release() },
                    Message::GetLatestRelease,
                )
            }
            Message::SettingsPressed => {
                self.view = View::Settings;
                Command::none()
            }
            Message::RefreshButtonPressed => {
                self.apps_view = AppsView::default();
                #[allow(unused_must_use)]
                {
                    self.update(Message::AppsAction(AppsMessage::ADBSatisfied(
                        self.adb_satisfied,
                    )));
                }
                Command::perform(get_devices_list(), Message::LoadDevices)
            }
            Message::RebootButtonPressed => {
                self.apps_view = AppsView::default();
                self.selected_device = None;
                self.devices_list = vec![];
                Command::perform(
                    perform_adb_commands("reboot".to_string(), CommandType::Shell),
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
            Message::SettingsAction(msg) => {
                match msg {
                    SettingsMessage::RestoringDevice(ref output) => {
                        self.nb_running_async_adb_commands -= 1;
                        self.view = View::List;

                        #[allow(unused_must_use)]
                        {
                            self.apps_view.update(
                                &mut self.settings_view,
                                &mut self.selected_device.clone().unwrap_or_default(),
                                &mut self.update_state.uad_list,
                                AppsMessage::RestoringDevice(output.clone()),
                            );
                        }
                        if self.nb_running_async_adb_commands == 0 {
                            return self.update(Message::RefreshButtonPressed);
                        }
                    }
                    SettingsMessage::MultiUserMode(toggled) if toggled => {
                        for user in self.apps_view.phone_packages.clone() {
                            for (i, _) in user.iter().filter(|&pkg| pkg.selected).enumerate() {
                                for u in self
                                    .selected_device
                                    .as_ref()
                                    .expect("Device should be selected")
                                    .user_list
                                    .iter()
                                    .filter(|&u| !u.protected)
                                {
                                    self.apps_view.phone_packages[u.index][i].selected = true;
                                }
                            }
                        }
                    }
                    _ => (),
                }
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
            Message::AboutAction(msg) => {
                self.about_view.update(msg.clone());

                match msg {
                    AboutMessage::UpdateUadLists => {
                        self.update_state.uad_list = UadListState::Downloading;
                        self.apps_view.loading_state = ListLoadingState::DownloadingList;
                        self.update(Message::AppsAction(AppsMessage::LoadUadList(true)))
                    }
                    AboutMessage::DoSelfUpdate => {
                        #[cfg(feature = "self-update")]
                        if let Some(release) = self.update_state.self_update.latest_release.as_ref()
                        {
                            self.update_state.self_update.status = SelfUpdateStatus::Updating;
                            self.apps_view.loading_state = ListLoadingState::_UpdatingUad;
                            let bin_name = bin_name().to_owned();
                            Command::perform(
                                download_update_to_temp_file(bin_name, release.clone()),
                                Message::_NewReleaseDownloaded,
                            )
                        } else {
                            Command::none()
                        }
                        #[cfg(not(feature = "self-update"))]
                        Command::none()
                    }
                    AboutMessage::UrlPressed(_) => Command::none(),
                }
            }
            Message::DeviceSelected(s_device) => {
                self.selected_device = Some(s_device.clone());
                self.view = View::List;
                #[allow(unsafe_code)]
                unsafe {
                    set_adb_serial(s_device.adb_id)
                };
                info!("{:-^65}", "-");
                info!(
                    "ANDROID_SDK: {} | DEVICE: {}",
                    s_device.android_sdk, s_device.model
                );
                info!("{:-^65}", "-");
                self.apps_view.loading_state = ListLoadingState::FindingPhones;

                #[allow(unused_must_use)]
                {
                    self.update(Message::SettingsAction(SettingsMessage::LoadDeviceSettings));
                    self.update(Message::AppsAction(AppsMessage::ToggleAllSelected(false)));
                    self.update(Message::AppsAction(AppsMessage::ClearSelectedPackages));
                }
                self.update(Message::AppsAction(AppsMessage::LoadPhonePackages((
                    self.apps_view.uad_lists.clone(),
                    UadListState::Done,
                ))))
            }
            #[cfg(feature = "self-update")]
            Message::_NewReleaseDownloaded(res) => {
                debug!("{NAME} update has been downloaded!");

                if let Ok((relaunch_path, cleanup_path)) = res {
                    let mut args: Vec<_> = std::env::args().skip(1).collect();

                    // Remove the `--self-update-temp` arg from args if it exists,
                    // since we need to pass it cleanly. Otherwise new process will
                    // fail during arg parsing.
                    if let Some(idx) = args.iter().position(|a| a == "--self-update-temp") {
                        args.remove(idx);
                        // Remove path passed after this arg
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
                                error!("Could not remove temp update file: {}", e);
                            }
                            std::process::exit(0)
                        }
                        Err(error) => {
                            if let Err(e) = remove_file(cleanup_path) {
                                error!("Could not remove temp update file: {}", e);
                            }
                            error!("Failed to update {NAME}: {}", error);
                        }
                    }
                } else {
                    error!("Failed to update {NAME}!");
                    #[allow(unused_must_use)]
                    {
                        self.update(Message::AppsAction(AppsMessage::UpdateFailed));
                        self.update_state.self_update.status = SelfUpdateStatus::Failed;
                    }
                }
                Command::none()
            }
            Message::GetLatestRelease(release) => {
                match release {
                    Ok(r) => {
                        self.update_state.self_update.status = SelfUpdateStatus::Done;
                        self.update_state.self_update.latest_release = r;
                    }
                    Err(()) => self.update_state.self_update.status = SelfUpdateStatus::Failed,
                };
                Command::none()
            }
            Message::FontLoaded(result) => {
                if let Err(error) = result {
                    error!("Couldn't load font: {error:?}");
                }

                Command::none()
            }
            Message::ADBSatisfied(result) => {
                self.adb_satisfied = result;
                self.update(Message::AppsAction(AppsMessage::ADBSatisfied(
                    self.adb_satisfied,
                )))
            }
            Message::Nothing => Command::none(),
        }
    }

    fn view(&self) -> Element<Self::Message, Self::Theme, Renderer> {
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
            .align_items(Alignment::Center)
            .into()
    }
}

impl UadGui {
    pub fn start() -> iced::Result {
        let logo: &[u8] = match dark_light::detect() {
            dark_light::Mode::Dark => include_bytes!("../../resources/assets/logo-dark.png"),
            dark_light::Mode::Light => include_bytes!("../../resources/assets/logo-light.png"),
            dark_light::Mode::Default => include_bytes!("../../resources/assets/logo-light.png"),
        };

        Self::run(Settings {
            id: Some(String::from(NAME)),
            window: Window {
                size: iced::Size {
                    width: 950.0,
                    height: 700.0,
                },
                resizable: true,
                decorations: true,
                icon: icon::from_file_data(logo, Some(ImageFormat::Png)).ok(),
                ..iced::window::Settings::default()
            },
            default_text_size: iced::Pixels(16.0),
            ..Settings::default()
        })
    }
}

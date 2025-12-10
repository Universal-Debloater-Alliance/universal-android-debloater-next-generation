use crate::CACHE_DIR;
use crate::core::adb;
use crate::core::helpers::button_primary;
use crate::core::theme::Theme;
use crate::core::uad_lists::LIST_FNAME;
use crate::core::utils::{NAME, last_modified_date, open_url};
use crate::gui::{UpdateState, style, widgets::text};
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length, Renderer};
use std::path::PathBuf;

#[cfg(feature = "self-update")]
use crate::core::update::SelfUpdateStatus;

#[derive(Default, Debug, Clone)]
pub struct About {}

#[derive(Debug, Clone)]
pub enum Message {
    UrlPressed(PathBuf),
    UpdateUadLists,
    DoSelfUpdate,
}

impl About {
    #[allow(
        clippy::unused_self,
        reason = "Trait-like shape required by GUI architecture"
    )]
    pub fn update(&mut self, msg: Message) {
        if let Message::UrlPressed(url) = msg {
            open_url(url);
        }
        // other events are handled by UadGui update()
    }
    #[allow(
        clippy::unused_self,
        reason = "Trait-like shape required by GUI architecture"
    )]
    pub fn view(&self, update_state: &UpdateState) -> Element<'_, Message, Theme, Renderer> {
        let about_text = text(format!(
            "Universal Android Debloater Next Generation ({NAME}) is a free and open-source community project \naiming at simplifying the removal of pre-installed apps on any Android device."
        ));

        let descr_container = container(about_text)
            .width(Length::Fill)
            .padding(25)
            .style(style::Container::Frame);

        let date = last_modified_date(CACHE_DIR.join(LIST_FNAME));
        let uad_list_text =
            text(format!("{NAME} package list: v{}", date.format("%Y%m%d"))).width(250);
        let last_update_text = text(update_state.uad_list.to_string());
        let uad_lists_btn = button_primary("Update").on_press(Message::UpdateUadLists);

        #[cfg(feature = "self-update")]
        let self_update_row = {
            let self_update_btn = button_primary("Update").on_press(Message::DoSelfUpdate);

            let uad_version_text =
                text(format!("{NAME} version: v{}", env!("CARGO_PKG_VERSION"))).width(250);

            let self_update_text = update_state
                .self_update
                .latest_release
                .as_ref()
                .map_or_else(
                    || {
                        if update_state.self_update.status == SelfUpdateStatus::Done {
                            "(No update available)".to_string()
                        } else {
                            update_state.self_update.status.to_string()
                        }
                    },
                    |r| {
                        if update_state.self_update.status == SelfUpdateStatus::Updating {
                            update_state.self_update.status.to_string()
                        } else {
                            format!("({} available)", r.tag_name)
                        }
                    },
                );

            let last_self_update_text = text(self_update_text).style(style::Text::Default);

            row![uad_version_text, self_update_btn, last_self_update_text,]
                .align_y(Alignment::Center)
                .spacing(10)
                .width(550)
        };

        let uad_list_row = row![uad_list_text, uad_lists_btn, last_update_text,]
            .align_y(Alignment::Center)
            .spacing(10)
            .width(550);

        /*
        There's no need to fetch this info every time the view is updated,
        we could cache it in a `static` `LazyLock`.

        But what if the system updates ADB while the app is running?
        the numbers will be out of sync!

        However, the server will still be the "old" version
        until it's killed
        */
        let adb_version_text = text(match adb::ACommand::new().version() {
            Ok(s) => s
                .lines()
                .nth(0)
                .unwrap_or_else(|| unreachable!())
                // This allocation is good.
                // If it was a ref, the app would hold the entire string
                // instead of the relevant slice.
                .to_string(),
            Err(e) => {
                error!("{e}");
                "Couldn't fetch ADB version. Is it installed?".into()
                // satisfy `match` by inferring the type of the `Ok` arm
            }
        })
        .width(250);
        let adb_version_row = row![adb_version_text].align_y(Alignment::Center).width(550);

        #[cfg(feature = "self-update")]
        let update_column = column![uad_list_row, self_update_row, adb_version_row];
        #[cfg(not(feature = "self-update"))]
        let update_column = column![uad_list_row, adb_version_row];

        let update_column = update_column.align_x(Alignment::Center).spacing(10);

        let update_container = container(update_column)
            .width(Length::Fill)
            .center_x(Length::Fill)
            .padding(10)
            .style(style::Container::Frame);

        let website_btn =
            button_primary("GitHub page").on_press(Message::UrlPressed(PathBuf::from(
                "https://github.com/Universal-Debloater-Alliance/universal-android-debloater",
            )));

        let issue_btn = button_primary("Have an issue?")
            .on_press(Message::UrlPressed(PathBuf::from(
            "https://github.com/Universal-Debloater-Alliance/universal-android-debloater/issues",
        )));

        let log_btn = button_primary("Locate the logfiles")
            .on_press(Message::UrlPressed(CACHE_DIR.to_path_buf()));

        let wiki_btn = button_primary("Wiki").on_press(Message::UrlPressed(PathBuf::from(
            "https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki",
        )));

        let row = row![website_btn, wiki_btn, issue_btn, log_btn,].spacing(20);

        let content = column![
            Space::new().width(Length::Fill).height(Length::Shrink),
            descr_container,
            update_container,
            row,
        ]
        .width(Length::Fill)
        .spacing(20)
        .align_x(Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .into()
    }
}

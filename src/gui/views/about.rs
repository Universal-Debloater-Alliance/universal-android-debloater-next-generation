use crate::CACHE_DIR;
use crate::core::adb;
use crate::core::helpers::button_primary;
use crate::core::uad_lists::LIST_FNAME;
use crate::core::utils::{NAME, last_modified_date, open_url};
use crate::gui::{UpdateState, style, widgets::text};
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length};
use std::path::PathBuf;

#[cfg(feature = "self-update")]
use crate::core::update::SelfUpdateStatus;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const ROW_WIDTH: u16 = 550;
const TEXT_WIDTH: u16 = 250;

#[derive(Default, Debug, Clone)]
pub struct About {}

#[derive(Debug, Clone)]
pub enum Message {
    UrlPressed(PathBuf),
    UpdateUadLists,
    DoSelfUpdate,
}

impl About {
    pub fn update(&mut self, msg: Message) {
        if let Message::UrlPressed(url) = msg {
            open_url(url);
        }
        // other events are handled by UadGui update()
    }

    pub fn view(&self, update_state: &UpdateState) -> Element<Message> {
        let about_text = text(format!(
            "Universal Android Debloater Next Generation ({NAME}) is a free \
             and open-source community project \naiming at simplifying the \
             removal of pre-installed apps on any Android device."
        ));

        let descr_container = container(about_text)
            .width(Length::Fill)
            .padding(25)
            .style(style::frame_container());

        let date = last_modified_date(CACHE_DIR.join(LIST_FNAME));
        let uad_list_text =
            text(format!("{NAME} package list: v{}", date.format("%Y%m%d"))).width(TEXT_WIDTH);
        let last_update_text = text(update_state.uad_list.to_string());
        let uad_lists_btn = button_primary("Update").on_press(Message::UpdateUadLists);

        #[cfg(feature = "self-update")]
        let self_update_row = {
            let self_update_btn = button_primary("Update").on_press(Message::DoSelfUpdate);

            let uad_version_text = text(format!("{NAME} version: v{VERSION}")).width(TEXT_WIDTH);

            let status = &update_state.self_update.status;
            let latest = update_state.self_update.latest_release.as_ref();
            let self_update_text = if let Some(r) = latest {
                if matches!(status, SelfUpdateStatus::Updating) {
                    status.to_string()
                } else {
                    format!("({} available)", r.tag_name)
                }
            } else if matches!(status, SelfUpdateStatus::Done) {
                "(No update available)".to_string()
            } else {
                status.to_string()
            };

            let last_self_update_text = text(self_update_text);

            row![uad_version_text, self_update_btn, last_self_update_text,]
                .align_y(Alignment::Center)
                .spacing(10)
                .width(ROW_WIDTH)
        };

        let uad_list_row = row![uad_list_text, uad_lists_btn, last_update_text,]
            .align_y(Alignment::Center)
            .spacing(10)
            .width(ROW_WIDTH);

        let adb_version_text = text(match adb::ACommand::new().version() {
            Ok(s) => s.lines().next().unwrap_or("").to_string(),
            Err(e) => {
                error!("{e}");
                "Couldn't fetch ADB version. Is it installed?".into()
            }
        })
        .width(TEXT_WIDTH);
        let adb_version_row = row![adb_version_text]
            .align_y(Alignment::Center)
            .width(ROW_WIDTH);

        #[cfg(feature = "self-update")]
        let update_column = column![uad_list_row, self_update_row, adb_version_row];
        #[cfg(not(feature = "self-update"))]
        let update_column = column![uad_list_row, adb_version_row];

        let update_column = update_column.align_x(Alignment::Center).spacing(10);

        let update_container = container(update_column)
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .padding(10)
            .style(style::frame_container());

        let link_btn = |label: &'static str, url: &'static str| {
            button_primary(label).on_press(Message::UrlPressed(PathBuf::from(url)))
        };

        let website_btn = link_btn(
            "GitHub page",
            "https://github.com/Universal-Debloater-Alliance/universal-android-debloater",
        );
        let issue_btn = link_btn(
            "Have an issue?",
            "https://github.com/Universal-Debloater-Alliance/universal-android-debloater/issues",
        );

        let log_btn = button_primary("Locate the logfiles")
            .on_press(Message::UrlPressed(CACHE_DIR.to_path_buf()));

        let wiki_btn = link_btn(
            "Wiki",
            "https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki",
        );

        let row = row![website_btn, wiki_btn, issue_btn, log_btn,].spacing(20);

        let content = column![
            Space::new(Length::Fill, Length::Shrink),
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

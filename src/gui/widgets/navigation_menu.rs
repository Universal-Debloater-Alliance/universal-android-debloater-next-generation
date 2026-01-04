use crate::core::helpers::button_primary;
pub use crate::core::sync::Phone;
use crate::core::theme::Theme;
use crate::core::update::{SelfUpdateState, SelfUpdateStatus};
pub use crate::gui::views::about::Message as AboutMessage;
pub use crate::gui::views::list::{List as AppsView, LoadingState as ListLoadingState};
use crate::gui::{Message, style, widgets::text};
use iced::widget::{Space, button, container, pick_list, row, tooltip};
use iced::{Alignment, Element, Font, Length, Renderer, alignment, font};
/// resources/assets/icons.ttf, loaded in [`crate::gui::UadGui`]
pub const ICONS: Font = Font {
    family: font::Family::Name("icomoon"),
    ..Font::DEFAULT
};

pub fn nav_menu<'a>(
    device_list: &'a [Phone],
    selected_device: Option<Phone>,
    apps_view: &AppsView,
    self_update_state: &SelfUpdateState,
) -> Element<'a, Message, Theme, Renderer> {
    let apps_refresh_btn = button_primary(
        text("\u{E900}")
            .font(ICONS)
            .width(22)
            .align_x(alignment::Horizontal::Center),
    )
    .on_press(Message::RefreshButtonPressed);

    let apps_refresh_tooltip = tooltip(
        apps_refresh_btn,
        "Refresh apps (Ctrl+Shift+5)",
        tooltip::Position::Bottom,
    )
    .style(style::Container::Tooltip)
    .gap(4);

    let reboot_btn = button_primary("Reboot").on_press(Message::RebootButtonPressed);

    let reboot_btn = tooltip(
        reboot_btn,
        "Reboot device (Ctrl+Shift+R)",
        tooltip::Position::Bottom,
    )
    .style(style::Container::Tooltip)
    .gap(4);

    let uad_version_text = if let Some(r) = &self_update_state.latest_release {
        match self_update_state.status {
            SelfUpdateStatus::Failed => text(format!("Failed to update to {}", r.tag_name)),
            SelfUpdateStatus::Checking => text(SelfUpdateStatus::Checking.to_string()),
            SelfUpdateStatus::Done => text(format!(
                "Update available: {} -> {}",
                env!("CARGO_PKG_VERSION"),
                r.tag_name
            )),
            SelfUpdateStatus::Updating => text("Updating please wait..."),
        }
    } else {
        text(format!("v{}", env!("CARGO_PKG_VERSION")))
    };

    let update_btn = if self_update_state.latest_release.is_some() {
        button("Update")
            .on_press(Message::AboutAction(AboutMessage::DoSelfUpdate))
            .padding([5, 10])
            .style(style::Button::SelfUpdate)
    } else {
        button("").height(0).width(0).style(style::Button::Hidden)
    };

    let apps_btn = button_primary("Apps").on_press(Message::AppsPress);

    let about_btn = button_primary("About").on_press(Message::AboutPressed);

    let settings_btn = button_primary(
        text("\u{E994}")
            .font(ICONS)
            .width(22)
            .align_x(alignment::Horizontal::Center),
    )
    .on_press(Message::SettingsPressed);

    let device_list_text = match apps_view.loading_state {
        ListLoadingState::FindingPhones => text("Finding connected devices..."),
        _ => text("No devices/emulators found"),
    };

    let row = match selected_device {
        Some(phone) => row![
            reboot_btn,
            apps_refresh_tooltip,
            pick_list(device_list, Some(phone), Message::DeviceSelected,),
            Space::new().width(Length::Fill).height(Length::Shrink),
            uad_version_text,
            update_btn,
            apps_btn,
            about_btn,
            settings_btn,
        ]
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .spacing(10),
        None => row![
            reboot_btn,
            apps_refresh_tooltip,
            device_list_text,
            Space::new().width(Length::Fill).height(Length::Shrink),
            uad_version_text,
            update_btn,
            apps_btn,
            about_btn,
            settings_btn,
        ]
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .spacing(10),
    };

    container(row)
        .width(Length::Fill)
        .padding(10)
        .style(style::Container::Frame)
        .into()
}

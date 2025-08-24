pub use crate::core::sync::Phone;
use crate::core::update::{SelfUpdateState, SelfUpdateStatus};
pub use crate::gui::views::about::Message as AboutMessage;
pub use crate::gui::views::list::{List as AppsView, LoadingState as ListLoadingState};
use crate::gui::{Message, style, widgets::text};
use iced::widget::{Space, button, container, pick_list, row, tooltip};
use iced::{Alignment, Element, Font, Length, alignment, font};

pub const ICONS: Font = Font {
    family: font::Family::Name("icomoon"),
    ..Font::DEFAULT
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn nav_menu<'a>(
    device_list: &'a [Phone],
    selected_device: Option<&'a Phone>,
    apps_view: &AppsView,
    self_update_state: &SelfUpdateState,
) -> Element<'a, Message> {
    let primary = style::primary_button();

    let icon_text = |ch: char| {
        text(ch.to_string())
            .font(ICONS)
            .width(22)
            .align_x(alignment::Horizontal::Center)
    };

    let apps_refresh_btn = button(icon_text('\u{E900}'))
        .style(primary)
        .on_press(Message::RefreshButtonPressed);

    let apps_refresh_tooltip = tooltip(apps_refresh_btn, "Refresh apps", tooltip::Position::Bottom)
        .style(style::tooltip_container())
        .gap(4);

    let reboot_btn = button("Reboot")
        .style(primary)
        .on_press(Message::RebootButtonPressed);

    let uad_version_text = if let Some(r) = &self_update_state.latest_release {
        match self_update_state.status {
            SelfUpdateStatus::Failed => text(format!("Failed to update to {}", r.tag_name)),
            SelfUpdateStatus::Checking => text(SelfUpdateStatus::Checking.to_string()),
            SelfUpdateStatus::Done => {
                text(format!("Update available: {} -> {}", VERSION, r.tag_name))
            }
            SelfUpdateStatus::Updating => text("Updating please wait..."),
        }
    } else {
        text(format!("v{}", VERSION))
    };

    let apps_btn = button("Apps").style(primary).on_press(Message::AppsPress);

    let about_btn = button("About")
        .style(primary)
        .on_press(Message::AboutPressed);

    let settings_btn = button(icon_text('\u{E994}'))
        .style(primary)
        .on_press(Message::SettingsPressed);

    let device_list_text = match apps_view.loading_state {
        ListLoadingState::FindingPhones => text("Finding connected devices..."),
        _ => text("No devices/emulators found"),
    };

    let device_widget: Element<'a, Message> = if let Some(phone) = selected_device {
        pick_list(device_list, Some(phone.clone()), Message::DeviceSelected).into()
    } else {
        device_list_text.into()
    };

    let mut row = row![
        reboot_btn,
        apps_refresh_tooltip,
        device_widget,
        Space::new(Length::Fill, Length::Shrink),
        uad_version_text,
    ]
    .width(Length::Fill)
    .align_y(Alignment::Center)
    .spacing(10);

    if self_update_state.latest_release.is_some() {
        let update_btn = button("Update")
            .on_press(Message::AboutAction(AboutMessage::DoSelfUpdate))
            .padding([5, 10])
            .style(primary);
        row = row.push(update_btn);
    }

    row = row.push(apps_btn).push(about_btn).push(settings_btn);

    container(row)
        .width(Length::Fill)
        .padding(10)
        .style(style::frame_container())
        .into()
}

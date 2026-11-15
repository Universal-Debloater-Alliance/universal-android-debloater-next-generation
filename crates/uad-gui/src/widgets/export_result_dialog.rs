use crate::style;
use crate::theme::Theme;
use crate::widgets::primary_button::button_primary;
use crate::widgets::text;
use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Element, Length};
use std::path::{Path, PathBuf};

/// Dialog shown after an export: the full path of the file, or why it failed.
/// `open_folder` receives the folder that holds the exported file.
pub fn export_result_dialog<'a, Message: Clone + 'a>(
    what: &str,
    result: &Result<PathBuf, String>,
    close: Message,
    open_folder: impl Fn(PathBuf) -> Message,
) -> Element<'a, Message, Theme> {
    let (title, details, title_style): (&str, String, fn(&Theme) -> _) = match result {
        Ok(path) => (
            "Export complete",
            format!("Exported {what} to:\n{}", path.display()),
            style::Text::Ok,
        ),
        Err(err) => (
            "Export failed",
            format!("Could not export {what}:\n{err}"),
            style::Text::Danger,
        ),
    };

    let mut buttons = row![Space::new().width(Length::Fill)].spacing(10);
    if let Some(folder) = result
        .as_ref()
        .ok()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
    {
        buttons = buttons.push(button_primary(text("Open folder")).on_press(open_folder(folder)));
    }
    buttons = buttons.push(button(text("Close")).padding([5, 10]).on_press(close));

    container(
        column![
            text(title).size(24).style(title_style),
            text(details),
            buttons,
        ]
        .spacing(16)
        .align_x(Alignment::Start),
    )
    .width(560)
    .padding(20)
    .style(style::Container::Frame)
    .into()
}

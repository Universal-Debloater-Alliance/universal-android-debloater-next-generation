use crate::style;
use crate::theme::Theme;
use iced::Element;
use iced::widget::button::Button;

/// Wrapper function for `iced::widget::button` with padding and style applied
pub fn button_primary<'a, Message>(
    content: impl Into<Element<'a, Message, Theme>>,
) -> Button<'a, Message, Theme> {
    iced::widget::button(content)
        .padding([5, 10])
        .style(style::Button::Primary)
}

/// Opens a file picker dialog to select a folder
pub async fn open_folder() -> Result<std::path::PathBuf, uad_core::utils::Error> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose a backup location")
        .pick_folder()
        .await
        .ok_or(uad_core::utils::Error::DialogClosed)
        .map(|f| f.path().to_path_buf())
}

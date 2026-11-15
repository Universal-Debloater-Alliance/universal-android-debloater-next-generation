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

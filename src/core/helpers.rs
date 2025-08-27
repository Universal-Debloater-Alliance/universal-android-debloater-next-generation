use crate::gui::style;
use iced::Element;
use iced::widget::button;
use iced::widget::button::Button;

/// Wrapper function for `iced::widget::button` with padding and style applied
pub fn button_primary<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> Button<'a, Message> {
    button(content)
        .padding([5, 10])
        .style(style::primary_button())
}

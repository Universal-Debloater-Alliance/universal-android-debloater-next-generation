use iced::widget::button;
use iced::widget::button::Button;
use iced::Element;
use crate::gui::styling;

/// Wrapper function for `iced::widget::button` with padding and style applied
pub fn button_primary<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> Button<'a, Message> {
    button(content)
        .padding([5, 10])
        .style(styling::primary_button())
}
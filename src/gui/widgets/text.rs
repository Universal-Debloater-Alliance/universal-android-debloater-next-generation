#![allow(
    clippy::disallowed_types,
    reason = "this is the replacement that enforces advanced shaping for disallowed [`iced::widget::Text`]"
)]

use iced::advanced::text::IntoFragment;
use iced::widget;

// Creates a new Text widget with advanced shaping.
pub fn text<'a>(text: impl IntoFragment<'a>) -> widget::Text<'a, crate::core::theme::Theme> {
    widget::Text::new(text).shaping(iced::widget::text::Shaping::Advanced)
}

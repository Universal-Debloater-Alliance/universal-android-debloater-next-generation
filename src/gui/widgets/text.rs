#![allow(
    clippy::disallowed_types,
    reason = "this is the replacement that enforces advanced shaping for disallowed [`iced::widget::Text`]"
)]

use iced::widget::Text;

// Creates a new Text widget with advanced shaping.
pub fn text<'a, Theme, Renderer>(text: impl ToString) -> Text<'a, Theme, Renderer>
where
    Theme: iced::widget::text::StyleSheet,
    Renderer: iced::advanced::text::Renderer,
{
    Text::new(text.to_string()).shaping(iced::widget::text::Shaping::Advanced)
}

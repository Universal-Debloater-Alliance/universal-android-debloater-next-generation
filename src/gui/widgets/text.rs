use iced::{advanced::text::IntoFragment, widget::Text};

pub fn text<'a>(text: impl IntoFragment<'a>) -> Text<'a> {
    Text::new(text).shaping(iced::widget::text::Shaping::Advanced)
}

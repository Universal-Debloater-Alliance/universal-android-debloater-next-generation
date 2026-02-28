use iced::theme::{self, Mode, Palette, Style};

pub use uad_core::theme::{BaseColors, BrightColors, ColorPalette, NormalColors, OS_COLOR_SCHEME};

/// GUI-local wrapper around the core Theme to satisfy orphan rules for
/// iced's Catalog traits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme(pub uad_core::theme::Theme);

#[allow(
    non_upper_case_globals,
    reason = "Keep variant-like names matching core Theme"
)]
impl Theme {
    pub const Auto: Self = Self(uad_core::theme::Theme::Auto);
    pub const Lupin: Self = Self(uad_core::theme::Theme::Lupin);
    pub const Dark: Self = Self(uad_core::theme::Theme::Dark);
    pub const Light: Self = Self(uad_core::theme::Theme::Light);

    pub const ALL: [Self; 4] = [Self::Auto, Self::Lupin, Self::Dark, Self::Light];

    #[must_use]
    pub fn palette(self) -> ColorPalette {
        self.0.palette()
    }
}

impl From<uad_core::theme::Theme> for Theme {
    fn from(value: uad_core::theme::Theme) -> Self {
        Self(value)
    }
}

impl From<Theme> for uad_core::theme::Theme {
    fn from(value: Theme) -> Self {
        value.0
    }
}

/// Converts a string to the GUI's Theme type
#[must_use]
pub fn string_to_theme(theme: &str) -> Theme {
    Theme(uad_core::theme::string_to_theme(theme))
}

impl theme::Base for Theme {
    fn default(preference: Mode) -> Self {
        Self(<uad_core::theme::Theme as theme::Base>::default(preference))
    }

    fn mode(&self) -> Mode {
        <uad_core::theme::Theme as theme::Base>::mode(&self.0)
    }

    fn base(&self) -> Style {
        <uad_core::theme::Theme as theme::Base>::base(&self.0)
    }

    fn palette(&self) -> Option<Palette> {
        <uad_core::theme::Theme as theme::Base>::palette(&self.0)
    }

    fn name(&self) -> &str {
        <uad_core::theme::Theme as theme::Base>::name(&self.0)
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

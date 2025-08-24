use dark_light;
use iced::Theme as IcedTheme;
use std::sync::LazyLock;

pub static OS_COLOR_SCHEME: LazyLock<dark_light::Mode> =
    LazyLock::new(|| dark_light::detect().unwrap_or(dark_light::Mode::Unspecified));

#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
pub enum Theme {
    #[default]
    Auto,
    Lupin,
    Dark,
    Light,
}

impl Theme {
    pub const ALL: [Self; 4] = [Self::Auto, Self::Lupin, Self::Dark, Self::Light];
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Dark => "Dark",
                Self::Light => "Light", 
                Self::Lupin => "Lupin",
                Self::Auto => "Auto (follow system theme)",
            }
        )
    }
}

// Convert to iced's built-in themes
pub fn string_to_theme(theme_str: &str) -> Theme {
    match theme_str {
        "Dark" => Theme::Dark,
        "Light" => Theme::Light,
        "Lupin" => Theme::Lupin,
        _ => Theme::Auto,
    }
}

// Convert to iced theme
pub fn to_iced_theme(theme: Theme) -> IcedTheme {
    match theme {
        Theme::Dark => IcedTheme::Dark,
        Theme::Light => IcedTheme::Light,
        // TODO: create Custom Theme
        Theme::Lupin => IcedTheme::CatppuccinMocha, // closest built-in theme
        Theme::Auto => match *OS_COLOR_SCHEME {
            dark_light::Mode::Light => IcedTheme::Light,
            _ => IcedTheme::Dark,
        }
    }
}
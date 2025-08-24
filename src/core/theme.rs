use dark_light;
use iced::{Color, color, theme};
use std::sync::LazyLock;

/// Detect system dark/light mode once at startup
pub static OS_COLOR_SCHEME: LazyLock<dark_light::Mode> =
    LazyLock::new(|| dark_light::detect().unwrap_or(dark_light::Mode::Unspecified));

/// Full color palette for a theme
#[derive(Debug, Clone, Copy)]
pub struct ColorPalette {
    pub name: &'static str,
    // Base
    pub background: Color,
    pub foreground: Color,
    // Text / Surface
    pub text: Color,
    #[allow(dead_code)]
    pub surface: Color,
    // Normal
    pub primary: Color,
    pub secondary: Color,
    pub error: Color,
    // Bright
    pub bright_primary: Color,
    #[allow(dead_code)]
    pub bright_secondary: Color,
    #[allow(dead_code)]
    pub bright_error: Color,
}

macro_rules! declare_themes {
    (
        $(
            $variant:ident [$const_name:ident] {
                label: $label:expr,
                background: $bg:expr,
                foreground: $fg:expr,
                text: $text:expr,
                surface: $surface:expr,
                primary: $primary:expr,
                secondary: $secondary:expr,
                error: $error:expr,
                bright_primary: $bpri:expr,
                bright_secondary: $bsec:expr,
                bright_error: $berr:expr
            }
        ),+ $(,)?
    ) => {
        #[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
        pub enum Theme {
            #[default]
            Auto,
            $( $variant, )+
        }

        $(
        #[allow(clippy::unreadable_literal)]
        pub const $const_name: ColorPalette = ColorPalette {
            name: $label,
            background: $bg,
            foreground: $fg,
            text: $text,
            surface: $surface,
            primary: $primary,
            secondary: $secondary,
            error: $error,
            bright_primary: $bpri,
            bright_secondary: $bsec,
            bright_error: $berr,
        };
        )+

        impl Theme {
            pub const ALL: &'static [Self] = &[
                Self::Auto,
                $( Self::$variant, )+
            ];

            pub fn palette(self) -> &'static ColorPalette {
                match self {
                    Self::Auto => match *OS_COLOR_SCHEME {
                        dark_light::Mode::Light => &LIGHT,
                        _ => &DARK,
                    },
                    $( Self::$variant => & $const_name, )+
                }
            }
        }

        impl std::fmt::Display for Theme {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Theme::Auto => write!(f, "Auto (system)"),
                    $( Theme::$variant => write!(f, stringify!($variant)), )+
                }
            }
        }

        pub fn string_to_theme(s: &str) -> Theme {
            match s {
                "Auto" | "Auto (system)" => Theme::Auto,
                $( stringify!($variant) => Theme::$variant, )+
                _ => Theme::Auto,
            }
        }

        pub fn get_color_palette(theme: &theme::Theme) -> &'static ColorPalette {
            let p = theme.palette();
            $(
                if p.background == $const_name.background
                    && p.text == $const_name.text
                    && p.primary == $const_name.primary
                    && p.success == $const_name.secondary
                    && p.danger == $const_name.error
                {
                    return &$const_name;
                }
            )+
            // Fallback
            Theme::Auto.palette()
        }

        pub fn to_iced_theme(theme: Theme) -> theme::Theme {
            let palette = theme.palette();
            theme::Theme::custom(
                palette.name.to_string(),
                theme::Palette {
                    background: palette.background,
                    text: palette.text,
                    primary: palette.primary,
                    success: palette.secondary,
                    danger: palette.error,
                },
            )
        }
    };
}

// Define themes in one place
declare_themes! {
    Dark[DARK] {
        label: "UAD Dark",
        background: color!(0x111111),
        foreground: color!(0x1C1C1C),
        text: color!(0xE0E0E0),
        surface: color!(0x828282),
        primary: color!(0x5E4266),
        secondary: color!(0x386E50),
        error: color!(0x992B2B),
        bright_primary: color!(0xBA84FC),
        bright_secondary: color!(0x49EB7A),
        bright_error: color!(0xC13047)
    },
    Light[LIGHT] {
        label: "UAD Light",
        background: color!(0xEEEEEE),
        foreground: color!(0xE0E0E0),
        text: color!(0x000000),
        surface: color!(0x818181),
        primary: color!(0x818181),
        secondary: color!(0xF9D659),
        error: color!(0x992B2B),
        bright_primary: color!(0x673AB7),
        bright_secondary: color!(0x3797A4),
        bright_error: color!(0xC13047)
    },
    Lupin[LUPIN] {
        label: "UAD Lupin",
        background: color!(0x282A36),
        foreground: color!(0x353746),
        text: color!(0xF4F8F3),
        surface: color!(0xA2A4A3),
        primary: color!(0x58406F),
        secondary: color!(0x386E50),
        error: color!(0xA13034),
        bright_primary: color!(0xBD94F9),
        bright_secondary: color!(0x49EB7A),
        bright_error: color!(0xE63E6D)
    }
}

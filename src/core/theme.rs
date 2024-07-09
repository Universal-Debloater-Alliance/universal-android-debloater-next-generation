use dark_light;
use iced::{color, Color};

#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
/// Color scheme
pub enum Theme {
    #[default]
    /// `Dark` or `Light`, according to `dark_light`
    Auto,
    /// `Dark`-ish and purple
    Lupin,
    /// white on black
    Dark,
    /// black on white
    Light,
}

#[derive(Debug, Clone, Copy)]
pub struct BaseColors {
    pub background: Color,
    pub foreground: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct NormalColors {
    pub primary: Color,
    #[allow(dead_code)]
    pub secondary: Color,
    pub surface: Color,
    pub error: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct BrightColors {
    pub primary: Color,
    pub secondary: Color,
    pub surface: Color,
    pub error: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct ColorPalette {
    pub base: BaseColors,
    pub normal: NormalColors,
    pub bright: BrightColors,
}

impl Theme {
    pub const ALL: [Self; 4] = [Self::Auto, Self::Lupin, Self::Dark, Self::Light];

    pub fn palette(self) -> ColorPalette {
        const DARK: ColorPalette = ColorPalette {
            base: BaseColors {
                background: color!(0x111111),
                foreground: color!(0x1C1C1C),
            },
            normal: NormalColors {
                primary: color!(0x5E4266),
                secondary: color!(0x386e50),
                surface: color!(0x828282),
                error: color!(0x992B2B),
            },
            bright: BrightColors {
                primary: color!(0xBA84FC),
                secondary: color!(0x49eb7a),
                surface: color!(0xE0E0E0),
                error: color!(0xC13047),
            },
        };
        const LIGHT: ColorPalette = ColorPalette {
            base: BaseColors {
                background: color!(0xEEEEEE),
                foreground: color!(0xE0E0E0),
            },
            normal: NormalColors {
                primary: color!(0x818181),
                secondary: color!(0xF9D659),
                surface: color!(0x818181),
                error: color!(0x992B2B),
            },
            bright: BrightColors {
                primary: color!(0x673AB7),
                secondary: color!(0x3797A4),
                surface: color!(0x000000),
                error: color!(0xC13047),
            },
        };
        const LUPIN: ColorPalette = ColorPalette {
            base: BaseColors {
                background: color!(0x282a36),
                foreground: color!(0x353746),
            },
            normal: NormalColors {
                primary: color!(0x58406F),
                secondary: color!(0x386e50),
                surface: color!(0xa2a4a3),
                error: color!(0xA13034),
            },
            bright: BrightColors {
                primary: color!(0xbd94f9),
                secondary: color!(0x49eb7a),
                surface: color!(0xf4f8f3),
                error: color!(0xE63E6D),
            },
        };
        match self {
            Self::Dark => DARK,
            Self::Light => LIGHT,
            Self::Lupin => LUPIN,
            Self::Auto => match dark_light::detect() {
                dark_light::Mode::Dark => DARK,
                dark_light::Mode::Light => LIGHT,
                // If the mode can't be detected, fall back to dark.
                dark_light::Mode::Default => DARK,
            },
        }
    }
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

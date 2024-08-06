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
                background: color!(0x0011_1111),
                foreground: color!(0x001C_1C1C),
            },
            normal: NormalColors {
                primary: color!(0x005E_4266),
                secondary: color!(0x0038_6e50),
                surface: color!(0x0082_8282),
                error: color!(0x0099_2B2B),
            },
            bright: BrightColors {
                primary: color!(0x00BA_84FC),
                secondary: color!(0x0049_eb7a),
                surface: color!(0x00E0_E0E0),
                error: color!(0x00C1_3047),
            },
        };
        const LIGHT: ColorPalette = ColorPalette {
            base: BaseColors {
                background: color!(0x00EE_EEEE),
                foreground: color!(0x00E0_E0E0),
            },
            normal: NormalColors {
                primary: color!(0x0081_8181),
                secondary: color!(0x00F9_D659),
                surface: color!(0x0081_8181),
                error: color!(0x0099_2B2B),
            },
            bright: BrightColors {
                primary: color!(0x0067_3AB7),
                secondary: color!(0x0037_97A4),
                surface: color!(0x0000_0000),
                error: color!(0x00C1_3047),
            },
        };
        const LUPIN: ColorPalette = ColorPalette {
            base: BaseColors {
                background: color!(0x0028_2a36),
                foreground: color!(0x0035_3746),
            },
            normal: NormalColors {
                primary: color!(0x0058_406F),
                secondary: color!(0x0038_6e50),
                surface: color!(0x00a2_a4a3),
                error: color!(0x00A1_3034),
            },
            bright: BrightColors {
                primary: color!(0x00bd_94f9),
                secondary: color!(0x0049_eb7a),
                surface: color!(0x00f4_f8f3),
                error: color!(0x00E6_3E6D),
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

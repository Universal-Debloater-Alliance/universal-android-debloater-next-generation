use iced::theme::{self, Mode, Palette, Style};
use iced::{Color, color};
use std::sync::LazyLock;

/*
In-memory caching.
This fixes the perf bug
caused by Iced repeatedly calling `palette`.

Coincidentally, this also ensures consistent colors across the GUI,
at the cost of requiring a restart to update the palette.
(this is just a patch, not a fix)
*/
pub static OS_COLOR_SCHEME: LazyLock<dark_light::Mode> =
    LazyLock::new(|| dark_light::detect().unwrap_or(dark_light::Mode::Unspecified));

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

    #[allow(
        clippy::unreadable_literal,
        reason = "https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/pull/578#discussion_r1759653408"
    )]
    /// This `fn` _could_ be `const`,
    /// but `deref`ing a lazy-`static` is non-`const`.
    #[must_use]
    pub fn palette(self) -> ColorPalette {
        const DARK: ColorPalette = ColorPalette {
            base: BaseColors {
                background: color!(0x111111),
                foreground: color!(0x1C1C1C),
            },
            normal: NormalColors {
                primary: color!(0x5E4266),
                surface: color!(0x8C8C8C),
                error: color!(0x992B2B),
            },
            bright: BrightColors {
                primary: color!(0xBA84FC),
                secondary: color!(0x49EB7A),
                surface: color!(0xE0E0E0),
                error: color!(0xE5566B),
            },
        };
        const LIGHT: ColorPalette = ColorPalette {
            base: BaseColors {
                background: color!(0xEEEEEE),
                foreground: color!(0xE0E0E0),
            },
            normal: NormalColors {
                primary: color!(0x818181),
                surface: color!(0x5F5F5F),
                error: color!(0x992B2B),
            },
            bright: BrightColors {
                primary: color!(0x673AB7),
                secondary: color!(0x1D6B75),
                surface: color!(0x000000),
                error: color!(0xA5243A),
            },
        };
        const LUPIN: ColorPalette = ColorPalette {
            base: BaseColors {
                background: color!(0x282A36),
                foreground: color!(0x353746),
            },
            normal: NormalColors {
                primary: color!(0x58406F),
                surface: color!(0xA2A4A3),
                error: color!(0xA13034),
            },
            bright: BrightColors {
                primary: color!(0xBD94F9),
                secondary: color!(0x49EB7A),
                surface: color!(0xF4F8F3),
                error: color!(0xFF7A95),
            },
        };
        match self {
            Self::Dark => DARK,
            Self::Light => LIGHT,
            Self::Lupin => LUPIN,
            Self::Auto => match *OS_COLOR_SCHEME {
                dark_light::Mode::Light => LIGHT,
                dark_light::Mode::Dark | dark_light::Mode::Unspecified => DARK,
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

/// Converts a string to the GUI's Theme type
#[must_use]
pub fn string_to_theme(theme: &str) -> Theme {
    match theme {
        "Lupin" => Theme::Lupin,
        "Dark" => Theme::Dark,
        "Light" => Theme::Light,
        _ => Theme::Auto,
    }
}

/// WCAG 2.2 relative luminance of an opaque colour.
#[cfg(test)]
fn relative_luminance(c: Color) -> f32 {
    let channel = |v: f32| {
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// WCAG 2.2 contrast ratio between two opaque colours, from 1 to 21.
#[cfg(test)]
fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `top` with its alpha painted over the opaque `bottom`.
#[cfg(test)]
fn blend(top: Color, bottom: Color) -> Color {
    let mix = |t: f32, b: f32| t * top.a + b * (1.0 - top.a);
    Color::from_rgb(
        mix(top.r, bottom.r),
        mix(top.g, bottom.g),
        mix(top.b, bottom.b),
    )
}

impl theme::Base for Theme {
    fn default(preference: Mode) -> Self {
        match preference {
            Mode::Light => Self::Light,
            Mode::Dark => Self::Dark,
            Mode::None => match *OS_COLOR_SCHEME {
                dark_light::Mode::Light => Self::Light,
                dark_light::Mode::Dark | dark_light::Mode::Unspecified => Self::Dark,
            },
        }
    }

    fn mode(&self) -> Mode {
        let resolved = match self {
            Self::Auto => match *OS_COLOR_SCHEME {
                dark_light::Mode::Light => Self::Light,
                dark_light::Mode::Dark | dark_light::Mode::Unspecified => Self::Dark,
            },
            other => *other,
        };

        match resolved {
            Self::Light => Mode::Light,
            _ => Mode::Dark,
        }
    }

    fn base(&self) -> Style {
        let p = Self::palette(*self);
        Style {
            background_color: p.base.background,
            text_color: p.bright.surface,
        }
    }

    fn palette(&self) -> Option<Palette> {
        let p = Self::palette(*self);
        Some(Palette {
            background: p.base.background,
            text: p.bright.surface,
            primary: p.normal.primary,
            success: p.bright.secondary,
            warning: p.normal.surface,
            danger: p.normal.error,
        })
    }

    fn name(&self) -> &str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Lupin => "Lupin",
            Self::Auto => "Auto",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text must reach 4.5:1 and non-text UI (borders, scrollbars) 3:1 (WCAG 2.2 AA).
    #[test]
    fn every_theme_meets_wcag_aa_contrast() {
        let mut failures = vec![];
        for theme in [Theme::Dark, Theme::Light, Theme::Lupin] {
            let p = theme.palette();
            let selected_row = blend(p.normal.primary.scale_alpha(0.25), p.base.background);
            let text_pairs = [
                (
                    "body text on background",
                    p.bright.surface,
                    p.base.background,
                ),
                ("body text on frame", p.bright.surface, p.base.foreground),
                (
                    "commentary on background",
                    p.normal.surface,
                    p.base.background,
                ),
                ("commentary on frame", p.normal.surface, p.base.foreground),
                (
                    "ok text on background",
                    p.bright.secondary,
                    p.base.background,
                ),
                ("ok text on frame", p.bright.secondary, p.base.foreground),
                (
                    "danger text on background",
                    p.bright.error,
                    p.base.background,
                ),
                ("danger text on frame", p.bright.error, p.base.foreground),
                ("primary button text", p.bright.primary, p.base.foreground),
                ("selected row text", p.bright.primary, selected_row),
            ];
            for (role, fg, bg) in text_pairs {
                let ratio = contrast_ratio(fg, bg);
                if ratio < 4.5 {
                    failures.push(format!("{theme:?}: {role} is {ratio:.2}:1, needs 4.5:1"));
                }
            }
            let ui_pairs = [
                (
                    "scrollbar on background",
                    p.normal.surface,
                    p.base.background,
                ),
                (
                    "checkbox border on frame",
                    p.normal.surface,
                    p.base.foreground,
                ),
            ];
            for (role, fg, bg) in ui_pairs {
                let ratio = contrast_ratio(fg, bg);
                if ratio < 3.0 {
                    failures.push(format!("{theme:?}: {role} is {ratio:.2}:1, needs 3:1"));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

use crate::core::theme::get_color_palette;
use iced::widget::{button, container};
use iced::{Background, Border, Color, Shadow, Theme};

// Radii
const RADIUS_SM: f32 = 2.0;
const RADIUS_MD: f32 = 5.0;

// Containers

#[inline]
fn container_style(
    bg: Color,
    text: Color,
    border_color: Color,
    border_width: f32,
    radius: f32,
) -> container::Style {
    container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(text),
        border: Border {
            color: border_color,
            width: border_width,
            radius: radius.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn frame_container() -> impl Fn(&Theme) -> container::Style {
    |theme| {
        let c = get_color_palette(theme);
        container_style(c.foreground, c.text, Color::TRANSPARENT, 0.0, RADIUS_MD)
    }
}

pub fn bordered_frame_container() -> impl Fn(&Theme) -> container::Style {
    |theme| {
        let c = get_color_palette(theme);
        let p = theme.palette();
        container_style(c.foreground, c.text, p.danger, 1.0, RADIUS_MD)
    }
}

pub fn tooltip_container() -> impl Fn(&Theme) -> container::Style {
    |theme| {
        let c = get_color_palette(theme);
        let p = theme.palette();
        container_style(c.foreground, c.text, p.primary, 1.0, 8.0)
    }
}

pub fn background_container() -> impl Fn(&Theme) -> container::Style {
    |theme| {
        let p = theme.palette();
        container_style(p.background, p.text, Color::TRANSPARENT, 0.0, RADIUS_MD)
    }
}

// Buttons: shared helpers

#[inline]
fn dim_alpha(mut c: Color, factor: f32) -> Color {
    c.a *= factor;
    c
}

#[inline]
fn filled(color: Color, text: Color, radius: f32, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Background::Color(color)),
        text_color: text,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: radius.into(),
        },
        shadow: Shadow::default(),
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(Color { a: 0.9, ..color })),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(Color { a: 0.7, ..color })),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: base.background.map(|b| match b {
                Background::Color(c) => Background::Color(dim_alpha(c, 0.5)),
                _ => b,
            }),
            ..base
        },
    }
}

#[inline]
fn outlined(stroke: Color, text: Color, radius: f32, status: button::Status) -> button::Style {
    let active = button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: text,
        border: Border {
            width: 1.0,
            color: stroke,
            radius: radius.into(),
        },
        ..button::Style::default()
    };

    match status {
        button::Status::Active => active,
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(Color { a: 0.1, ..stroke })),
            ..active
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(Color { a: 0.2, ..stroke })),
            ..active
        },
        button::Status::Disabled => button::Style {
            background: active.background,
            text_color: dim_alpha(text, 0.6),
            border: Border {
                color: dim_alpha(stroke, 0.5),
                ..active.border
            },
            ..active
        },
    }
}

// Public button styles (closures, same API as before)

pub fn primary_button() -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    |theme, status| {
        let p = theme.palette();
        let c = get_color_palette(theme);
        filled(p.primary, c.text, RADIUS_SM, status)
    }
}

pub fn danger_button() -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    |theme, status| {
        let p = theme.palette();
        let c = get_color_palette(theme);
        filled(p.danger, c.text, RADIUS_SM, status)
    }
}

pub fn restore_button() -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    |theme, status| {
        let p = theme.palette();
        let c = get_color_palette(theme);
        filled(p.success, c.text, RADIUS_SM, status)
    }
}

pub fn secondary_button() -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    |theme, status| {
        let p = theme.palette();
        outlined(p.primary, p.text, RADIUS_SM, status)
    }
}

pub fn package_button() -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    |theme, status| {
        let c = get_color_palette(theme);
        let p = theme.palette();

        let base = button::Style {
            background: Some(Background::Color(c.foreground)),
            text_color: c.text,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: RADIUS_MD.into(),
            },
            shadow: Shadow::default(),
        };

        match status {
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color {
                    a: 0.2,
                    ..p.primary
                })),
                ..base
            },
            _ => base,
        }
    }
}

pub fn selected_package_button() -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    |theme, _status| {
        let c = get_color_palette(theme);
        let p = theme.palette();
        button::Style {
            background: Some(Background::Color(Color {
                a: 0.25,
                ..p.primary
            })),
            text_color: c.bright_primary,
            border: Border {
                color: p.primary,
                width: 1.0,
                radius: RADIUS_MD.into(),
            },
            shadow: Shadow::default(),
        }
    }
}

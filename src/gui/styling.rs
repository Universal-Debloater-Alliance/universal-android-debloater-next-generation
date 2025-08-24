use iced::widget::{button, container};
use iced::{Background, Border, Color, Shadow};

pub fn frame_container() -> impl Fn(&iced::Theme) -> container::Style {
    |_| container::Style {
        background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
        text_color: Some(Color::WHITE),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 5.0.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn bordered_frame_container() -> impl Fn(&iced::Theme) -> container::Style {
    |_| container::Style {
        background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
        text_color: Some(Color::WHITE),
        border: Border {
            color: Color::from_rgb(0.8, 0.2, 0.2),
            width: 1.0,
            radius: 5.0.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn tooltip_container() -> impl Fn(&iced::Theme) -> container::Style {
    |_| container::Style {
        background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
        text_color: Some(Color::WHITE),
        border: Border {
            color: Color::from_rgb(0.5, 0.3, 0.8),
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn background_container() -> impl Fn(&iced::Theme) -> container::Style {
    |_| container::Style {
        background: Some(Background::Color(Color::from_rgb(0.05, 0.05, 0.05))),
        text_color: Some(Color::WHITE),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 5.0.into(),
        },
        shadow: Shadow::default(),
    }
}

// Button styling functions now return a closure: `impl Fn(&iced::Theme, button::Status) -> button::Style`
pub fn primary_button() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let active = button::Style {
            background: Some(Background::Color(Color::from_rgb(0.3, 0.2, 0.6))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.5, 0.3, 0.8),
                width: 1.0,
                radius: 2.0.into(),
            },
            shadow: Shadow::default(),
        };

        match status {
            button::Status::Active => active,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.4, 0.3, 0.7))),
                ..active
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.2, 0.1, 0.5))),
                ..active
            },
            button::Status::Disabled => {
                let background = active.background.map(|background| match background {
                    Background::Color(color) => Background::Color(Color { a: color.a * 0.5, ..color }),
                    _ => background,
                });
                button::Style { background, ..active }
            },
        }
    }
}

pub fn secondary_button() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = theme.palette();
        let base = button::Style {
            text_color: palette.text,
            border: Border {
                width: 1.0,
                color: palette.primary,
                radius: 2.0.into(),
            },
            ..button::Style::default()
        };

        match status {
            button::Status::Active => button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                ..base
            },
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color { a: 0.1, ..palette.primary })),
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(Color { a: 0.2, ..palette.primary })),
                ..base
            },
            button::Status::Disabled => {
                let disabled = button::secondary(theme, status); // Fallback to built-in for disabled
                disabled
            }
        }
    }
}


pub fn danger_button() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let active = button::Style {
            background: Some(Background::Color(Color::from_rgb(0.8, 0.2, 0.2))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.9, 0.3, 0.3),
                width: 1.0,
                radius: 2.0.into(),
            },
            shadow: Shadow::default(),
        };

        match status {
            button::Status::Active => active,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.9, 0.3, 0.3))),
                ..active
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.7, 0.1, 0.1))),
                ..active
            },
            button::Status::Disabled => {
                let background = active.background.map(|background| match background {
                    Background::Color(color) => Background::Color(Color { a: color.a * 0.5, ..color }),
                    _ => background,
                });
                button::Style { background, ..active }
            },
        }
    }
}

pub fn package_button() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let active = button::Style {
            background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 5.0.into(),
            },
            shadow: Shadow::default(),
        };
        
        match status {
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.15, 0.15, 0.15))),
                ..active
            },
            _ => active,
        }
    }
}

pub fn selected_package_button() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, _| button::Style {
        background: Some(Background::Color(Color::from_rgba(0.5, 0.3, 0.8, 0.25))),
        text_color: Color::from_rgb(0.7, 0.5, 1.0),
        border: Border {
            color: Color::from_rgb(0.5, 0.3, 0.8),
            width: 1.0, // Add a subtle border to show selection
            radius: 5.0.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn restore_button() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let active = button::Style {
            background: Some(Background::Color(Color::from_rgb(0.2, 0.6, 0.3))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.3, 0.7, 0.4),
                width: 1.0,
                radius: 2.0.into(),
            },
            shadow: Shadow::default(),
        };

        match status {
            button::Status::Active => active,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.3, 0.7, 0.4))),
                ..active
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.1, 0.5, 0.2))),
                ..active
            },
            button::Status::Disabled => {
                let background = active.background.map(|background| match background {
                    Background::Color(color) => Background::Color(Color { a: color.a * 0.5, ..color }),
                    _ => background,
                });
                button::Style { background, ..active }
            },
        }
    }
}

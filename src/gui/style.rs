use crate::core::theme::Theme;
use iced::widget::{
    button, checkbox, container, overlay, pick_list, radio, scrollable, text, text_editor,
    text_input,
};
use iced::{Background, Border, Color, Shadow, application};

impl application::DefaultStyle for Theme {
    fn default_style(&self) -> application::Appearance {
        let p = self.palette();
        application::Appearance {
            background_color: p.base.background,
            text_color: p.bright.surface,
        }
    }
}

// Implement theming catalogs for our custom `Theme` so generic widgets
// like `Button<'_, Message, Theme, Renderer>` compile under iced 0.13.

impl button::Catalog for Theme {
    type Class<'a> = button::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as button::Catalog>::Class<'a> {
        Box::new(|t: &Theme, s: button::Status| Button::Primary(t, s))
    }

    fn style(
        &self,
        class: &<Self as button::Catalog>::Class<'_>,
        status: button::Status,
    ) -> button::Style {
        (class)(self, status)
    }
}

impl checkbox::Catalog for Theme {
    type Class<'a> = checkbox::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as checkbox::Catalog>::Class<'a> {
        Box::new(|t: &Theme, s: checkbox::Status| CheckBox::SettingsEnabled(t, s))
    }

    fn style(
        &self,
        class: &<Self as checkbox::Catalog>::Class<'_>,
        status: checkbox::Status,
    ) -> checkbox::Style {
        (class)(self, status)
    }
}

impl container::Catalog for Theme {
    type Class<'a> = container::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as container::Catalog>::Class<'a> {
        Box::new(|t: &Theme| Container::Background(t))
    }

    fn style(&self, class: &<Self as container::Catalog>::Class<'_>) -> container::Style {
        (class)(self)
    }
}

impl scrollable::Catalog for Theme {
    type Class<'a> = scrollable::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as scrollable::Catalog>::Class<'a> {
        Box::new(|t: &Theme, s: scrollable::Status| Scrollable::Description(t, s))
    }

    fn style(
        &self,
        class: &<Self as scrollable::Catalog>::Class<'_>,
        status: scrollable::Status,
    ) -> scrollable::Style {
        (class)(self, status)
    }
}

impl text::Catalog for Theme {
    type Class<'a> = text::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as text::Catalog>::Class<'a> {
        Box::new(|t: &Theme| Text::Default(t))
    }

    fn style(&self, class: &<Self as text::Catalog>::Class<'_>) -> text::Style {
        (class)(self)
    }
}

// Additional catalogs required by Iced 0.13 generic theming

impl text_input::Catalog for Theme {
    type Class<'a> = text_input::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as text_input::Catalog>::Class<'a> {
        Box::new(|t: &Theme, s: text_input::Status| {
            let p = t.palette();

            let active = text_input::Style {
                background: Background::Color(p.base.foreground),
                border: Border {
                    color: p.base.foreground,
                    width: 0.0,
                    radius: 5.0.into(),
                },
                icon: Color {
                    a: 0.5,
                    ..p.normal.primary
                },
                placeholder: p.normal.surface,
                value: p.bright.primary,
                selection: p.normal.primary,
            };

            let focused = text_input::Style {
                background: Background::Color(p.base.foreground),
                border: Border {
                    color: Color {
                        a: 0.5,
                        ..p.normal.primary
                    },
                    width: 1.0,
                    radius: 2.0.into(),
                },
                icon: Color {
                    a: 0.5,
                    ..p.normal.primary
                },
                placeholder: p.normal.surface,
                value: p.bright.primary,
                selection: p.normal.primary,
            };

            let disabled = text_input::Style {
                background: Background::Color(p.base.background),
                border: Border {
                    color: Color {
                        a: 0.5,
                        ..p.base.foreground
                    },
                    width: 1.0,
                    radius: 2.0.into(),
                },
                icon: Color {
                    a: 0.5,
                    ..p.base.foreground
                },
                placeholder: p.normal.surface,
                value: p.bright.primary,
                selection: p.normal.primary,
            };

            match s {
                text_input::Status::Active => active,
                text_input::Status::Focused | text_input::Status::Hovered => focused,
                text_input::Status::Disabled => disabled,
            }
        })
    }

    fn style(
        &self,
        class: &<Self as text_input::Catalog>::Class<'_>,
        status: text_input::Status,
    ) -> text_input::Style {
        (class)(self, status)
    }
}

impl pick_list::Catalog for Theme {
    type Class<'a> = pick_list::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as pick_list::Catalog>::Class<'a> {
        Box::new(|t: &Theme, s: pick_list::Status| {
            let p = t.palette();
            let border_color = match s {
                pick_list::Status::Hovered => p.normal.primary,
                _ => Color {
                    a: 0.5,
                    ..p.normal.primary
                },
            };
            pick_list::Style {
                text_color: p.bright.surface,
                placeholder_color: p.bright.surface,
                handle_color: p.bright.surface,
                background: Background::Color(p.base.background.into()),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: 2.0.into(),
                },
            }
        })
    }

    fn style(
        &self,
        class: &<Self as pick_list::Catalog>::Class<'_>,
        status: pick_list::Status,
    ) -> pick_list::Style {
        (class)(self, status)
    }
}

impl overlay::menu::Catalog for Theme {
    type Class<'a> = overlay::menu::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as overlay::menu::Catalog>::Class<'a> {
        Box::new(|t: &Theme| {
            let p = t.palette();
            overlay::menu::Style {
                text_color: p.bright.surface,
                background: p.base.background.into(),
                border: Border {
                    color: p.base.background,
                    width: 1.0,
                    radius: 2.0.into(),
                },
                selected_text_color: p.bright.surface,
                selected_background: p.normal.primary.into(),
            }
        })
    }

    fn style(&self, class: &<Self as overlay::menu::Catalog>::Class<'_>) -> overlay::menu::Style {
        (class)(self)
    }
}

impl radio::Catalog for Theme {
    type Class<'a> = radio::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as radio::Catalog>::Class<'a> {
        Box::new(|t: &Theme, s: radio::Status| {
            let p = t.palette();
            let active = radio::Style {
                background: Color::TRANSPARENT.into(),
                dot_color: p.bright.primary,
                border_width: 1.0,
                border_color: p.bright.primary,
                text_color: None,
            };

            match s {
                radio::Status::Active { .. } => active,
                radio::Status::Hovered { .. } => radio::Style {
                    border_width: 2.0,
                    ..active
                },
            }
        })
    }

    fn style(
        &self,
        class: &<Self as radio::Catalog>::Class<'_>,
        status: radio::Status,
    ) -> radio::Style {
        (class)(self, status)
    }
}

impl text_editor::Catalog for Theme {
    type Class<'a> = text_editor::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as text_editor::Catalog>::Class<'a> {
        Box::new(|t: &Theme, s: text_editor::Status| {
            let p = t.palette();
            let active = text_editor::Style {
                background: Background::Color(p.base.foreground),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                icon: p.bright.surface,
                placeholder: p.normal.surface,
                value: p.bright.surface,
                selection: Color {
                    a: 0.3,
                    ..p.normal.primary
                },
            };
            match s {
                text_editor::Status::Active
                | text_editor::Status::Focused
                | text_editor::Status::Hovered => active,
                text_editor::Status::Disabled => active,
            }
        })
    }

    fn style(
        &self,
        class: &<Self as text_editor::Catalog>::Class<'_>,
        status: text_editor::Status,
    ) -> text_editor::Style {
        (class)(self, status)
    }
}

// Rule styling for custom Theme (needed by vertical_rule)
impl iced::widget::rule::Catalog for Theme {
    type Class<'a> = iced::widget::rule::StyleFn<'a, Theme>;

    fn default<'a>() -> <Self as iced::widget::rule::Catalog>::Class<'a> {
        Box::new(|t: &Theme| {
            let p = t.palette();
            iced::widget::rule::Style {
                color: p.bright.surface,
                width: 2,
                radius: 2.0.into(),
                fill_mode: iced::widget::rule::FillMode::Full,
            }
        })
    }

    fn style(
        &self,
        class: &<Self as iced::widget::rule::Catalog>::Class<'_>,
    ) -> iced::widget::rule::Style {
        (class)(self)
    }
}

#[allow(non_snake_case)]
pub mod Container {
    use super::*;

    #[allow(dead_code)]
    pub fn Invisible(_: &Theme) -> container::Style {
        container::Style::default()
    }

    pub fn Frame(theme: &Theme) -> container::Style {
        let p = theme.palette();
        container::Style {
            background: Some(Background::Color(p.base.foreground)),
            text_color: Some(p.bright.surface),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 5.0.into(),
            },
            shadow: Shadow::default(),
        }
    }

    pub fn BorderedFrame(theme: &Theme) -> container::Style {
        let p = theme.palette();
        container::Style {
            background: Some(Background::Color(p.base.foreground)),
            text_color: Some(p.bright.surface),
            border: Border {
                color: p.normal.error,
                width: 1.0,
                radius: 5.0.into(),
            },
            shadow: Shadow::default(),
        }
    }

    #[allow(dead_code)]
    pub fn Tooltip(theme: &Theme) -> container::Style {
        let p = theme.palette();
        container::Style {
            background: Some(Background::Color(p.base.foreground)),
            text_color: Some(p.bright.surface),
            border: Border {
                color: p.normal.primary,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: Shadow::default(),
        }
    }

    pub fn Background(theme: &Theme) -> container::Style {
        let p = theme.palette();
        container::Style {
            background: Some(Background::Color(p.base.background)),
            text_color: Some(p.bright.surface),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 5.0.into(),
            },
            shadow: Shadow::default(),
        }
    }
}

#[allow(non_snake_case)]
pub mod Button {
    use super::*;

    #[allow(dead_code)]
    fn base(border_color: Color) -> button::Style {
        button::Style {
            background: None,
            text_color: Color::WHITE,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 2.0.into(),
            },
            shadow: Shadow::default(),
        }
    }

    pub fn Primary(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        let mut style = style_active_hover_disabled(p.bright.primary, p.bright.primary, status);
        if matches!(status, button::Status::Active | button::Status::Pressed) {
            style.background = Some(Background::Color(p.base.foreground));
        }
        style
    }

    #[allow(dead_code)]
    pub fn SelfUpdate(theme: &Theme, status: button::Status) -> button::Style {
        Primary(theme, status)
    }

    pub fn RestorePackage(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        let mut style = style_active_hover_disabled(p.bright.secondary, p.bright.secondary, status);
        if matches!(status, button::Status::Active | button::Status::Pressed) {
            style.background = Some(Background::Color(p.base.foreground));
        }
        if matches!(status, button::Status::Disabled) {
            style.background = Some(Background::Color(Color {
                a: 0.05,
                ..p.normal.primary
            }));
            style.text_color = p.bright.primary;
        }
        style
    }

    pub fn UninstallPackage(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        let mut style = style_active_hover_disabled(p.bright.error, p.bright.error, status);
        if matches!(status, button::Status::Active | button::Status::Pressed) {
            style.background = Some(Background::Color(p.base.foreground));
        }
        style
    }

    #[allow(dead_code)]
    pub fn Unavailable(theme: &Theme, status: button::Status) -> button::Style {
        UninstallPackage(theme, status)
    }

    pub fn NormalPackage(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        match status {
            button::Status::Active | button::Status::Pressed => button::Style {
                background: Some(Background::Color(p.base.foreground)),
                text_color: p.bright.surface,
                border: Border {
                    color: p.base.background,
                    width: 0.0,
                    radius: 5.0.into(),
                },
                shadow: Shadow::default(),
            },
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color {
                    a: 0.25,
                    ..p.normal.primary
                })),
                text_color: p.bright.surface,
                border: Border {
                    color: p.base.background,
                    width: 0.0,
                    radius: 5.0.into(),
                },
                shadow: Shadow::default(),
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(p.base.foreground)),
                text_color: p.bright.surface,
                border: Border {
                    color: p.base.background,
                    width: 0.0,
                    radius: 5.0.into(),
                },
                shadow: Shadow::default(),
            },
        }
    }

    pub fn SelectedPackage(theme: &Theme, _status: button::Status) -> button::Style {
        let p = theme.palette();
        button::Style {
            background: Some(Background::Color(Color {
                a: 0.25,
                ..p.normal.primary
            })),
            text_color: p.bright.primary,
            border: Border {
                color: p.normal.primary,
                width: 0.0,
                radius: 5.0.into(),
            },
            shadow: Shadow::default(),
        }
    }

    #[allow(dead_code)]
    pub fn Hidden(_: &Theme, _: button::Status) -> button::Style {
        button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: Color::TRANSPARENT,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 5.0.into(),
            },
            shadow: Shadow::default(),
        }
    }

    fn style_active_hover_disabled(
        main: Color,
        text: Color,
        status: button::Status,
    ) -> button::Style {
        match status {
            button::Status::Active | button::Status::Pressed => button::Style {
                background: Some(Background::Color(main)),
                text_color: text,
                border: Border {
                    color: Color { a: 0.5, ..main },
                    width: 1.0,
                    radius: 2.0.into(),
                },
                shadow: Shadow::default(),
            },
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color { a: 0.25, ..main })),
                text_color: text,
                border: Border {
                    color: Color { a: 0.5, ..main },
                    width: 1.0,
                    radius: 2.0.into(),
                },
                shadow: Shadow::default(),
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(Color { a: 0.05, ..main })),
                text_color: Color { a: 0.5, ..text },
                border: Border {
                    color: Color { a: 0.5, ..main },
                    width: 1.0,
                    radius: 2.0.into(),
                },
                shadow: Shadow::default(),
            },
        }
    }
}

#[allow(non_snake_case)]
pub mod Scrollable {
    use super::*;

    #[allow(dead_code)]
    fn rails(scroller_color: Color) -> (scrollable::Rail, scrollable::Rail) {
        let rail = scrollable::Rail {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 5.0.into(),
            },
            scroller: scrollable::Scroller {
                color: scroller_color,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 1.0,
                    radius: 5.0.into(),
                },
            },
        };
        (rail, rail)
    }

    pub fn Description(theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
        let p = theme.palette();
        let (v, h) = rails(p.normal.surface);
        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: v,
            horizontal_rail: h,
            gap: Some(Background::Color(Color::TRANSPARENT)),
        }
    }

    pub fn Packages(theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
        let p = theme.palette();
        let (v, h) = rails(p.base.foreground);
        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: v,
            horizontal_rail: h,
            gap: Some(Background::Color(Color::TRANSPARENT)),
        }
    }
}

#[allow(non_snake_case)]
pub mod CheckBox {
    use super::*;

    pub fn PackageEnabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let pal = theme.palette();
        checkbox::Style {
            background: Background::Color(pal.base.background),
            icon_color: pal.bright.primary,
            border: Border {
                color: pal.base.background,
                width: 1.0,
                radius: 5.0.into(),
            },
            text_color: Some(pal.bright.surface),
        }
    }

    pub fn PackageDisabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let pal = theme.palette();
        checkbox::Style {
            background: Background::Color(Color {
                a: 0.55,
                ..pal.base.background
            }),
            icon_color: pal.bright.primary,
            border: Border {
                color: pal.normal.primary,
                width: 1.0,
                radius: 5.0.into(),
            },
            text_color: Some(pal.normal.primary),
        }
    }

    pub fn SettingsEnabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let pal = theme.palette();
        checkbox::Style {
            background: Background::Color(pal.base.background),
            icon_color: pal.bright.primary,
            border: Border {
                color: pal.bright.primary,
                width: 1.0,
                radius: 5.0.into(),
            },
            text_color: Some(pal.bright.surface),
        }
    }

    pub fn SettingsDisabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let pal = theme.palette();
        checkbox::Style {
            background: Background::Color(pal.base.foreground),
            icon_color: pal.bright.primary,
            border: Border {
                color: pal.normal.primary,
                width: 1.0,
                radius: 5.0.into(),
            },
            text_color: Some(pal.bright.surface),
        }
    }
}

#[allow(non_snake_case)]
pub mod Text {
    use super::*;

    pub fn Default(theme: &Theme) -> text::Style {
        let _ = theme;
        text::Style::default()
    }

    pub fn Ok(theme: &Theme) -> text::Style {
        let p = theme.palette();
        text::Style {
            color: Some(p.bright.secondary),
        }
    }

    pub fn Danger(theme: &Theme) -> text::Style {
        let p = theme.palette();
        text::Style {
            color: Some(p.bright.error),
        }
    }

    pub fn Commentary(theme: &Theme) -> text::Style {
        let p = theme.palette();
        text::Style {
            color: Some(p.normal.surface),
        }
    }

    #[allow(dead_code)]
    pub fn Color(c: Color) -> impl Fn(&Theme) -> text::Style {
        move |_t: &Theme| text::Style { color: Some(c) }
    }
}

// Unit tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette() {
        let palette = Theme::default().palette();

        assert_ne!(palette.base.background, palette.base.foreground);
        assert_ne!(palette.normal.primary, Color::BLACK);
        assert_ne!(palette.normal.surface, Color::BLACK);
        assert_ne!(palette.bright.primary, Color::BLACK);
        // if `LIGHT` then this can be `BLACK`
        // https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/pull/730#issuecomment-2525405134
        //assert_ne!(palette.bright.surface, Color::BLACK);
        assert_ne!(palette.normal.error, Color::BLACK);
        assert_ne!(palette.bright.error, Color::BLACK);
    }
}

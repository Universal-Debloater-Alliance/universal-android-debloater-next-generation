#![allow(
    non_snake_case,
    clippy::trivially_copy_pass_by_ref,
    clippy::wildcard_imports,
    reason = "Iced style modules use PascalCase and &Theme; wildcard for local convenience"
)]
use crate::theme::{ColorPalette, Theme};
use iced::widget::{
    button, checkbox, container, overlay, pick_list, radio, scrollable, text, text_editor,
    text_input,
};
use iced::{Background, Border, Color, Shadow};

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
                text_input::Status::Focused { .. } | text_input::Status::Hovered => focused,
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
                background: Background::Color(p.base.background),
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
                shadow: Shadow::default(),
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
        Box::new(|t: &Theme, _s: text_editor::Status| {
            let p = t.palette();
            text_editor::Style {
                background: Background::Color(p.base.foreground),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                placeholder: p.normal.surface,
                value: p.bright.surface,
                selection: Color {
                    a: 0.3,
                    ..p.normal.primary
                },
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
                radius: 2.0.into(),
                fill_mode: iced::widget::rule::FillMode::Full,
                snap: true,
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

pub mod Container {
    use super::*;

    #[allow(dead_code, reason = "Used by other themes or future styles")]
    #[must_use]
    pub fn Invisible(_: &Theme) -> container::Style {
        container::Style::default()
    }

    #[must_use]
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
            snap: true,
        }
    }

    #[must_use]
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
            snap: true,
        }
    }

    #[allow(dead_code, reason = "Currently unused in some views")]
    #[must_use]
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
            snap: true,
        }
    }

    #[must_use]
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
            snap: true,
        }
    }
}

pub mod Button {
    use super::*;

    #[allow(
        dead_code,
        reason = "Helper used by multiple styles; may be inlined by compiler"
    )]
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
            snap: true,
        }
    }

    #[must_use]
    pub fn Primary(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        let mut style = style_active_hover_disabled(p.bright.primary, p.bright.primary, status);
        if matches!(status, button::Status::Active | button::Status::Pressed) {
            style.background = Some(Background::Color(p.base.foreground));
        }
        style
    }

    #[allow(
        dead_code,
        reason = "Alias kept for semantic clarity in some call-sites"
    )]
    #[must_use]
    pub fn SelfUpdate(theme: &Theme, status: button::Status) -> button::Style {
        Primary(theme, status)
    }

    #[must_use]
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

    #[must_use]
    pub fn UninstallPackage(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        let mut style = style_active_hover_disabled(p.bright.error, p.bright.error, status);
        if matches!(status, button::Status::Active | button::Status::Pressed) {
            style.background = Some(Background::Color(p.base.foreground));
        }
        style
    }

    #[allow(
        dead_code,
        reason = "Style exposed for disabled state buttons in some contexts"
    )]
    #[must_use]
    pub fn Unavailable(theme: &Theme, status: button::Status) -> button::Style {
        UninstallPackage(theme, status)
    }

    #[must_use]
    pub fn NormalPackage(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        match status {
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
                snap: true,
            },
            _ => button::Style {
                background: Some(Background::Color(p.base.foreground)),
                text_color: p.bright.surface,
                border: Border {
                    color: p.base.background,
                    width: 0.0,
                    radius: 5.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
        }
    }

    #[must_use]
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
            snap: true,
        }
    }

    #[allow(dead_code, reason = "Used in views where buttons must be invisible")]
    #[must_use]
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
            snap: true,
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
                snap: true,
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
                snap: true,
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
                snap: true,
            },
        }
    }
}

pub mod Scrollable {
    use super::*;

    #[allow(dead_code, reason = "Kept for future custom rails variations")]
    fn rails(scroller_color: Color) -> (scrollable::Rail, scrollable::Rail) {
        let rail = scrollable::Rail {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 5.0.into(),
            },
            scroller: scrollable::Scroller {
                background: Background::Color(scroller_color),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 1.0,
                    radius: 5.0.into(),
                },
            },
        };
        (rail, rail)
    }

    fn autoscroll(p: ColorPalette) -> scrollable::AutoScroll {
        scrollable::AutoScroll {
            background: Background::Color(Color {
                a: 0.05,
                ..p.base.background
            }),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 5.0.into(),
            },
            shadow: Shadow::default(),
            icon: p.bright.surface,
        }
    }

    #[must_use]
    pub fn Description(theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
        let p = theme.palette();
        let (v, h) = rails(p.normal.surface);
        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: v,
            horizontal_rail: h,
            gap: Some(Background::Color(Color::TRANSPARENT)),
            auto_scroll: autoscroll(p),
        }
    }

    #[must_use]
    pub fn Packages(theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
        let p = theme.palette();
        let (v, h) = rails(p.base.foreground);
        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: v,
            horizontal_rail: h,
            gap: Some(Background::Color(Color::TRANSPARENT)),
            auto_scroll: autoscroll(p),
        }
    }
}

pub mod CheckBox {
    use super::*;

    #[must_use]
    pub fn PackageEnabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let p = theme.palette();
        checkbox::Style {
            background: Background::Color(p.base.background),
            icon_color: p.bright.primary,
            border: Border {
                color: p.base.background,
                width: 1.0,
                radius: 5.0.into(),
            },
            text_color: Some(p.bright.surface),
        }
    }

    #[must_use]
    pub fn PackageDisabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let p = theme.palette();
        checkbox::Style {
            background: Background::Color(Color {
                a: 0.55,
                ..p.base.background
            }),
            icon_color: p.bright.primary,
            border: Border {
                color: p.normal.primary,
                width: 1.0,
                radius: 5.0.into(),
            },
            text_color: Some(p.normal.primary),
        }
    }

    #[must_use]
    pub fn SettingsEnabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let p = theme.palette();
        checkbox::Style {
            background: Background::Color(p.base.background),
            icon_color: p.bright.primary,
            border: Border {
                color: p.bright.primary,
                width: 1.0,
                radius: 5.0.into(),
            },
            text_color: Some(p.bright.surface),
        }
    }

    #[must_use]
    pub fn SettingsDisabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let p = theme.palette();
        checkbox::Style {
            background: Background::Color(p.base.foreground),
            icon_color: p.bright.primary,
            border: Border {
                color: p.normal.primary,
                width: 1.0,
                radius: 5.0.into(),
            },
            text_color: Some(p.bright.surface),
        }
    }
}

pub mod Text {
    use super::*;

    #[must_use]
    pub fn Default(theme: &Theme) -> text::Style {
        let _ = theme;
        text::Style::default()
    }

    #[must_use]
    pub fn Ok(theme: &Theme) -> text::Style {
        let p = theme.palette();
        text::Style {
            color: Some(p.bright.secondary),
        }
    }

    #[must_use]
    pub fn Danger(theme: &Theme) -> text::Style {
        let p = theme.palette();
        text::Style {
            color: Some(p.bright.error),
        }
    }

    #[must_use]
    pub fn Commentary(theme: &Theme) -> text::Style {
        let p = theme.palette();
        text::Style {
            color: Some(p.normal.surface),
        }
    }

    #[allow(
        dead_code,
        reason = "Convenience factory used by dynamic text coloring"
    )]
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
        let palette = Theme::Dark.palette();

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

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
// like `Button<'_, Message, Theme, Renderer>` compile.

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
        Box::new(|t: &Theme, s: scrollable::Status| Scrollable::Panel(t, s))
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
                icon: p.normal.primary.scale_alpha(0.5),
                placeholder: p.normal.surface,
                value: p.bright.primary,
                selection: p.normal.primary,
            };

            let focused = text_input::Style {
                background: Background::Color(p.base.foreground),
                border: Border {
                    color: p.normal.primary.scale_alpha(0.5),
                    width: 1.0,
                    radius: 2.0.into(),
                },
                icon: p.normal.primary.scale_alpha(0.5),
                placeholder: p.normal.surface,
                value: p.bright.primary,
                selection: p.normal.primary,
            };

            let disabled = text_input::Style {
                background: Background::Color(p.base.background),
                border: Border {
                    color: p.base.foreground.scale_alpha(0.5),
                    width: 1.0,
                    radius: 2.0.into(),
                },
                icon: p.base.foreground.scale_alpha(0.5),
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
                _ => p.normal.primary.scale_alpha(0.5),
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

/// A border of `width` pixels in `color`, with rounded corners of `radius` pixels.
fn border(color: Color, width: f32, radius: f32) -> Border {
    Border {
        color,
        width,
        radius: radius.into(),
    }
}

/// A flat (shadowless) container.
fn panel(background: Color, text: Color, border: Border) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        text_color: Some(text),
        border,
        shadow: Shadow::default(),
        snap: true,
    }
}

/// A flat (shadowless) button.
fn flat_button(background: Color, text: Color, border: Border) -> button::Style {
    button::Style {
        background: Some(Background::Color(background)),
        text_color: text,
        border,
        shadow: Shadow::default(),
        snap: true,
    }
}

pub mod Container {
    use super::*;

    #[must_use]
    pub fn Invisible(_: &Theme) -> container::Style {
        container::Style::default()
    }

    #[must_use]
    pub fn Frame(theme: &Theme) -> container::Style {
        let p = theme.palette();
        panel(
            p.base.foreground,
            p.bright.surface,
            border(Color::TRANSPARENT, 0.0, 5.0),
        )
    }

    #[must_use]
    pub fn BorderedFrame(theme: &Theme) -> container::Style {
        let p = theme.palette();
        panel(
            p.base.foreground,
            p.bright.surface,
            border(p.normal.error, 1.0, 5.0),
        )
    }

    #[must_use]
    pub fn Tooltip(theme: &Theme) -> container::Style {
        let p = theme.palette();
        panel(
            p.base.foreground,
            p.bright.surface,
            border(p.normal.primary, 1.0, 8.0),
        )
    }

    #[must_use]
    pub fn Background(theme: &Theme) -> container::Style {
        let p = theme.palette();
        panel(
            p.base.background,
            p.bright.surface,
            border(Color::TRANSPARENT, 0.0, 5.0),
        )
    }
}

pub mod Button {
    use super::*;

    #[must_use]
    pub fn Primary(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        outlined(p.bright.primary, p, status)
    }

    #[must_use]
    pub fn RestorePackage(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        let mut style = outlined(p.bright.secondary, p, status);
        if matches!(status, button::Status::Disabled) {
            style.background = Some(Background::Color(p.normal.primary.scale_alpha(0.05)));
            style.text_color = p.bright.primary;
        }
        style
    }

    #[must_use]
    pub fn UninstallPackage(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        outlined(p.bright.error, p, status)
    }

    #[must_use]
    pub fn NormalPackage(theme: &Theme, status: button::Status) -> button::Style {
        let p = theme.palette();
        let background = match status {
            button::Status::Hovered => p.normal.primary.scale_alpha(0.25),
            _ => p.base.foreground,
        };
        flat_button(
            background,
            p.bright.surface,
            border(p.base.background, 0.0, 5.0),
        )
    }

    #[must_use]
    pub fn SelectedPackage(theme: &Theme, _status: button::Status) -> button::Style {
        let p = theme.palette();
        flat_button(
            p.normal.primary.scale_alpha(0.25),
            p.bright.primary,
            border(p.normal.primary, 0.0, 5.0),
        )
    }

    #[must_use]
    pub fn Hidden(_: &Theme, _: button::Status) -> button::Style {
        flat_button(
            Color::TRANSPARENT,
            Color::TRANSPARENT,
            border(Color::TRANSPARENT, 0.0, 5.0),
        )
    }

    /// A frame-coloured button with `accent` text and outline, tinted on hover.
    fn outlined(accent: Color, p: ColorPalette, status: button::Status) -> button::Style {
        let outline = border(Color { a: 0.5, ..accent }, 1.0, 2.0);
        match status {
            button::Status::Active | button::Status::Pressed => {
                flat_button(p.base.foreground, accent, outline)
            }
            button::Status::Hovered => flat_button(Color { a: 0.25, ..accent }, accent, outline),
            button::Status::Disabled => flat_button(
                Color { a: 0.05, ..accent },
                Color { a: 0.5, ..accent },
                outline,
            ),
        }
    }
}

pub mod Scrollable {
    use super::*;

    /// Package list and description panels: a visible scroller on transparent rails.
    #[must_use]
    pub fn Panel(theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
        let p = theme.palette();
        let rail = scrollable::Rail {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: border(Color::TRANSPARENT, 0.0, 5.0),
            scroller: scrollable::Scroller {
                background: Background::Color(p.normal.surface),
                border: border(Color::TRANSPARENT, 1.0, 5.0),
            },
        };
        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: rail,
            horizontal_rail: rail,
            gap: Some(Background::Color(Color::TRANSPARENT)),
            auto_scroll: scrollable::AutoScroll {
                background: Background::Color(p.base.background.scale_alpha(0.05)),
                border: border(Color::TRANSPARENT, 0.0, 5.0),
                shadow: Shadow::default(),
                icon: p.bright.surface,
            },
        }
    }
}

pub mod CheckBox {
    use super::*;

    fn checkbox_style(
        background: Color,
        border_color: Color,
        text: Color,
        p: ColorPalette,
    ) -> checkbox::Style {
        checkbox::Style {
            background: Background::Color(background),
            icon_color: p.bright.primary,
            border: border(border_color, 1.0, 5.0),
            text_color: Some(text),
        }
    }

    #[must_use]
    pub fn PackageEnabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let p = theme.palette();
        checkbox_style(p.base.background, p.normal.surface, p.bright.surface, p)
    }

    #[must_use]
    pub fn PackageDisabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let p = theme.palette();
        checkbox_style(
            p.base.background.scale_alpha(0.55),
            p.normal.primary,
            p.normal.primary,
            p,
        )
    }

    #[must_use]
    pub fn SettingsEnabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let p = theme.palette();
        checkbox_style(p.base.background, p.bright.primary, p.bright.surface, p)
    }

    #[must_use]
    pub fn SettingsDisabled(theme: &Theme, _status: checkbox::Status) -> checkbox::Style {
        let p = theme.palette();
        checkbox_style(p.base.foreground, p.normal.primary, p.bright.surface, p)
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
}

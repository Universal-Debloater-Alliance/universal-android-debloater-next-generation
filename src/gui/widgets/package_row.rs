use crate::core::sync::Phone;
use crate::core::theme::Theme;
use crate::core::uad_lists::{PackageState, Removal, UadList};
use crate::gui::style;
use crate::gui::views::settings::Settings;
use crate::gui::widgets::text;

use iced::widget::{Space, button, checkbox, row};
use iced::{Alignment, Element, Length, Renderer, Task, alignment};

#[derive(Clone, Debug)]
pub struct PackageRow {
    pub name: String,
    pub state: PackageState,
    pub description: String,
    pub uad_list: UadList,
    pub removal: Removal,
    pub selected: bool,
    pub current: bool,
}

#[derive(Clone, Debug)]
pub enum Message {
    PackagePressed,
    ActionPressed,
    ToggleSelection(bool),
}

impl PackageRow {
    pub fn new(
        name: &str,
        state: PackageState,
        description: &str,
        uad_list: UadList,
        removal: Removal,
        selected: bool,
        current: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            state,
            description: description.to_string(),
            uad_list,
            removal,
            selected,
            current,
        }
    }

    #[allow(
        clippy::unused_self,
        reason = "Consistent component API; may change later"
    )]
    pub fn update(&mut self, _message: &Message) -> Task<Message> {
        Task::none()
    }

    pub fn view(
        &self,
        settings: &Settings,
        _phone: &Phone,
    ) -> Element<'_, Message, Theme, Renderer> {
        //let trash_svg = format!("{}/resources/assets/trash.svg", env!("CARGO_MANIFEST_DIR"));
        //let restore_svg = format!("{}/resources/assets/rotate.svg", env!("CARGO_MANIFEST_DIR"));
        let button_style: fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style;
        let action_text;
        let action_btn;
        let selection_checkbox;

        match self.state {
            PackageState::Enabled => {
                action_text = if settings.device.disable_mode {
                    "Disable"
                } else {
                    "Uninstall"
                };
                button_style = style::Button::UninstallPackage;
            }
            PackageState::Disabled => {
                action_text = "Enable";
                button_style = style::Button::RestorePackage;
            }
            PackageState::Uninstalled => {
                action_text = "Restore";
                button_style = style::Button::RestorePackage;
            }
            PackageState::All => {
                action_text = "Error";
                button_style = style::Button::RestorePackage;
                warn!("Incredible! Something impossible happened!");
            }
        }
        // Disable any removal action for unsafe packages if expert_mode is disabled
        if self.removal != Removal::Unsafe
            || self.state != PackageState::Enabled
            || settings.general.expert_mode
        {
            selection_checkbox = checkbox(self.selected)
                .on_toggle(Message::ToggleSelection)
                .size(20)
                .style(style::CheckBox::PackageEnabled);

            action_btn = button(
                text(action_text)
                    .align_x(alignment::Horizontal::Center)
                    .width(100),
            )
            .on_press(Message::ActionPressed);
        } else {
            selection_checkbox = checkbox(self.selected)
                .on_toggle(Message::ToggleSelection)
                .size(20)
                .style(style::CheckBox::PackageDisabled);

            action_btn = button(
                text(action_text)
                    .align_x(alignment::Horizontal::Center)
                    .width(100),
            );
        }

        row![
            button(
                row![
                    selection_checkbox,
                    text(&self.name).width(Length::FillPortion(8)),
                    action_btn.style(button_style)
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            )
            .padding(8)
            .style(if self.current {
                style::Button::SelectedPackage
            } else {
                style::Button::NormalPackage
            })
            .width(Length::Fill)
            .on_press(Message::PackagePressed),
            Space::new().width(Length::Fixed(15.0))
        ]
        .align_y(Alignment::Center)
        .into()
    }
}

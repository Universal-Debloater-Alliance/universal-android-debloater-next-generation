use crate::core::sync::Phone;
use crate::core::uad_lists::{PackageState, Removal, UadList};
use crate::gui::style;
use crate::gui::views::settings::Settings;
use crate::gui::widgets::text;

use iced::widget::{Space, button, checkbox, row};
use iced::{Alignment, Element, Length, Task, alignment};

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

    pub fn update(&mut self, _message: &Message) -> Task<Message> {
        Task::none()
    }

    pub fn view(&self, settings: &Settings, _phone: &Phone) -> Element<Message> {
        let action_text = match self.state {
            PackageState::Enabled => {
                if settings.device.disable_mode {
                    "Disable"
                } else {
                    "Uninstall"
                }
            }
            PackageState::Disabled => "Enable",
            PackageState::Uninstalled => "Restore",
            PackageState::All => {
                warn!("Incredible! Something impossible happened!");
                "Error"
            }
        };

        let can_perform_action = self.removal != Removal::Unsafe
            || self.state != PackageState::Enabled
            || settings.general.expert_mode;

        let is_enabled = matches!(self.state, PackageState::Enabled);
        let is_current = self.current;

        // Allocation-free style selectors
        let action_style = move |theme: &iced::Theme, status: iced::widget::button::Status| {
            if is_enabled {
                style::danger_button()(theme, status)
            } else {
                style::primary_button()(theme, status)
            }
        };

        let package_style = move |theme: &iced::Theme, status: iced::widget::button::Status| {
            if is_current {
                style::selected_package_button()(theme, status)
            } else {
                style::package_button()(theme, status)
            }
        };

        let selection_checkbox = checkbox("", self.selected).on_toggle(Message::ToggleSelection);

        // Build action button once; only add on_press when allowed
        let mut action_btn = button(
            text(action_text)
                .align_x(alignment::Horizontal::Center)
                .width(100),
        )
        .style(action_style);

        if can_perform_action {
            action_btn = action_btn.on_press(Message::ActionPressed);
        } else {
            // Force primary style to communicate non-destructive default when disabled
            action_btn = action_btn.style(style::primary_button());
        }

        row![
            button(
                row![
                    selection_checkbox,
                    text(&self.name).width(Length::FillPortion(8)),
                    action_btn
                ]
                .align_y(Alignment::Center)
            )
            .padding(8)
            .style(package_style)
            .width(Length::Fill)
            .on_press(Message::PackagePressed),
            Space::with_width(15)
        ]
        .align_y(Alignment::Center)
        .into()
    }
}

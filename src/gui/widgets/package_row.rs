use crate::core::sync::Phone;
use crate::core::uad_lists::{PackageState, Removal, UadList};
use crate::gui::styling;
use crate::gui::views::settings::Settings;
use crate::gui::widgets::text;

use iced::widget::{Space, button, checkbox, row};
use iced::{Alignment, Task, Element, Length, alignment};

fn get_action_button_style(enabled: bool) -> Box<dyn Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style> {
    if enabled {
        Box::new(styling::danger_button())
    } else {
        Box::new(styling::primary_button())
    }
}

fn get_package_button_style(current: bool) -> Box<dyn Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style> {
    if current {
        Box::new(styling::selected_package_button())
    } else {
        Box::new(styling::package_button())
    }
}

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

        let selection_checkbox = checkbox("", self.selected)
            .on_toggle(Message::ToggleSelection);

        let action_btn = if can_perform_action {
            button(
                text(action_text)
                    .align_x(alignment::Horizontal::Center)
                    .width(100),
            )
            .style(get_action_button_style(self.state == PackageState::Enabled))
            .on_press(Message::ActionPressed)
        } else {
            button(
                text(action_text)
                    .align_x(alignment::Horizontal::Center)
                    .width(100),
            )
            .style(styling::primary_button())
        };

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
            .style(get_package_button_style(self.current))
            .width(Length::Fill)
            .on_press(Message::PackagePressed),
            Space::with_width(15)
        ]
        .align_y(Alignment::Center)
        .into()
    }
}
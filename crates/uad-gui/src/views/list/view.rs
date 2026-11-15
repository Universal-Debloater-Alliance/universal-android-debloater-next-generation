use super::dialogs::error_view;
use crate::style;
use crate::theme::Theme;
use crate::widgets::export_result_dialog::export_result_dialog;
use crate::widgets::primary_button::button_primary;
use std::path::PathBuf;
use uad_core::NAME;
use uad_core::sync::Phone;
use uad_core::uad_lists::{PackageState, Removal, UadList};

use super::{List, LoadingState, Message};
use crate::views::settings::Settings;
use crate::widgets::modal::Modal;
use crate::widgets::text;
use iced::widget::{
    Space, button, checkbox, column, container, pick_list, row, scrollable, text_editor,
    text_input, tooltip,
};
use iced::{Alignment, Element, Length, Renderer, alignment};

impl List {
    /// Builds the main view for the app list interface
    pub fn view(
        &self,
        settings: &Settings,
        selected_device: &Phone,
    ) -> Element<'_, Message, Theme, Renderer> {
        match &self.loading_state {
            LoadingState::DownloadingList => waiting_view(
                &format!("Downloading latest {NAME} lists from GitHub. Please wait..."),
                Some(button("No internet?").on_press(Message::LoadUadList(false))),
                style::Text::Default,
            ),
            LoadingState::FindingPhones => {
                if self.is_adb_satisfied {
                    waiting_view("Finding connected devices...", None, style::Text::Default)
                } else {
                    waiting_view(
                        "ADB is not installed on your system, install ADB and relaunch application.",
                        Some(button("Read on how to get started.")
                    .on_press(Message::GoToUrl(PathBuf::from(
                        "https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Getting-started",
                    )))),
                        style::Text::Danger,
                    )
                }
            }
            LoadingState::LoadingPackages => waiting_view(
                "Pulling packages from the device. Please wait...",
                None,
                style::Text::Default,
            ),
            LoadingState::UpdatingUad => waiting_view(
                &format!("Updating {NAME}. Please wait..."),
                None,
                style::Text::Default,
            ),
            LoadingState::RestoringDevice(device) => waiting_view(
                &format!("Restoring device: {device}"),
                None,
                style::Text::Default,
            ),
            LoadingState::Ready => self.ready_view(settings, selected_device),
            LoadingState::FailedToUpdate => waiting_view(
                "Failed to download update",
                Some(button("Go back").on_press(Message::LoadUadList(false))),
                style::Text::Danger,
            ),
        }
    }

    pub(super) fn control_panel(
        &self,
        selected_device: &Phone,
    ) -> Element<'_, Message, Theme, Renderer> {
        let search_packages = text_input("Search packages...", &self.input_value)
            .width(Length::Fill)
            .on_input(Message::SearchInputChanged)
            .padding([5, 10]);

        let select_all_checkbox = checkbox(self.all_selected)
            .on_toggle(Message::ToggleAllSelected)
            .size(20)
            .style(style::CheckBox::SettingsEnabled)
            .spacing(0); // no label, so remove space entirely

        let col_sel_all = row![
            tooltip(
                select_all_checkbox,
                if self.all_selected {
                    "Deselect all"
                } else {
                    "Select all"
                },
                tooltip::Position::Top,
            )
            .style(style::Container::Tooltip)
            .gap(4)
        ]
        .padding(8);

        let user_picklist = pick_list(
            selected_device.user_list.clone(),
            self.selected_user,
            Message::UserSelected,
        )
        .width(85);

        let list_picklist =
            pick_list(UadList::ALL, self.selected_list, Message::ListSelected).width(96);

        let package_state_picklist = pick_list(
            PackageState::ALL,
            self.selected_package_state,
            Message::PackageStateSelected,
        )
        .width(120);

        let removal_picklist = pick_list(
            Removal::ALL,
            self.selected_removal,
            Message::RemovalSelected,
        )
        .width(150);

        row![
            col_sel_all,
            search_packages,
            user_picklist,
            removal_picklist,
            package_state_picklist,
            list_picklist,
        ]
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .spacing(6)
        .padding(iced::Padding {
            top: 0.0,
            right: 16.0,
            bottom: 0.0,
            left: 0.0,
        })
        .into()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "One screen layout, read top to bottom"
    )]
    pub(super) fn ready_view(
        &self,
        settings: &Settings,
        selected_device: &Phone,
    ) -> Element<'_, Message, Theme, Renderer> {
        let packages = self
            .filtered_packages
            .iter()
            .fold(column![].spacing(6), |col, &i| {
                col.push(
                    self.phone_packages[self.selected_user.unwrap_or_default().index][i]
                        .view(settings, selected_device)
                        .map(move |msg| Message::List(i, msg)),
                )
            });

        let packages_scrollable: Element<'_, Message, Theme, Renderer> =
            if self.filtered_packages.is_empty() {
                let hint = if selected_device.adb_id.is_empty() {
                    "No device connected. Turn on USB debugging on the phone, connect it, \
                     accept the prompt on the phone, then press the refresh button."
                } else {
                    "No package matches these filters. Clear the search or choose other filters."
                };
                // `max_width` keeps the hint under about 70 characters per line.
                container(container(text(hint).style(style::Text::Commentary)).max_width(560))
                    .center(Length::Fill)
                    .height(Length::FillPortion(6))
                    .padding(20)
                    .into()
            } else {
                scrollable(packages)
                    .height(Length::FillPortion(6))
                    .style(style::Scrollable::Panel)
                    .into()
            };

        let description_scroll =
            scrollable(text_editor(&self.description_content).on_action(Message::DescriptionEdit))
                .style(style::Scrollable::Panel);

        let description_panel =
            container(column![self.details_header(), description_scroll].spacing(6))
                .padding(6)
                .height(Length::FillPortion(2))
                .width(Length::Fill)
                .style(style::Container::Frame);

        let review_selection = {
            let tmp_widget = text(format!(
                "Review selection ({})",
                self.selected_packages.len()
            ));
            if self.selected_packages.is_empty() {
                button(tmp_widget).padding([5, 10])
            } else {
                button_primary(tmp_widget).on_press(Message::ApplyActionOnSelection)
            }
        };

        let mut export_selection = button(text(format!(
            "Export current selection ({})",
            self.selected_packages.len()
        )))
        .padding([5, 10]);
        if !self.selected_packages.is_empty() {
            export_selection = export_selection
                .on_press(Message::ExportSelection)
                .style(style::Button::Primary);
        }
        // lock
        let export_selection = export_selection;

        let action_row = row![
            export_selection,
            Space::new().width(Length::Fill).height(Length::Shrink),
            review_selection
        ]
        .width(Length::Fill)
        .spacing(10)
        .align_y(Alignment::Center);

        let unavailable = container(
                    column![
                        text("ADB is not authorized to access this user!").size(20)
                            .style(style::Text::Danger),
                        text("The most likely reason is that it is the user of your work profile (also called Secure Folder on Samsung devices). There's really no solution, other than completely disabling your work profile in your device settings.")
                            .style(style::Text::Commentary)
                            .align_x(alignment::Horizontal::Center),
                    ]
                    .spacing(6)
                    .align_x(Alignment::Center)
                )
                .padding(10)
                .center_x(Length::Shrink)
                .style(style::Container::BorderedFrame);

        let control_panel = self.control_panel(selected_device);

        // Fallback notifications area
        let notifications_area: Element<'_, Message, Theme, Renderer> =
            if self.fallback_notifications.is_empty() {
                Space::new().into()
            } else {
                let notification_texts: Vec<_> = self
                    .fallback_notifications
                    .iter()
                    .map(|msg| text(msg).style(style::Text::Commentary).into())
                    .collect();

                container(
                    column![
                        text("Fallback Actions Performed:").style(style::Text::Default),
                        column(notification_texts).spacing(4),
                        row![
                            Space::new().width(Length::Fill).height(Length::Shrink),
                            button(text("Dismiss"))
                                .on_press(Message::DismissFallbackNotifications)
                                .style(style::Button::Primary)
                                .padding([4, 10]),
                        ]
                    ]
                    .spacing(6),
                )
                .padding(8)
                .style(style::Container::BorderedFrame)
                .into()
            };

        let content = if selected_device.user_list.is_empty()
            || match self.selected_user {
                Some(u) => !self.phone_packages[u.index].is_empty(),
                // If no user has been selected,
                // then it could be considered as "equivalent"
                // to the case where the `user_list` is empty?
                // However, this is inconsistent,
                // because other parts of the code simply use a `default` `User`.
                None => true,
            } {
            column![
                control_panel,
                notifications_area,
                packages_scrollable,
                description_panel,
                action_row,
            ]
        } else {
            column![
                control_panel,
                notifications_area,
                container(unavailable)
                    .height(Length::Fill)
                    .center_y(Length::Fill),
            ]
        }
        .width(Length::Fill)
        .spacing(10)
        .align_x(Alignment::Center);

        if self.selection_modal {
            return Modal::new(
                content.padding(10),
                self.apply_selection_modal(
                    selected_device,
                    settings,
                    &self.phone_packages[self.selected_user.unwrap_or_default().index],
                ),
            )
            .on_blur(Message::ModalHide)
            .into();
        }

        if let Some(result) = &self.export_result {
            let dialog = export_result_dialog(
                "the selected package names",
                result,
                Message::ModalHide,
                Message::GoToUrl,
            );
            return Modal::new(content.padding(10), dialog)
                .on_blur(Message::ModalHide)
                .into();
        }

        if let Some(selection) = self.pending_action {
            return Modal::new(
                content.padding(10),
                self.confirm_action_modal(settings, selection),
            )
            .on_blur(Message::ModalHide)
            .into();
        }

        if let Some(err) = &self.error_modal {
            error_view(err, content, self.copy_confirmation).into()
        } else {
            container(content).height(Length::Fill).padding(10).into()
        }
    }

    /// Name, list and rating of the package shown in the details panel.
    pub(super) fn details_header(&self) -> Element<'_, Message, Theme, Renderer> {
        let i_user = self.selected_user.unwrap_or_default().index;
        match self
            .phone_packages
            .get(i_user)
            .and_then(|packages| packages.get(self.current_package_index))
            .filter(|package| package.current)
        {
            Some(package) => row![
                text(&package.name).size(16),
                text(format!("list: {}", package.list)).style(style::Text::Commentary),
                text(package.removal.to_string()).style(rating_style(package.removal)),
            ]
            .spacing(14)
            .align_y(Alignment::Center)
            .into(),
            None => text("Click a package to see what it does and what depends on it.")
                .style(style::Text::Commentary)
                .into(),
        }
    }
}

/// Text colour for a removal rating: green for low risk, red for high risk.
pub(super) fn rating_style(removal: Removal) -> fn(&Theme) -> iced::widget::text::Style {
    match removal {
        Removal::Recommended => style::Text::Ok,
        Removal::Expert | Removal::Unsafe => style::Text::Danger,
        Removal::Advanced | Removal::Unlisted | Removal::All => style::Text::Default,
    }
}

pub(super) fn waiting_view<'a>(
    displayed_text: &(impl ToString + ?Sized),
    btn: Option<button::Button<'a, Message, Theme, Renderer>>,
    text_style: impl Fn(&Theme) -> iced::widget::text::Style + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    let col = column![]
        .spacing(10)
        .align_x(Alignment::Center)
        .push(text(displayed_text.to_string()).style(text_style).size(20));

    let col = match btn {
        Some(btn) => col.push(btn.style(style::Button::Primary).padding([5, 10])),
        None => col,
    };

    container(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .center_x(Length::Fill)
        .style(style::Container::Frame)
        .into()
}

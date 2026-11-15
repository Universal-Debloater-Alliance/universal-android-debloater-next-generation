use super::view::rating_style;
use crate::style;
use crate::theme::Theme;
use crate::widgets::navigation_menu::ICONS;
use uad_core::package_text::truncate_description;
use uad_core::sync::Phone;
use uad_core::uad_lists::{PackageState, Removal};

use super::{List, Message, SummaryEntry};
use crate::views::settings::Settings;
use crate::widgets::modal::Modal;
use crate::widgets::package_row::{PackageRow, action_label};
use crate::widgets::text;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{
    Column, Space, button, checkbox, column, container, radio, row, rule, scrollable, tooltip,
};
use iced::{Alignment, Element, Length, Renderer, alignment};

impl List {
    #[allow(
        clippy::too_many_lines,
        reason = "Modal construction is verbose by nature"
    )]
    pub(super) fn apply_selection_modal(
        &self,
        device: &Phone,
        settings: &Settings,
        packages: &[PackageRow],
    ) -> Element<'_, Message, Theme, Renderer> {
        const PACK_NO_USER_MSG: &str = "`selected_packages` implies a user must be selected";

        // 5 element slice is cheap
        let mut summaries = Removal::CATEGORIES.map(SummaryEntry::from);
        for p in packages.iter().filter(|p| p.selected) {
            let summary = &mut summaries[p.removal as usize];
            match p.state {
                PackageState::Uninstalled | PackageState::Disabled => summary.restore += 1,
                _ => summary.discard += 1,
            }
        }

        let radio_btn_users = device.user_list.iter().filter(|&u| !u.protected).fold(
            row![].spacing(10),
            |row, &user| {
                row.push(
                    radio(
                        format!("{}", user.clone()),
                        user,
                        self.selected_user,
                        Message::ModalUserSelected,
                    )
                    .size(24),
                )
            },
        );

        let title_ctn =
            container(row![text("Review your selection").size(24)].align_y(Alignment::Center))
                .width(Length::Fill)
                .style(style::Container::Frame)
                .padding([10, 0])
                .center_y(Length::Shrink)
                .center_x(Length::Fill);

        let users_ctn = container(radio_btn_users)
            .padding(10)
            .center_x(Length::Shrink)
            .style(style::Container::Frame);

        let explaination_ctn = container(
            row![
                text("The action for the selected user will be applied to all other users")
                    .style(style::Text::Danger),
                tooltip(
                    text("\u{EA0C}")
                        .font(ICONS)
                        .width(22)
                        .align_x(alignment::Horizontal::Center)
                        .style(style::Text::Commentary),
                    "Let's say you choose user 0. If a selected package on user 0\n\
                        is set to be uninstalled and if this same package is disabled on user 10,\n\
                        then the package on both users will be uninstalled.",
                    tooltip::Position::Top,
                )
                .gap(20)
                .padding(10)
                .style(style::Container::Tooltip)
            ]
            .spacing(10),
        )
        .center_x(Length::Shrink)
        .padding(10)
        .style(style::Container::BorderedFrame);

        let modal_btn_row = row![
            button(text("Cancel")).on_press(Message::ModalHide),
            Space::new().width(Length::Fill),
            button(text("Apply")).on_press(Message::ModalValidate),
        ]
        .padding(iced::Padding {
            top: 0.0,
            right: 15.0,
            bottom: 10.0,
            left: 10.0,
        });

        let recap_view = summaries
            .iter()
            .fold(column![].spacing(6).width(Length::Fill), |col, r| {
                col.push(recap(settings, r))
            });

        let selected_pkgs_ctn = container(
            container(
                scrollable(
                    container(
                        if self
                            .selected_packages
                            .iter()
                            .any(|s| s.0 == self.selected_user.expect(PACK_NO_USER_MSG).index)
                        {
                            self.selected_packages
                                .iter()
                                .filter(|s| {
                                    s.0 == self.selected_user.expect(PACK_NO_USER_MSG).index
                                })
                                .fold(
                                    column![].spacing(6).width(Length::Shrink),
                                    |col, selection| {
                                        col.push(
                                            row![
                                                row![text(
                                                    self.phone_packages[selection.0][selection.1]
                                                        .removal
                                                        .to_string()
                                                )]
                                                .width(120),
                                                row![text(
                                                    self.phone_packages[selection.0][selection.1]
                                                        .list
                                                        .to_string()
                                                )]
                                                .width(55),
                                                row![text(
                                                    self.phone_packages[selection.0][selection.1]
                                                        .name
                                                        .clone()
                                                ),]
                                                .width(540),
                                                row![match self.phone_packages[selection.0]
                                                    [selection.1]
                                                    .state
                                                {
                                                    PackageState::Enabled =>
                                                        if settings.device.disable_mode {
                                                            text("Disable")
                                                                .style(style::Text::Danger)
                                                        } else {
                                                            text("Uninstall")
                                                                .style(style::Text::Danger)
                                                        },
                                                    PackageState::Disabled =>
                                                        text("Enable").style(style::Text::Ok),
                                                    PackageState::Uninstalled =>
                                                        text("Restore").style(style::Text::Ok),
                                                    PackageState::All => text("Impossible")
                                                        .style(style::Text::Danger),
                                                },]
                                                .width(70),
                                            ]
                                            .width(Length::Shrink)
                                            .spacing(20),
                                        )
                                    },
                                )
                        } else {
                            column![text("No packages selected for this user")]
                                .align_x(Alignment::Center)
                                .width(Length::Fill)
                        },
                    )
                    .padding(10)
                    .width(Length::Shrink)
                    .style(style::Container::Invisible),
                )
                .direction(Direction::Both {
                    vertical: Scrollbar::default(),
                    horizontal: Scrollbar::default(),
                })
                .style(style::Scrollable::Panel),
            )
            .width(Length::Fill)
            .style(style::Container::Frame),
        )
        .width(Length::Fill)
        .max_height(150)
        .padding([0, 10]);

        let i_user = self.selected_user.unwrap_or_default().index;
        let selection: Vec<usize> = self
            .selected_packages
            .iter()
            .filter(|s| s.0 == i_user)
            .map(|s| s.1)
            .collect();
        let relations_ctn =
            container(self.relations_panel(settings, i_user, &selection)).padding([0, 10]);

        container(
            if device.user_list.iter().filter(|&u| !u.protected).count() > 1
                && settings.device.multi_user_mode
            {
                column![
                    title_ctn,
                    users_ctn,
                    row![explaination_ctn].padding([0, 10]),
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    relations_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            } else if !settings.device.multi_user_mode {
                column![
                    title_ctn,
                    users_ctn,
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    relations_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            } else {
                column![
                    title_ctn,
                    container(recap_view).padding(10),
                    selected_pkgs_ctn,
                    relations_ctn,
                    modal_btn_row,
                ]
                .spacing(10)
                .align_x(Alignment::Center)
            },
        )
        .width(900)
        .height(Length::Shrink)
        .max_height(760)
        .style(style::Container::Background)
        .into()
    }

    /// Related packages, warnings about dependents and missing requirements
    /// for `selection` (package indexes of one user). Empty when nothing applies.
    pub(super) fn relations_panel(
        &self,
        settings: &Settings,
        i_user: usize,
        selection: &[usize],
    ) -> Element<'_, Message, Theme, Renderer> {
        let (plan, missing) = self.removal_plan(i_user, selection);
        let rating = |name: &str| {
            self.phone_packages[i_user]
                .iter()
                .find(|p| p.name == name)
                .map_or(Removal::Unlisted, |p| p.removal)
        };
        let verb = if settings.device.disable_mode {
            "disable"
        } else {
            "remove"
        };

        let mut lines: Vec<Element<'_, Message, Theme, Renderer>> = vec![];
        if !plan.related.is_empty() {
            lines.push(
                checkbox(self.include_related)
                    .label(format!(
                        "Also {verb} {} related package(s) listed below",
                        plan.related.len()
                    ))
                    .on_toggle(Message::IncludeRelatedToggled)
                    .size(18)
                    .style(style::CheckBox::SettingsEnabled)
                    .into(),
            );
            for related in &plan.related {
                lines.push(
                    text(format!(
                        "    {} [{}]: {}",
                        related.name,
                        rating(&related.name),
                        related.reason
                    ))
                    .style(style::Text::Commentary)
                    .into(),
                );
            }
        }
        for skipped in &plan.excluded_unsafe {
            lines.push(
                text(format!(
                    "{} is rated Unsafe ({}). It is never added automatically.",
                    skipped.name, skipped.reason
                ))
                .style(style::Text::Danger)
                .into(),
            );
        }
        for risk in &plan.at_risk {
            lines.push(
                text(format!(
                    "Warning: {} may stop working, because it needs {}.",
                    risk.dependent, risk.requirement
                ))
                .style(style::Text::Danger)
                .into(),
            );
        }
        for (package, requirement) in &missing {
            lines.push(
                text(format!(
                    "Warning: {package} needs {requirement}, which is not enabled. \
                     Restore it too, or {package} may not work."
                ))
                .style(style::Text::Danger)
                .into(),
            );
        }

        if lines.is_empty() {
            return Space::new().into();
        }
        container(scrollable(column(lines).spacing(4).padding(4)))
            .width(Length::Fill)
            .max_height(170)
            .padding(8)
            .style(style::Container::BorderedFrame)
            .into()
    }

    /// Asks before acting on one package, showing its rating and relations.
    pub(super) fn confirm_action_modal(
        &self,
        settings: &Settings,
        (i_user, i_package): (usize, usize),
    ) -> Element<'_, Message, Theme, Renderer> {
        let package = &self.phone_packages[i_user][i_package];
        let action = action_label(package.state, settings.device.disable_mode);
        let removing = package.state == PackageState::Enabled;

        let mut body = column![
            text(format!("{action} {}?", package.name)).size(22),
            row![
                text("Removal rating:"),
                text(package.removal.to_string()).style(rating_style(package.removal)),
            ]
            .spacing(6),
            text(package.removal.explanation()).style(if removing {
                rating_style(package.removal)
            } else {
                style::Text::Commentary
            }),
            text(truncate_description(&package.description, 240)),
            self.relations_panel(settings, i_user, &[i_package]),
        ]
        .spacing(12);

        if removing && settings.device.disable_mode {
            body = body.push(
                text("Disabling keeps the package installed. You can enable it again at any time.")
                    .style(style::Text::Commentary),
            );
        } else if removing {
            body = body.push(
                text(
                    "Uninstalling removes the package for this user only. \
                     You can restore it later from the list (filter by \"Uninstalled\").",
                )
                .style(style::Text::Commentary),
            );
        }

        let confirm = button(text(action))
            .padding([5, 10])
            .on_press(Message::ConfirmPendingAction)
            .style(if removing {
                style::Button::UninstallPackage
            } else {
                style::Button::RestorePackage
            });
        body = body.push(row![
            button(text("Cancel"))
                .padding([5, 10])
                .on_press(Message::ModalHide),
            Space::new().width(Length::Fill),
            confirm,
        ]);

        container(body)
            .width(680)
            .padding(20)
            .style(style::Container::Background)
            .into()
    }
}

pub(super) fn error_view<'a>(
    error: &'a str,
    content: Column<'a, Message, Theme, Renderer>,
    copy_confirmation: bool,
) -> Modal<'a, Message, Theme, Renderer> {
    let title_ctn = container(
        row![text("Failed to perform ADB operation").size(24)].align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .style(style::Container::Frame)
    .padding([10, 0])
    .center_y(Length::Shrink)
    .center_x(Length::Shrink);

    let modal_btn_row = row![
        button(
            text(if copy_confirmation {
                "Copied!"
            } else {
                "Copy error"
            })
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .on_press_maybe(if copy_confirmation {
            None
        } else {
            Some(Message::CopyError(error.to_string()))
        })
        .style(style::Button::Primary),
        button(
            text("Close")
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .on_press(Message::ModalHide)
    ]
    .padding(iced::Padding {
        top: 10.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    });

    let text_box = scrollable(text(error).width(Length::Fill)).height(400);

    let ctn = container(column![title_ctn, text_box, modal_btn_row])
        .height(Length::Shrink)
        .max_height(700)
        .padding(10)
        .style(style::Container::Frame);

    Modal::new(content, ctn).on_blur(Message::ModalHide)
}

pub(super) fn recap<'a>(
    settings: &Settings,
    recap: &SummaryEntry,
) -> Element<'a, Message, Theme, Renderer> {
    container(
        row![
            text(recap.category.to_string())
                .size(19)
                .width(Length::FillPortion(1)),
            rule::vertical(5),
            row![
                if settings.device.disable_mode {
                    text("Disable").style(style::Text::Danger)
                } else {
                    text("Uninstall").style(style::Text::Danger)
                },
                Space::new().width(Length::Fill),
                text(recap.discard.to_string()).style(style::Text::Danger)
            ]
            .width(Length::FillPortion(1)),
            rule::vertical(5),
            row![
                if settings.device.disable_mode {
                    text("Enable").style(style::Text::Ok)
                } else {
                    text("Restore").style(style::Text::Ok)
                },
                Space::new().width(Length::Fill),
                text(recap.restore.to_string()).style(style::Text::Ok)
            ]
            .width(Length::FillPortion(1))
        ]
        .spacing(20)
        .padding(iced::Padding {
            top: 0.0,
            right: 10.0,
            bottom: 0.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .align_y(Alignment::Center),
    )
    .padding(10)
    .width(Length::Fill)
    .height(45)
    .style(style::Container::Frame)
    .into()
}

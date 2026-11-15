# Design

Purpose: how the UAD-ng GUI looks and behaves. Read when: changing any screen, widget, colour or text in `crates/uad-gui`.
Last verified: 2026-11-15.

## Tokens

Colours live only in `crates/uad-gui/src/theme.rs`. Widget styles in `style.rs` read them from `Theme::palette()`. Never write a colour literal anywhere else.

| Role (`palette` field) | Used for                                    | Dark      | Light     | Lupin     |
| ---------------------- | ------------------------------------------- | --------- | --------- | --------- |
| `base.background`      | Window background                           | `#111111` | `#EEEEEE` | `#282A36` |
| `base.foreground`      | Frames, rows, button fill                   | `#1C1C1C` | `#E0E0E0` | `#353746` |
| `bright.surface`       | Body text                                   | `#E0E0E0` | `#000000` | `#F4F8F3` |
| `normal.surface`       | Secondary text, scrollbars, checkbox border | `#8C8C8C` | `#5F5F5F` | `#A2A4A3` |
| `bright.primary`       | Accent: primary buttons, selection, focus   | `#BA84FC` | `#673AB7` | `#BD94F9` |
| `normal.primary`       | Selected-row tint, disabled outlines        | `#5E4266` | `#818181` | `#58406F` |
| `bright.secondary`     | Success, "Restore", `Recommended` rating    | `#49EB7A` | `#1D6B75` | `#49EB7A` |
| `bright.error`         | Danger, "Uninstall", `Expert` and `Unsafe`  | `#E5566B` | `#A5243A` | `#FF7A95` |
| `normal.error`         | Warning frame border                        | `#992B2B` | `#992B2B` | `#A13034` |

- Contrast: every text role reaches 4.5:1 and scrollbars and checkbox borders 3:1 on both `base` colours, in every theme. Enforced by `every_theme_meets_wcag_aa_contrast` in `theme.rs`.
- `Auto` resolves to Dark or Light from the OS setting once at start-up (`OS_COLOR_SCHEME`). Changing the OS theme needs a restart.
- Type: default 16 px (`gui.rs`). Section headings 26 px, dialog titles 24 px, sub-headings 20 px. Linux uses the "Adwaita Sans" family to avoid clipped glyphs.
- Spacing: 10 px between controls and inside frames, 6 px inside dense lists, 20 px between sections.
- Window: opens at 950 x 700, minimum 900 x 560 (`min_size` in `gui.rs`), so the filter row and navigation bar never clip.

## Components

| Component                 | File                                                   | States                                                       |
| ------------------------- | ------------------------------------------------------ | ------------------------------------------------------------ |
| Primary (outlined) button | `widgets/primary_button.rs`                            | active, hovered (tint), pressed, disabled (50% text)         |
| Restore / Uninstall       | `style::Button::RestorePackage` and `UninstallPackage` | same as primary, green or red accent                         |
| Package row               | `widgets/package_row.rs`                               | normal, hovered, selected, checkbox on or off                |
| Navigation bar            | `widgets/navigation_menu.rs`                           | device picker, "finding devices", "no devices", update ready |
| Modal                     | `widgets/modal.rs`                                     | closes on outside click (`on_blur`)                          |
| Export result dialog      | `widgets/export_result_dialog.rs`                      | success (path and "Open folder"), failure (reason)           |
| Shaped text               | `widgets/text.rs`                                      | the only allowed text widget (advanced shaping)              |
| List dialogs              | `views/list/dialogs.rs`                                | review selection, confirm one action, relations, errors      |

## Patterns

- Icons: the `icomoon` font from `resources/assets/icons.ttf` (`ICONS` in `navigation_menu.rs`). No emoji as icons.
- Keyboard: Ctrl+Shift+A (apps), +I (about), +S (settings), +5 (refresh), +R (reboot). Shown in tooltips.
- Ratings are named in text and coloured: green for `Recommended`, red for `Expert` and `Unsafe`, default text for the rest. Colour never carries the meaning alone.
- Empty states say what to do next: no device ("Turn on USB debugging ..., then press the refresh button"), or no matching package ("Clear the search or choose other filters").
- Copy: name actions by what they do ("Review selection", "Export current selection"), call removal levels "rated" (`Recommended`, `Unsafe`), and keep long hints under about 70 characters per line.
- Motion: none. Iced redraws state changes instantly.

## Decisions

- Kept the existing visual identity (purple accent, three themes): this was an accessibility and layout pass on an established app, not a redesign, so no new references were gathered.
- Palette lightness adjusted, hue kept: 12 role pairs failed WCAG AA (for example Light secondary text at 2.95:1, Dark danger text at 3.07:1).
- Unchecked package checkboxes have a visible border: it matched the background before and was invisible on rows.
- Package-list scrollbar uses `normal.surface`: it used the frame colour and could not be seen.
- Minimum window size instead of a reflowing layout: below about 900 px the search field collapsed and navigation buttons left the window. A desktop tool with seven filters and actions in one row reads best at that width.
- Screenshots at 900 px and 1440 px in Dark and Light were reviewed after the change (list, settings and about screens, no device attached). The package list with a real device was not captured: no device was connected.

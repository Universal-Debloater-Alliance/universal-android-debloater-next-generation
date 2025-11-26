use crate::core::sync::Phone;
use crate::core::theme::Theme;
use crate::core::uad_lists::{PackageState, Removal, UadList};
use crate::gui::style;
use crate::gui::views::settings::Settings;
use crate::gui::widgets::text;
use iced::widget::image::Handle;
use iced::widget::{Image, Space, button, checkbox, row};
use iced::{Alignment, Command, Element, Length, Renderer, alignment};
use regex::Regex;
use std::io::Read;

//use crate::core::adb::extract_package;
use std::path::PathBuf;
//use crate::core::adb::handle_package_icon;

#[derive(Clone, Debug)]
pub struct PackageRow {
    pub name: String,
    pub state: PackageState,
    pub description: String,
    pub uad_list: UadList,
    pub removal: Removal,
    pub selected: bool,
    pub current: bool,
    pub icon_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub enum Message {
    PackagePressed,
    ActionPressed,
    ToggleSelection(bool),
    LoadIcon(String),
    IconLoaded(String, PathBuf),
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
        let icons_dir = PathBuf::from("resources/extracted_icons");
        let cached_icon = icons_dir.join(format!("{}.png", name));

        let icon_path = if cached_icon.exists() {
            Some(cached_icon)
        } else {
            println!("❌ No cached icon found for {}", name);
            None // will be loaded asynchronously
        };

        Self {
            name: name.to_string(),
            state,
            description: description.to_string(),
            uad_list,
            removal,
            selected,
            current,
            icon_path,
        }
    }

    pub fn handle_package_icon(
        package_name: &str,
        apks_dir: &PathBuf,
        icons_dir: &PathBuf,
    ) -> Result<PathBuf, String> {
        use crate::core::adb::pull_apk;
        use regex::Regex;
        use std::fs::File;
        use std::io::Read;
        use zip::ZipArchive;

        let local_apk_path = apks_dir.join(format!("{}.apk", package_name));
        let icon_path = icons_dir.join(format!("{}.png", package_name));

        // Return existing icon if present
        if icon_path.exists() {
            return Ok(icon_path);
        }

        println!("🔍 Icon not found for {}", package_name);

        // Pull APK if missing
        if !local_apk_path.exists() {
            println!("📦 Pulling APK for {}", package_name);
            pull_apk(package_name, apks_dir)?;
        }

        // Open APK
        let file =
            File::open(&local_apk_path).map_err(|e| format!("Failed to open APK: {:?}", e))?;
        let mut archive = ZipArchive::new(file).map_err(|e| format!("Invalid APK zip: {:?}", e))?;

        // Step 1: Collect all candidate images in drawable/mipmap folders
        let mut candidates: Vec<(String, u64)> = vec![];

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let name = file.name().to_string();

            // Only images in drawable/mipmap folders
            if !(name.starts_with("res/drawable")
                || name.starts_with("res/mipmap")
                || name.starts_with("res/mipmap-"))
            {
                continue;
            }

            // Accept png, webp, jpg, jpeg
            if !(name.ends_with(".png")
                || name.ends_with(".webp")
                || name.ends_with(".jpg")
                || name.ends_with(".jpeg"))
            {
                continue;
            }

            candidates.push((name.clone(), file.size()));
        }

        // Step 2: Prefer launcher-named icons
        let mut launcher_candidates: Vec<(String, u64)> = candidates
            .iter()
            .filter(|(name, _)| {
                let lname = name.to_lowercase();
                lname.contains("ic_launcher")
                    || lname.contains("launcher")
                    || lname.contains("foreground")
                //|| lname.contains("background")
            })
            .cloned()
            .collect();

        // Pick largest launcher candidate
        launcher_candidates.sort_by(|a, b| b.1.cmp(&a.1));
        if let Some((name, _)) = launcher_candidates.first() {
            let mut file = archive
                .by_name(name)
                .map_err(|e| format!("Failed to read icon: {:?}", e))?;
            let mut out_file = File::create(&icon_path)
                .map_err(|e| format!("Failed to create icon file: {:?}", e))?;
            std::io::copy(&mut file, &mut out_file)
                .map_err(|e| format!("Failed to write icon: {:?}", e))?;
            println!("✅ Extracted launcher icon for {}", package_name);
            return Ok(icon_path);
        }

        // Step 3: Adaptive XML icons
        let mut adaptive_xml: Option<String> = None;
        for i in 0..archive.len() {
            let file = archive.by_index(i).unwrap();
            let name = file.name().to_string();
            let lname = name.to_lowercase();

            if (name.starts_with("res/drawable") || name.starts_with("res/mipmap"))
                && name.ends_with(".xml")
                && (lname.contains("ic_launcher") || lname.contains("launcher"))
            {
                adaptive_xml = Some(name.clone());
                break;
            }
        }

        if let Some(xml_name) = adaptive_xml {
            println!("Found adaptive icon XML: {}", xml_name);

            let xml_contents = {
                let mut xml_file = archive
                    .by_name(&xml_name)
                    .map_err(|e| format!("Failed to read XML: {:?}", e))?;
                let mut s = String::new();
                xml_file
                    .read_to_string(&mut s)
                    .map_err(|e| format!("Failed to read XML content: {:?}", e))?;
                s // return from block
            };

            // Match drawable, src, foreground
            let mut xml_candidates: Vec<String> = vec![];

            let re =
                Regex::new(r#"android:(?:drawable|src|foreground|background)="@(\w+)/([\w\d_]+)""#)
                    .unwrap();
            let densities = ["xxxhdpi", "xxhdpi", "xhdpi", "hdpi", "mdpi", "ldpi", ""];
            let extensions = ["png", "webp", "jpg", "jpeg"];

            for cap in re.captures_iter(&xml_contents) {
                let folder = &cap[1];
                let base = &cap[2];

                for d in &densities {
                    for ext in &extensions {
                        let candidate = if d.is_empty() {
                            format!("res/{}/{}.{}", folder, base, ext)
                        } else {
                            format!("res/{}-{}/{}.{}", folder, d, base, ext)
                        };
                        // Just push name for now
                        xml_candidates.push(candidate);
                    }
                }
            }
            let mut xml_candidates_with_size: Vec<(String, u64)> = vec![];
            for candidate in xml_candidates {
                if let Ok(file) = archive.by_name(&candidate) {
                    xml_candidates_with_size.push((candidate.clone(), file.size()));
                }
            }
            xml_candidates_with_size.sort_by(|a, b| b.1.cmp(&a.1));
            if let Some((name, _)) = xml_candidates_with_size.first() {
                let mut file = archive
                    .by_name(name)
                    .map_err(|e| format!("Failed to read XML candidate: {:?}", e))?;
                let mut out_file = File::create(&icon_path)
                    .map_err(|e| format!("Failed to create icon file: {:?}", e))?;
                std::io::copy(&mut file, &mut out_file)
                    .map_err(|e| format!("Failed to write icon: {:?}", e))?;
                return Ok(icon_path);
            }
        }

        // Step 4: Fallback to largest image overall
        candidates.sort_by(|a, b| b.1.cmp(&a.1));
        if let Some((name, _)) = candidates.first() {
            let mut file = archive
                .by_name(name)
                .map_err(|e| format!("Failed to read fallback icon: {:?}", e))?;
            let mut out_file = File::create(&icon_path)
                .map_err(|e| format!("Failed to create icon file: {:?}", e))?;
            std::io::copy(&mut file, &mut out_file)
                .map_err(|e| format!("Failed to write icon: {:?}", e))?;
            return Ok(icon_path);
        }

        // Step 5: No icon found
        Ok(PathBuf::from("resources/Images/dummy.png"))
    }

    pub fn update(&mut self, message: &Message) -> Command<Message> {
        match message {
            Message::IconLoaded(pkg_name, path) if *pkg_name == self.name => {
                self.icon_path = Some(path.clone());
                Command::none()
            }

            // Trigger async extraction
            Message::LoadIcon(pkg_name) if *pkg_name == self.name => {
                let package_name = pkg_name.clone(); // base name
                let package_name_for_closure = pkg_name.clone(); // clone for closure

                let apks_dir = PathBuf::from("resources/extracted_apks");
                let icons_dir = PathBuf::from("resources/extracted_icons");

                Command::perform(
                    async move {
                        println!("🔍 Handling icon for {}", package_name);

                        match PackageRow::handle_package_icon(&package_name, &apks_dir, &icons_dir)
                        {
                            Ok(path) => path,
                            Err(_) => PathBuf::from("resources/Images/dummy.png"),
                        }
                    },
                    move |path| Message::IconLoaded(package_name_for_closure, path),
                )
            }

            _ => Command::none(),
        }
    }

    pub fn view(&self, settings: &Settings, _phone: &Phone) -> Element<Message, Theme, Renderer> {
        //let trash_svg = format!("{}/resources/assets/trash.svg", env!("CARGO_MANIFEST_DIR"));
        //let restore_svg = format!("{}/resources/assets/rotate.svg", env!("CARGO_MANIFEST_DIR"));
        let button_style;
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
            selection_checkbox = checkbox("", self.selected)
                .on_toggle(Message::ToggleSelection)
                .style(style::CheckBox::PackageEnabled);

            action_btn = button(
                text(action_text)
                    .horizontal_alignment(alignment::Horizontal::Center)
                    .width(100),
            )
            .on_press(Message::ActionPressed);
        } else {
            selection_checkbox = checkbox("", self.selected)
                .on_toggle(Message::ToggleSelection)
                .style(style::CheckBox::PackageDisabled);

            action_btn = button(
                text(action_text)
                    .horizontal_alignment(alignment::Horizontal::Center)
                    .width(100),
            );
        }

        let icon_path = self
            .icon_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("resources/Images/dummy.png"));

        let icon: Image<Handle> = Image::new(Handle::from_path(icon_path))
            .width(34)
            .height(34);

        row![
            button(
                row![
                    selection_checkbox,
                    icon,
                    text(&self.name).width(Length::FillPortion(8)),
                    action_btn.style(button_style)
                ]
                .align_items(Alignment::Center)
            )
            .padding(8)
            .style(if self.current {
                style::Button::SelectedPackage
            } else {
                style::Button::NormalPackage
            })
            .width(Length::Fill)
            .on_press(Message::PackagePressed),
            Space::with_width(15)
        ]
        .align_items(Alignment::Center)
        .into()
    }
}

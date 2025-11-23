use crate::core::sync::Phone;
use crate::core::theme::Theme;
use crate::core::uad_lists::{PackageState, Removal, UadList};
use regex::Regex;
use std::borrow::Cow;
use std::io::Read;

use crate::gui::style;
use crate::gui::views::settings::Settings;
use crate::gui::widgets::text;
use iced::widget::image::Handle;
use iced::widget::{Image, Space, button, checkbox, row};
use iced::{Alignment, Command, Element, Length, Renderer, alignment};

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
            println!("🖼️ Using cached icon for {}", name);
            Some(cached_icon)
        } else {
            println!("❌ No cached icon found for {}", name);
            None // will be loaded asynchronously later
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
        use std::fs::File;
        use std::io::Read;
        use zip::ZipArchive;

        let local_apk_path = apks_dir.join(format!("{}.apk", package_name));
        let icon_path = icons_dir.join(format!("{}.png", package_name));

        // If icon already exists, return it
        if icon_path.exists() {
            println!("🖼️ Using cached icon for {}", package_name);
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

        // ----------------------------
        // 1️⃣ First, try PNG icons
        // ----------------------------
        let mut launcher_png: Option<String> = None;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let name = file.name().to_string();
            let lname = name.to_lowercase();

            // Only consider launcher-related names
            let likely_launcher = lname.contains("ic_launcher") || lname.contains("launcher");

            if !likely_launcher || !name.ends_with(".png") {
                continue;
            }

            // Check density folders for PNG
            if name.starts_with("res/drawable-") || name.starts_with("res/drawable/") {
                launcher_png = Some(name.clone());
                break; // PNG found, stop here
            }
        }

        if let Some(png_name) = launcher_png {
            let mut png_file = archive
                .by_name(&png_name)
                .map_err(|e| format!("Failed to read PNG: {:?}", e))?;
            let mut out_file = File::create(&icon_path)
                .map_err(|e| format!("Failed to create icon file: {:?}", e))?;
            std::io::copy(&mut png_file, &mut out_file)
                .map_err(|e| format!("Failed to write icon: {:?}", e))?;
            println!("✅ Extracted PNG icon for {}", package_name);
            return Ok(icon_path);
        }

        // ----------------------------
        // 2️⃣ Fallback: XML adaptive icon
        // ----------------------------
        let mut launcher_xml: Option<String> = None;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let name = file.name().to_string();
            let lname = name.to_lowercase();

            if (name.starts_with("res/drawable") || name.starts_with("res/mipmap"))
                && name.ends_with(".xml")
                && (lname.contains("ic_launcher") || lname.contains("launcher"))
            {
                launcher_xml = Some(name.clone());
                break;
            }
        }

        if let Some(xml_name) = launcher_xml {
            println!("Found adaptive icon XML: {}", xml_name);

            let xml_contents = {
                let mut xml_contents = String::new();
                let mut xml_file = archive
                    .by_name(&xml_name)
                    .map_err(|e| format!("XML missing: {:?}", e))?;
                xml_file
                    .read_to_string(&mut xml_contents)
                    .map_err(|e| format!("Failed to read XML file: {:?}", e))?;
                xml_contents // return it from the block
            };

            let re = regex::Regex::new(r#"android:drawable="@(\w+)/([\w\d_]+)""#).unwrap();

            for cap in re.captures_iter(&xml_contents) {
                let folder = &cap[1];
                let base = &cap[2];

                let search_paths = [
                    format!("res/{}-xxxhdpi/{}.png", folder, base),
                    format!("res/{}-xxhdpi/{}.png", folder, base),
                    format!("res/{}-xhdpi/{}.png", folder, base),
                    format!("res/{}-hdpi/{}.png", folder, base),
                    format!("res/{}-mdpi/{}.png", folder, base),
                    format!("res/{}-ldpi/{}.png", folder, base),
                    format!("res/{}/{}.png", folder, base),
                ];

                for candidate in search_paths {
                    if let Ok(mut png_file) = archive.by_name(&candidate) {
                        let mut out_file = File::create(&icon_path)
                            .map_err(|e| format!("Failed to create icon file: {:?}", e))?;
                        std::io::copy(&mut png_file, &mut out_file)
                            .map_err(|e| format!("Failed to write icon: {:?}", e))?;
                        println!("📦 Extracted adaptive icon layer from {}", candidate);
                        return Ok(icon_path);
                    }
                }
            }
        }

        // ----------------------------
        // 3️⃣ Fallback default icon
        // ----------------------------
        println!("⚠️ No launcher icon found, using default");
        Ok(PathBuf::from("resources/Images/dummy.png"))
    }

    pub fn update(&mut self, message: &Message) -> Command<Message> {
        match message {
            // Icon extraction finished
            Message::IconLoaded(pkg_name, path) if *pkg_name == self.name => {
                println!("✅ IconLoaded received for {}", pkg_name);
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

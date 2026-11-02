const META: &str = "../../resources/assets/manifest.rc";
const ICON: &str = "../../resources/assets/icon.ico";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed={META}");
    println!("cargo::rerun-if-changed={ICON}");
    if std::env::var("CARGO_CFG_TARGET_FAMILY")? == "windows" {
        embed_resource::compile(
            META,
            embed_resource::ParamsIncludeDirs(["../../resources/assets"]),
        )
        .manifest_optional()?;
    }
    Ok(())
}

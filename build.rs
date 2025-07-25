const META: &str = "resources/assets/manifest.rc";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed={META}");
    if std::env::var("CARGO_CFG_TARGET_FAMILY")? == "windows" {
        embed_resource::compile(META, embed_resource::NONE);
    }
    Ok(())
}

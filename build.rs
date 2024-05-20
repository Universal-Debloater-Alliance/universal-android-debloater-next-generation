use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    if env::var("CARGO_CFG_TARGET_FAMILY")? == "windows" {
        embed_resource::compile("resources/assets/manifest.rs", embed_resource::NONE);
    }

    Ok(())
}

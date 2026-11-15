#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::collapsible_if,
    clippy::uninlined_format_args,
    clippy::result_unit_err,
    reason = "Doc+style pedantic lints are out-of-scope for this pass"
)]

pub mod adb;
pub mod config;
pub mod export;
mod http_client;
pub mod opener;
pub mod package_text;
pub mod relations;
pub mod relative_time;
pub mod save;
pub mod sync;
pub mod uad_lists;
pub mod update;

use log::error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Canonical shortened name of the application
pub const NAME: &str = "UAD-ng";
/// Full name of the application
pub const FULL_NAME: &str = "Universal Android Debloater Next Generation";

/// `<OS config dir>/uad`, created on first use.
pub static CONFIG_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| create_app_dir(&dirs::config_dir().expect("Can't detect config dir")));
/// `<OS cache dir>/uad`, created on first use. Holds logs, the cached list and backups.
pub static CACHE_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| create_app_dir(&dirs::cache_dir().expect("Can't detect cache dir")));

fn create_app_dir(parent: &Path) -> PathBuf {
    let dir = parent.join("uad");
    if let Err(e) = fs::create_dir_all(&dir) {
        error!("Can't create directory: {}", dir.display());
        panic!("{e}");
    }
    dir
}

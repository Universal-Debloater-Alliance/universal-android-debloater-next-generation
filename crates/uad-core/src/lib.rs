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
pub mod save;
pub mod sync;
pub mod theme;
pub mod uad_lists;
pub mod update;
pub mod utils;

use std::path::PathBuf;
use std::sync::LazyLock;
pub static CONFIG_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| utils::setup_uad_dir(&dirs::config_dir().expect("Can't detect config dir")));
pub static CACHE_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| utils::setup_uad_dir(&dirs::cache_dir().expect("Can't detect cache dir")));

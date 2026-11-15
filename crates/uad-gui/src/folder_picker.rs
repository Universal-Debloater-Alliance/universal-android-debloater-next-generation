use std::path::PathBuf;

/// Asks the user for a backup folder. `None` when the dialog is closed without a choice.
pub async fn pick_backup_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose a backup location")
        .pick_folder()
        .await
        .map(|folder| folder.path().to_path_buf())
}

//! The wording of settings notices shown as banners in the main window.

use crate::settings::Notice;

/// What to tell the user about `notice`.
pub(super) fn text(notice: &Notice) -> String {
    match notice {
        Notice::Corrupt { file, moved_to, .. } => match moved_to {
            Some(moved) => format!(
                "{} couldn't be read and was reset. The old file was kept as {}.",
                file.display(),
                moved.display()
            ),
            None => format!("{} couldn't be read; settings won't be saved.", file.display()),
        },
        Notice::RestoredFromBackup { file } => {
            format!("{} was empty and was restored from its backup.", file.display())
        }
        Notice::NotPersisted { reason } => format!("Changes won't be saved. {reason}"),
        Notice::SaveFailed { error } => format!("Settings couldn't be saved: {error}"),
    }
}

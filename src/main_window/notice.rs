//! The wording of settings notices shown as banners in the main window.

use crate::settings::Notice;

/// What to tell the user about `notice`.
pub(super) fn text(notice: &Notice) -> String {
    match notice {
        Notice::Corrupt { file, moved_to, .. } => match moved_to {
            Some(moved) => tr!(
                "{file} couldn't be read and was reset. The old file was kept as {moved}.",
                file = file.display(),
                moved = moved.display()
            ),
            None => tr!("{file} couldn't be read; settings won't be saved.", file = file.display()),
        },
        Notice::RestoredFromBackup { file } => {
            tr!("{file} was empty and was restored from its backup.", file = file.display())
        }
        Notice::NotPersisted { reason } => tr!("Changes won't be saved. {reason}", reason),
        Notice::SaveFailed { error } => tr!("Settings couldn't be saved: {error}", error),
    }
}

//! What a media folder's row says about it (S4.2.6), in the library's empty state and in
//! Settings → Library (S5.9.3): how many items it has, its status, and the scan running in it.

use gpui::SharedString;

use super::indexer::RootState;
use crate::db::roots::RootStatus;

/// What the row says about `root`.
pub fn status_text(root: &RootState) -> SharedString {
    if let Some(progress) = root.indexing {
        return progress.label().into();
    }
    let items = crate::navigation::item_count(root.files);
    let text = match root.status {
        RootStatus::Ok => items,
        RootStatus::Slow => tr!("{items} · Slow network folder", items),
        RootStatus::Missing if root.files > 0 => {
            tr!("{items} · Drive not connected or folder moved", items)
        }
        RootStatus::Missing => tr!("This folder doesn't exist").to_owned(),
        RootStatus::PermissionDenied => tr!("VLC isn't allowed to read this folder").to_owned(),
    };
    match root.unreadable {
        0 => text.into(),
        n => trn!(
            "{text} · {count} folder couldn't be read",
            "{text} · {count} folders couldn't be read",
            n,
            text
        )
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::roots::RootKind;
    use crate::library::progress::Progress;

    fn root(status: RootStatus, files: usize) -> RootState {
        RootState {
            path: "/v".into(),
            kind: RootKind::Video,
            status,
            files,
            unreadable: 0,
            indexing: None,
        }
    }

    #[test]
    fn rows_say_what_each_folder_is_doing() {
        assert_eq!(status_text(&root(RootStatus::Ok, 1)), "1 item");
        assert_eq!(status_text(&root(RootStatus::Ok, 12_345)), "12,345 items");
        assert_eq!(status_text(&root(RootStatus::Missing, 0)), "This folder doesn't exist");
        let unplugged = root(RootStatus::Missing, 3);
        assert_eq!(status_text(&unplugged), "3 items · Drive not connected or folder moved");
        let indexing = RootState {
            indexing: Some(Progress { found: 1_234, estimate: 5_000 }),
            ..root(RootStatus::Ok, 0)
        };
        assert_eq!(status_text(&indexing), "Indexing 1,234 of ~5,000");
        let partly = RootState { unreadable: 2, ..root(RootStatus::Slow, 2) };
        assert_eq!(
            status_text(&partly),
            "2 items · Slow network folder · 2 folders couldn't be read"
        );
    }
}

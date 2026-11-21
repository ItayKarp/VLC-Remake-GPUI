//! The main window's side of diagnostics (S0.7): telling the user about the last
//! crash and copying debug info.

use gpui::{ClipboardItem, Context, Window};

use super::MainWindow;
use crate::diagnostics::{self, crash, debug_info, system};
use crate::settings;
use crate::ui::toast::{Level, Toast};

impl MainWindow {
    /// Offers to show the crash report the last run left, if there is one.
    pub(super) fn show_pending_crash(&mut self, cx: &mut Context<Self>) {
        let Some(report) = diagnostics::logs_dir().and_then(|logs| crash::take_pending(&logs))
        else {
            return;
        };
        let toast = Toast::new(
            Level::Warning,
            "VLC closed unexpectedly last time. A crash report was saved on this computer; \
             nothing was sent.",
        )
        .action("Show Report", move |_, cx| cx.reveal_path(&report));
        self.show_toast(toast, cx);
    }

    /// "Copy Debug Info" (S0.7.4): puts the debug info on the clipboard.
    pub(super) fn copy_debug_info(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        system::remember_gpu(window);
        let text = debug_info::text(settings::get(cx), system::gpu());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.show_toast(Toast::new(Level::Success, "Debug info copied to the clipboard."), cx);
    }
}

//! Accepting certificates libVLC can't verify, while "Verify certificates against the system
//! trust store" is off (S5.10.1).
//!
//! libVLC 3 has no option to skip verification: its TLS modules ask the user, through libVLC's
//! dialog callbacks, whether to go on to an "Insecure site", and fail the connection when nobody
//! answers. With verification on VLC installs no callbacks, so those connections fail. With it
//! off, [`install`] answers each of those questions with its first action ("View certificate",
//! then "Accept 24 hours"), and dismisses every other question so none waits for an answer.
//! Login dialogs stay unanswered, as before.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use super::LIBRARY;
use super::api::{Instance, LibVlc};

/// The title libVLC's TLS modules give the question about a certificate they can't verify.
const INSECURE_SITE: &str = "Insecure site";
/// `libvlc_dialog_post_action`'s first action.
const FIRST_ACTION: c_int = 1;

/// `libvlc_dialog_id`.
#[repr(C)]
pub struct DialogId {
    _private: [u8; 0],
}

/// `libvlc_dialog_cbs` from libVLC 3, field for field; a missing callback is NULL.
#[repr(C)]
pub struct DialogCallbacks {
    display_login: Option<unsafe extern "C" fn()>,
    display_question: Option<QuestionCallback>,
    display_progress: Option<unsafe extern "C" fn()>,
    cancel: Option<unsafe extern "C" fn(*mut c_void, *mut DialogId)>,
    update_progress: Option<unsafe extern "C" fn()>,
}

/// `pf_display_question`: data, id, title, text, type, cancel, action 1, action 2.
type QuestionCallback = unsafe extern "C" fn(
    *mut c_void,
    *mut DialogId,
    *const c_char,
    *const c_char,
    c_int,
    *const c_char,
    *const c_char,
    *const c_char,
);

/// The callbacks: questions answered or dismissed, nothing else shown.
static CALLBACKS: DialogCallbacks = DialogCallbacks {
    display_login: None,
    display_question: Some(question),
    display_progress: None,
    cancel: Some(cancelled),
    update_progress: None,
};

/// Has `instance` accept certificates it can't verify.
pub(super) fn install(vlc: &LibVlc, instance: *mut Instance) {
    tracing::warn!("libvlc: certificates aren't verified (Settings → Network)");
    // SAFETY: the instance is live, and the callbacks are static, so they outlive it.
    unsafe {
        (vlc.api.libvlc_dialog_set_callbacks)(instance, &raw const CALLBACKS, ptr::null_mut());
    }
}

/// Answers a certificate question with its first action, and dismisses any other question.
unsafe extern "C" fn question(
    _data: *mut c_void,
    id: *mut DialogId,
    title: *const c_char,
    _text: *const c_char,
    _kind: c_int,
    _cancel: *const c_char,
    _action1: *const c_char,
    _action2: *const c_char,
) {
    let Some(Ok(vlc)) = LIBRARY.get() else { return };
    // SAFETY: libVLC passes a NUL-terminated title, or NULL.
    let title = (!title.is_null()).then(|| unsafe { CStr::from_ptr(title) }.to_string_lossy());
    // SAFETY: `id` is the live dialog this call is about; answering before libVLC starts waiting
    // is allowed, and the id isn't used afterwards.
    unsafe {
        if title.as_deref() == Some(INSECURE_SITE) {
            tracing::warn!("libvlc: accepting a certificate it couldn't verify");
            (vlc.api.libvlc_dialog_post_action)(id, FIRST_ACTION);
        } else {
            (vlc.api.libvlc_dialog_dismiss)(id);
        }
    }
}

/// A dialog libVLC withdraws must be dismissed to be released. Questions are answered as they
/// show, so this is only a safeguard.
unsafe extern "C" fn cancelled(_data: *mut c_void, id: *mut DialogId) {
    let Some(Ok(vlc)) = LIBRARY.get() else { return };
    // SAFETY: `id` is the live dialog libVLC withdraws.
    unsafe { (vlc.api.libvlc_dialog_dismiss)(id) };
}

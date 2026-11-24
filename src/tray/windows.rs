//! The notification-area icon on Windows: `Shell_NotifyIconW` on a hidden window of our own, whose
//! window procedure shows the menu on a right click and Show VLC on a left click. Explorer
//! forgets every icon when it restarts and then broadcasts "TaskbarCreated" to top-level windows,
//! so the window is top-level (never shown) rather than message-only, and adds the icon again.

use std::cell::RefCell;
use std::ptr::{null, null_mut};

use anyhow::bail;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION, NIN_SELECT, NINF_KEY,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, GetSystemMetrics, IMAGE_ICON, LR_DEFAULTCOLOR, LR_SHARED, LoadImageW,
    MF_SEPARATOR, MF_STRING, PostMessageW, RegisterClassW, RegisterWindowMessageW, SM_CXSMICON,
    SM_CYSMICON, SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    WM_APP, WM_CONTEXTMENU, WM_NULL, WNDCLASSW, WS_OVERLAPPED,
};

use super::{CommandSender, TrayCommand};

/// The message the icon sends its window when it is clicked.
const CALLBACK_MESSAGE: u32 = WM_APP + 1;
/// The icon's id on its window; it has only one.
const ICON_ID: u32 = 1;
/// The app icon's resource id in the executable (`build.rs`).
const ICON_RESOURCE: usize = 1;
/// A click by keyboard (Space or Enter on the focused icon).
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
/// The tooltip.
const TOOLTIP: &str = "VLC";

thread_local! {
    /// Where the icon's window procedure sends commands, while the icon shows.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

struct State {
    sender: CommandSender,
    /// Explorer's "TaskbarCreated" message.
    taskbar_created: u32,
}

/// The icon and its hidden window; both go when dropped.
pub struct Icon {
    window: HWND,
}

impl Icon {
    pub fn show(sender: CommandSender) -> anyhow::Result<Self> {
        let class = wide("RedesignedVlcTray");
        // SAFETY: plain Win32 calls on this (the main) thread, with null-terminated names that
        // outlive them; the class's procedure is `window_procedure`.
        let window = unsafe {
            let instance = GetModuleHandleW(null());
            let mut class_info: WNDCLASSW = std::mem::zeroed();
            class_info.lpfnWndProc = Some(window_procedure);
            class_info.hInstance = instance;
            class_info.lpszClassName = class.as_ptr();
            // Fails harmlessly when an earlier icon already registered it.
            RegisterClassW(&raw const class_info);
            let none = null_mut();
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                none,
                none,
                instance,
                null(),
            )
        };
        if window.is_null() {
            bail!("couldn't make the tray icon's window");
        }
        // SAFETY: a null-terminated name.
        let taskbar_created = unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
        STATE.set(Some(State { sender, taskbar_created }));
        let icon = Self { window };
        if !add_icon(window) {
            bail!("the notification area refused the icon");
        }
        Ok(icon)
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        let data = notify_data(self.window);
        // SAFETY: the icon and window this made, on the thread that made them.
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &raw const data);
            DestroyWindow(self.window);
        }
        STATE.set(None);
    }
}

/// The icon's description, naming it by its window and id.
fn notify_data(window: HWND) -> NOTIFYICONDATAW {
    // SAFETY: an all-zero NOTIFYICONDATAW is valid: no flags, null handles, empty strings.
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = window;
    data.uID = ICON_ID;
    data
}

/// Adds the icon to the notification area. Returns whether it was taken.
fn add_icon(window: HWND) -> bool {
    let mut data = notify_data(window);
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = CALLBACK_MESSAGE;
    for (slot, unit) in data.szTip.iter_mut().zip(TOOLTIP.encode_utf16()) {
        *slot = unit;
    }
    // SAFETY: the executable's own icon resource, shared so it needs no freeing; `data` lives
    // through both calls.
    unsafe {
        let resource = std::ptr::without_provenance::<u16>(ICON_RESOURCE);
        let (width, height) = (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON));
        let flags = LR_DEFAULTCOLOR | LR_SHARED;
        data.hIcon =
            LoadImageW(GetModuleHandleW(null()), resource, IMAGE_ICON, width, height, flags);
        if Shell_NotifyIconW(NIM_ADD, &raw const data) == 0 {
            return false;
        }
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        Shell_NotifyIconW(NIM_SETVERSION, &raw const data);
    }
    true
}

/// The hidden window's procedure: the icon's clicks, and Explorer restarting.
unsafe extern "system" fn window_procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let (sender, taskbar_created) = STATE.with_borrow(|state| {
        state
            .as_ref()
            .map_or((None, 0), |state| (Some(state.sender.clone()), state.taskbar_created))
    });
    if message == CALLBACK_MESSAGE
        && let Some(sender) = sender
    {
        // With NOTIFYICON_VERSION_4 the low word of `lparam` is the event.
        let command = match (lparam & 0xFFFF) as u32 {
            WM_CONTEXTMENU => choose_from_menu(window),
            NIN_SELECT | NIN_KEYSELECT => Some(TrayCommand::ShowVlc),
            _ => None,
        };
        if let Some(command) = command {
            sender.unbounded_send(command).ok();
        }
        return 0;
    }
    if taskbar_created != 0 && message == taskbar_created {
        add_icon(window);
        return 0;
    }
    // SAFETY: the arguments Windows passed in.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// Shows the menu at the pointer and returns what was chosen.
fn choose_from_menu(window: HWND) -> Option<TrayCommand> {
    let labels: Vec<Vec<u16>> = TrayCommand::MENU
        .iter()
        .flat_map(|group| group.iter())
        .map(|command| wide(command.label()))
        .collect();
    let mut labels = labels.iter();
    // SAFETY: a menu this makes and destroys, with labels that outlive it; the foreground and
    // the empty message are what Windows asks for so the menu closes when clicked away from.
    let chosen = unsafe {
        let menu = CreatePopupMenu();
        for (index, group) in TrayCommand::MENU.iter().enumerate() {
            if index > 0 {
                AppendMenuW(menu, MF_SEPARATOR, 0, null());
            }
            for (command, label) in group.iter().zip(labels.by_ref()) {
                AppendMenuW(menu, MF_STRING, command.id() as usize, label.as_ptr());
            }
        }
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&raw mut point);
        SetForegroundWindow(window);
        let flags = TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON;
        let chosen = TrackPopupMenu(menu, flags, point.x, point.y, 0, window, null());
        PostMessageW(window, WM_NULL, 0, 0);
        DestroyMenu(menu);
        chosen
    };
    TrayCommand::from_id(i64::from(chosen))
}

/// `text` as a null-terminated UTF-16 string.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

/// Windows always has a notification area.
pub fn is_available() -> bool {
    true
}

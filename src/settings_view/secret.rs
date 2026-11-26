//! A password row's field (Settings → Privacy "Last.fm password", S5.5.3): what is typed goes
//! to the OS keychain ([`keychain`]) on Enter or when the field loses focus, and the field
//! empties again. Its hint says whether a password is saved, and Forget removes it. The secret
//! is never read back into the field.
//!
//! Without a keychain the field stays disabled and its hint says why: a password is never kept
//! anywhere else.

use gpui::{
    App, AppContext as _, Context, Entity, KeyDownEvent, Subscription, Window, div, prelude::*, px,
};

use crate::i18n::DirectionalStyle;
use crate::privacy::keychain::{self, KeychainError, Secret, Status};
use crate::scrobble;
use crate::ui::button::Button;
use crate::ui::text_input::{self, TextInput, Validation};

/// The space between the field and Forget.
const BUTTON_GAP: f32 = 8.;
/// The hint while the keychain is being asked.
const CHECKING: &str = tr_noop!("Checking the keychain…");
/// The hint with a password saved.
const SAVED: &str = tr_noop!("Saved in the keychain");
/// The hint without one.
const NOT_SAVED: &str = tr_noop!("Not saved");

/// A password field backed by the keychain.
pub(super) struct SecretField {
    secret: Secret,
    input: Entity<TextInput>,
    /// What the keychain holds; `None` while it is being asked.
    status: Option<Status>,
    /// Whether the row's dependency is unmet.
    parent_off: bool,
    _events: Subscription,
}

impl SecretField {
    /// The field for `secret`, labelled `label`, asking the keychain whether it is saved.
    pub(super) fn new(
        secret: Secret,
        label: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .masked()
                .with_label(label)
                .with_placeholder(crate::i18n::text(CHECKING))
        });
        let events =
            cx.subscribe(&input, |this, input, event: &text_input::Event, cx| match event {
                text_input::Event::Changed => {
                    input.update(cx, |input, cx| input.set_validation(Validation::None, cx));
                }
                text_input::Event::Submitted | text_input::Event::Blurred => this.save(cx),
            });
        let mut this = Self { secret, input, status: None, parent_off: false, _events: events };
        this.refresh(cx);
        this
    }

    /// Dims and disables the field while the row's parent setting is off.
    pub(super) fn set_parent_off(&mut self, off: bool, cx: &mut Context<Self>) {
        if self.parent_off != off {
            self.parent_off = off;
            self.show_status(cx);
        }
    }

    /// Asks the keychain whether the secret is saved, off the UI thread.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let secret = self.secret;
        let status = cx.background_spawn(async move { keychain::status(secret) });
        cx.spawn(async move |this, cx| {
            let status = status.await;
            this.update(cx, |this, cx| {
                this.status = Some(status);
                this.show_status(cx);
            })
            .ok();
        })
        .detach();
    }

    /// Saves what was typed, if anything, and empties the field once it is saved.
    fn save(&mut self, cx: &mut Context<Self>) {
        let typed = self.input.read(cx).text().to_owned();
        if typed.is_empty() || !self.can_edit() {
            return;
        }
        let secret = self.secret;
        let saved = cx.background_spawn(async move { keychain::save(secret, &typed) });
        Self::after(saved, cx);
    }

    /// Removes the secret from the keychain.
    fn forget(&mut self, cx: &mut Context<Self>) {
        let secret = self.secret;
        let forgotten = cx.background_spawn(async move { keychain::forget(secret) });
        Self::after(forgotten, cx);
    }

    /// Once the keychain did `done`, shows what it holds, or why it failed under the field.
    fn after(done: gpui::Task<Result<(), KeychainError>>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = done.await;
            this.update(cx, |this, cx| match result {
                Ok(()) => this.changed(cx),
                Err(error) => {
                    let message = Validation::Error(error.message().to_owned().into());
                    this.input.update(cx, |input, cx| input.set_validation(message, cx));
                }
            })
            .ok();
        })
        .detach();
    }

    /// The keychain took the change: the field empties, the hint says what is saved now, and
    /// scrobbling signs in again with a new last.fm password (S8.17.1).
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text("", cx));
        if self.secret == Secret::LastFmPassword {
            scrobble::password_changed(cx);
        }
        self.refresh(cx);
    }

    /// Whether a password can be typed: the parent is on and the keychain is there.
    fn can_edit(&self) -> bool {
        !self.parent_off && matches!(self.status, Some(Status::Saved | Status::NotSaved))
    }

    /// Puts the status in the field's hint and enables it when it can be edited.
    fn show_status(&mut self, cx: &mut Context<Self>) {
        let hint = match &self.status {
            None => crate::i18n::text(CHECKING).to_owned(),
            Some(Status::Saved) => crate::i18n::text(SAVED).to_owned(),
            Some(Status::NotSaved) => crate::i18n::text(NOT_SAVED).to_owned(),
            Some(Status::Unavailable(why)) => why.clone(),
        };
        let disabled = !self.can_edit();
        self.input.update(cx, |input, cx| {
            input.set_placeholder(hint, cx);
            input.set_disabled(disabled, cx);
        });
        cx.notify();
    }

    /// Escape drops what was typed, before anything else handles it.
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let escape = event.keystroke.key == "escape" && !event.keystroke.modifiers.modified();
        if escape && !self.input.read(cx).text().is_empty() {
            self.input.update(cx, |input, cx| {
                input.set_text("", cx);
                input.set_validation(Validation::None, cx);
            });
            cx.stop_propagation();
        }
    }
}

impl Render for SecretField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = self.status == Some(Status::Saved);
        let forget = saved.then(|| {
            let this = cx.entity().downgrade();
            Button::new("forget", tr!("Forget")).disabled(self.parent_off).on_click(
                move |_, _, cx: &mut App| {
                    this.update(cx, SecretField::forget).ok();
                },
            )
        });
        div()
            .w_full()
            .flex_row_start()
            .items_start()
            .gap(px(BUTTON_GAP))
            .on_key_down(cx.listener(Self::on_key_down))
            .child(div().flex_1().min_w_0().child(self.input.clone()))
            .children(forget.map(|button| div().flex_none().child(button)))
    }
}

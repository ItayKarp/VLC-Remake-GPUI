//! The settings panel's Subtitles tab (S6.3, `controls-subtitle_settings.png`): **Defaults** for
//! media opened next (`track_defaults.rs`), and **Synchronization** for the subtitle showing
//! now: Subtitle Track Delay (a field to the millisecond and a −10 s … +10 s slider filled from
//! 0 s), Subtitle Speed and Subtitle Duration Factor, and Reset, which puts that subtitle's three
//! back. Each heading has a line under it saying what its rows change, since the two sections
//! look alike but one changes the defaults and the other this video.
//!
//! The sync belongs to the subtitle showing (D24), and is remembered for it in this video
//! (S6.3.4). With no subtitle showing the rows are disabled. Speed and duration factor only
//! reach subtitles the app draws (S6.2), so they're disabled, with a note, while libVLC draws
//! the one showing (pictures, styled ASS/SSA). The designed Appearance rows are still to come:
//! the session values they need exist now (S7.1, `effects`).

use gpui::{Context, Div, Entity, Subscription, Window, div, prelude::*, px};

use super::number_row::{Changed, NumberRow, Spec};
use super::section;
use super::sync_rules;
use super::track_defaults::{DefaultsRows, TrackDefaults};
use crate::engine::subtitle_sync::{
    MAX_DELAY_MS, MAX_DURATION_FACTOR, MAX_SPEED, MIN_SPEED, SubtitleKey, SubtitleSync,
};
use crate::engine::tracks::TrackKind;
use crate::engine::values::Offset;
use crate::i18n::DirectionalStyle;
use crate::main_window::MainWindow;
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::slider::SliderModel;

/// The delay slider's reach either way, in seconds, and its ticks.
const DELAY_SLIDER: f64 = 10.;
/// Arrow keys move the delay by 50 ms, as G and H do; Shift by a second.
const DELAY_STEP: f64 = 0.05;
const DELAY_BIG_STEP: f64 = 1.;
/// How finely the speed and duration factor sliders move.
const SPEED_STEP: f64 = 0.001;
const DURATION_STEP: f64 = 0.05;
/// Milliseconds in a second.
pub(super) const MS_PER_SECOND: f64 = 1_000.;

/// Which of a sync's values a row edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyncValue {
    Delay,
    Speed,
    Duration,
}

/// The subtitle showing, as the Synchronization section tells of it.
struct Showing {
    /// "For “Movie.en.srt” in this video."
    target: String,
    sync: SubtitleSync,
    /// Whether speed and duration factor reach it (the app draws it).
    stretches: bool,
}

/// The tab's controls.
pub(in crate::main_window) struct SubtitleControls {
    pub(super) defaults: DefaultsRows,
    delay: Entity<NumberRow>,
    speed: Entity<NumberRow>,
    duration: Entity<NumberRow>,
    _events: [Subscription; 3],
}

impl SubtitleControls {
    /// The controls with no subtitle showing.
    pub(in crate::main_window) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let delay = cx.new(|cx| NumberRow::new(delay_spec(), window, cx));
        let speed = cx.new(|cx| NumberRow::new(speed_spec(), window, cx));
        let duration = cx.new(|cx| NumberRow::new(duration_spec(), window, cx));
        let follow = |row: &Entity<NumberRow>, value, cx: &mut Context<MainWindow>| {
            cx.subscribe(row, move |this, _, event: &Changed, cx| {
                this.change_sync(value, event.0, cx);
            })
        };
        let events = [
            follow(&delay, SyncValue::Delay, cx),
            follow(&speed, SyncValue::Speed, cx),
            follow(&duration, SyncValue::Duration, cx),
        ];
        let defaults = DefaultsRows::new(TrackDefaults::Subtitle, window, cx);
        Self { defaults, delay, speed, duration, _events: events }
    }
}

impl MainWindow {
    /// The Subtitles tab: Defaults, then Synchronization.
    pub(super) fn render_subtitles_tab(
        &mut self,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        self.sync_subtitle_controls(window, cx);
        div()
            .flex_row_start()
            .flex_col()
            .child(self.render_subtitle_defaults(p, cx))
            .child(div().h(px(1.)).bg(p.divider))
            .child(self.render_synchronization(p, cx))
    }

    fn render_subtitle_defaults(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let rows = self.settings_panel.subtitles.defaults.render(p, cx);
        section(
            tr!("Defaults"),
            tr!("For videos you open next. The subtitle button picks this video's."),
            p,
        )
        .children(rows)
    }

    fn render_synchronization(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let showing = self.showing_sync(cx);
        let note = match &showing {
            None => tr!("Pick a subtitle to sync it with the video.").to_owned(),
            Some(showing) if !showing.stretches => tr!(
                "{target} Speed and duration only change text files VLC draws itself.",
                target = showing.target
            ),
            Some(showing) => showing.target.clone(),
        };
        let unchanged = showing.as_ref().is_none_or(|showing| showing.sync.is_default());
        let reset = Button::new("reset-subtitle-sync", tr!("Reset"))
            .style(ButtonStyle::Ghost)
            .disabled(unchanged)
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_subtitle_sync(SubtitleSync::default(), cx);
            }));
        let controls = &self.settings_panel.subtitles;
        section(tr!("Synchronization"), note, p)
            .child(controls.delay.clone())
            .child(controls.speed.clone())
            .child(controls.duration.clone())
            .child(div().flex_row_start().justify_end().child(reset))
    }

    /// The subtitle showing and its sync; `None` while none shows.
    fn showing_sync(&self, cx: &Context<Self>) -> Option<Showing> {
        let player = self.loaded_player(cx)?;
        let player = player.read(cx);
        let key = player.subtitle_key()?;
        let name = match key {
            SubtitleKey::File(path) => {
                path.file_name().map(|name| name.to_string_lossy().into_owned())
            }
            SubtitleKey::Stream(_) => {
                let tracks = player.tracks();
                tracks.selected(TrackKind::Subtitle).and_then(|id| tracks.label(id))
            }
        };
        Some(Showing {
            target: name.map_or_else(String::new, |name| tr!("For “{name}” in this video.", name)),
            sync: player.subtitle_sync(),
            stretches: player.sync_stretches(),
        })
    }

    /// Shows the settings and the showing subtitle's sync in the controls.
    fn sync_subtitle_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_panel.subtitles.defaults.sync(window, cx);
        let showing = self.showing_sync(cx);
        let sync = showing.as_ref().map_or_default(|showing| showing.sync);
        let stretches = showing.as_ref().is_some_and(|showing| showing.stretches);
        let controls = &self.settings_panel.subtitles;
        let rows = [
            (&controls.delay, sync.delay().as_millis() as f64 / MS_PER_SECOND, showing.is_some()),
            (&controls.speed, f64::from(sync.speed()), stretches),
            (&controls.duration, f64::from(sync.duration_factor()), stretches),
        ];
        for (row, value, enabled) in rows {
            row.update(cx, |row, cx| {
                row.set_disabled(!enabled, cx);
                row.show(value, window, cx);
            });
        }
    }

    /// Changes one value of the showing subtitle's sync, as its row asked.
    fn change_sync(&mut self, value: SyncValue, to: f64, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let sync = player.read(cx).subtitle_sync();
        let sync = match value {
            SyncValue::Delay => {
                sync.with_delay(Offset::from_millis((to * MS_PER_SECOND).round() as i64))
            }
            SyncValue::Speed => sync.with_speed(to as f32),
            SyncValue::Duration => sync.with_duration_factor(to as f32),
        };
        self.set_subtitle_sync(sync, cx);
        cx.notify();
    }
}

/// Subtitle Track Delay: −10 s … +10 s on the slider, ten minutes either way in the field.
fn delay_spec() -> Spec {
    track_delay_spec(tr!("Subtitle Track Delay"))
}

/// A track delay labelled `label`: −10 s … +10 s on the slider, ten minutes either way in the
/// field, the arrow keys moving 50 ms as the delay keys do (S6.3.2, S7.6.3).
pub(super) fn track_delay_spec(label: &'static str) -> Spec {
    let mut model = SliderModel::new(-DELAY_SLIDER, DELAY_SLIDER, DELAY_STEP, 0.);
    model.big_step = DELAY_BIG_STEP;
    debug_assert!(MAX_DELAY_MS as f64 / MS_PER_SECOND >= DELAY_SLIDER);
    Spec {
        label,
        model,
        centered: true,
        marks: Some((DELAY_SLIDER, sync_rules::delay_mark)),
        format: sync_rules::delay_text,
        parse: sync_rules::parse_delay,
    }
}

/// Subtitle Speed: 0.5× to 2×.
fn speed_spec() -> Spec {
    Spec {
        label: tr!("Subtitle Speed"),
        model: SliderModel::new(f64::from(MIN_SPEED), f64::from(MAX_SPEED), SPEED_STEP, 1.),
        centered: false,
        marks: None,
        format: sync_rules::speed_text,
        parse: sync_rules::parse_speed,
    }
}

/// Subtitle Duration Factor: off (0) to 20.
fn duration_spec() -> Spec {
    Spec {
        label: tr!("Subtitle Duration Factor"),
        model: SliderModel::new(0., f64::from(MAX_DURATION_FACTOR), DURATION_STEP, 0.),
        centered: false,
        marks: None,
        format: sync_rules::duration_text,
        parse: sync_rules::parse_duration,
    }
}

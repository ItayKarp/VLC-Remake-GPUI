//! What a measured run saw: the marks the app sets as it goes, and the main window's frames.
//! Every method takes the time (and CPU time) it happens at, so tests can drive it.

use std::time::{Duration, Instant};

use super::frames::{Frame, FrameTally};

/// How long after pausing the window may still redraw for the pause itself (the play button's
/// icon, the status line) before frames and CPU count as idle.
pub const SETTLE: Duration = Duration::from_secs(1);

/// What the player is doing, as far as frames are judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Media plays: frames are timed against the frame budget.
    Playing,
    /// Media is paused: nothing should redraw.
    Paused,
    /// Anything else (opening, stopped, nothing open): not judged.
    Other,
}

/// The library's first scan: how many files it found and how long it took.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndexRun {
    /// Media files found.
    pub files: u64,
    /// From the scan starting to its end.
    pub took: Duration,
}

/// Paused time and the CPU used during it, after each pause settled.
#[derive(Clone, Copy, Debug, Default)]
pub struct Idle {
    /// Wall time spent paused.
    pub wall: Duration,
    /// CPU time used meanwhile; `None` where the OS doesn't say.
    pub cpu: Option<Duration>,
}

/// A pause going on: when it settles (or settled), and whether it has.
#[derive(Clone, Copy, Debug)]
struct Stretch {
    from: Instant,
    cpu: Sample,
}

/// The CPU time at the start of a settled pause.
#[derive(Clone, Copy, Debug)]
enum Sample {
    /// The pause hasn't settled yet.
    Pending,
    /// Taken when it settled; `None` where the OS doesn't say.
    Taken(Option<Duration>),
}

/// Everything a measured run recorded.
#[derive(Debug)]
pub struct Recorder {
    start: Instant,
    ready: Option<Duration>,
    opening: Option<Instant>,
    opens: Vec<Duration>,
    /// Phase changes, in order, so frames are judged by the phase they were drawn in.
    phases: Vec<(Instant, Phase)>,
    idle_since: Option<Stretch>,
    idle: Idle,
    frames: FrameTally,
    index: Option<IndexRun>,
    library_items: Option<u64>,
}

impl Recorder {
    /// A run whose process started at `start`.
    pub fn new(start: Instant) -> Self {
        Self {
            start,
            ready: None,
            opening: None,
            opens: Vec::new(),
            phases: vec![(start, Phase::Other)],
            idle_since: None,
            idle: Idle { wall: Duration::ZERO, cpu: Some(Duration::ZERO) },
            frames: FrameTally::default(),
            index: None,
            library_items: None,
        }
    }

    /// The main window drew its first frame at `at`; later calls are ignored.
    pub fn ready(&mut self, at: Instant) {
        self.ready.get_or_insert(at.saturating_duration_since(self.start));
    }

    /// From the process starting to the main window's first frame.
    pub fn cold_start(&self) -> Option<Duration> {
        self.ready
    }

    /// A file was asked to open at `at`.
    pub fn opening(&mut self, at: Instant) {
        self.opening = Some(at);
    }

    /// A picture reached the screen at `at`: the first since an open finishes timing it.
    pub fn picture(&mut self, at: Instant) {
        if let Some(asked) = self.opening.take() {
            self.opens.push(at.saturating_duration_since(asked));
        }
    }

    /// The slowest open's time to its first picture.
    pub fn slowest_open(&self) -> Option<Duration> {
        self.opens.iter().max().copied()
    }

    /// The player entered `phase` at `at`, when the process had used `cpu`.
    pub fn phase(&mut self, phase: Phase, at: Instant, cpu: Option<Duration>) {
        if self.phases.last().is_some_and(|&(_, last)| last == phase) {
            return;
        }
        self.end_idle(at, cpu);
        if phase == Phase::Paused {
            self.idle_since = Some(Stretch { from: at + SETTLE, cpu: Sample::Pending });
        }
        self.phases.push((at, phase));
    }

    /// Takes the CPU time once a pause has settled, so idle CPU starts from there.
    pub fn tick(&mut self, now: Instant, cpu: Option<Duration>) {
        if let Some(stretch) = &mut self.idle_since
            && matches!(stretch.cpu, Sample::Pending)
            && now >= stretch.from
        {
            *stretch = Stretch { from: now, cpu: Sample::Taken(cpu) };
        }
    }

    /// Paused time and its CPU, counting the pause going on at `now` too.
    pub fn idle(&self, now: Instant, cpu: Option<Duration>) -> Idle {
        let mut idle = self.idle;
        if let Some(since) = self.idle_since {
            add_idle(&mut idle, since, now, cpu);
        }
        idle
    }

    fn end_idle(&mut self, at: Instant, cpu: Option<Duration>) {
        if let Some(since) = self.idle_since.take() {
            add_idle(&mut self.idle, since, at, cpu);
        }
    }

    /// The main window drew `frame`, starting at `at`.
    #[cfg_attr(
        not(feature = "perf"),
        allow(
            dead_code,
            reason = "frames come from GPUI's frame timing, built by the perf feature"
        )
    )]
    pub fn frame(&mut self, frame: Frame, at: Instant) {
        match self.phase_at(at) {
            Phase::Playing => self.frames.playing(frame),
            Phase::Paused if self.settled_at(at) => self.frames.idle(),
            Phase::Paused | Phase::Other => {}
        }
    }

    /// The frames drawn so far.
    pub fn frames(&self) -> &FrameTally {
        &self.frames
    }

    #[cfg_attr(not(feature = "perf"), allow(dead_code, reason = "used by `frame`"))]
    fn phase_at(&self, at: Instant) -> Phase {
        let index = self.phases.partition_point(|&(since, _)| since <= at);
        self.phases.get(index.saturating_sub(1)).map_or(Phase::Other, |&(_, phase)| phase)
    }

    /// Whether `at` is past the settling time of the pause it falls in.
    #[cfg_attr(not(feature = "perf"), allow(dead_code, reason = "used by `frame`"))]
    fn settled_at(&self, at: Instant) -> bool {
        let index = self.phases.partition_point(|&(since, _)| since <= at);
        self.phases.get(index.saturating_sub(1)).is_some_and(|&(since, _)| at >= since + SETTLE)
    }

    /// The library's first scan finished; later scans are ignored.
    pub fn indexed(&mut self, run: IndexRun) {
        self.index.get_or_insert(run);
    }

    /// The library's first scan.
    pub fn index(&self) -> Option<IndexRun> {
        self.index
    }

    /// The library holds `items` media.
    pub fn library_items(&mut self, items: u64) {
        self.library_items = Some(items);
    }

    /// How many media the library held when last told.
    pub fn items(&self) -> Option<u64> {
        self.library_items
    }
}

/// Adds `stretch` up to `now` to `idle`, unless it hadn't settled by then.
fn add_idle(idle: &mut Idle, stretch: Stretch, now: Instant, cpu_now: Option<Duration>) {
    let Sample::Taken(cpu_then) = stretch.cpu else { return };
    if now <= stretch.from {
        return;
    }
    idle.wall += now - stretch.from;
    idle.cpu = match (idle.cpu, cpu_then, cpu_now) {
        (Some(total), Some(then), Some(now)) => Some(total + now.saturating_sub(then)),
        _ => None,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    fn frame() -> Frame {
        Frame { draw: ms(4), present: ms(1), refresh: None }
    }

    #[test]
    fn times_the_first_frame_and_each_open() {
        let start = Instant::now();
        let mut run = Recorder::new(start);
        run.ready(start + ms(700));
        run.ready(start + ms(900));
        assert_eq!(run.cold_start(), Some(ms(700)));
        run.opening(start + ms(1_000));
        run.picture(start + ms(1_300));
        run.picture(start + ms(1_400));
        run.opening(start + ms(2_000));
        run.picture(start + ms(2_100));
        assert_eq!(run.slowest_open(), Some(ms(300)));
    }

    #[test]
    fn frames_count_by_the_phase_they_were_drawn_in() {
        let start = Instant::now();
        let mut run = Recorder::new(start);
        run.phase(Phase::Playing, start, None);
        run.phase(Phase::Paused, start + ms(1_000), None);
        run.frame(frame(), start + ms(500));
        run.frame(frame(), start + ms(1_100));
        run.frame(frame(), start + ms(2_500));
        assert_eq!(run.frames().playing_p95_ms(), Some(5.));
        assert_eq!(run.frames().idle_frames(), 1, "the frame drawn for the pause doesn't count");
    }

    #[test]
    fn idle_cpu_starts_once_the_pause_settles() {
        let start = Instant::now();
        let mut run = Recorder::new(start);
        run.phase(Phase::Paused, start, Some(ms(100)));
        run.tick(start + ms(500), Some(ms(150)));
        run.tick(start + SETTLE, Some(ms(200)));
        run.phase(Phase::Playing, start + SETTLE + ms(4_000), Some(ms(240)));
        let idle = run.idle(start + ms(9_000), Some(ms(900)));
        assert_eq!(idle.wall, ms(4_000));
        assert_eq!(idle.cpu, Some(ms(40)));
    }

    #[test]
    fn a_pause_going_on_counts_up_to_now() {
        let start = Instant::now();
        let mut run = Recorder::new(start);
        run.phase(Phase::Paused, start, Some(ms(0)));
        run.tick(start + SETTLE, Some(ms(10)));
        let idle = run.idle(start + SETTLE + ms(2_000), Some(ms(30)));
        assert_eq!((idle.wall, idle.cpu), (ms(2_000), Some(ms(20))));
    }

    #[test]
    fn only_the_first_scan_counts() {
        let mut run = Recorder::new(Instant::now());
        run.indexed(IndexRun { files: 10, took: ms(20) });
        run.indexed(IndexRun { files: 99, took: ms(1) });
        assert_eq!(run.index().map(|run| run.files), Some(10));
    }
}

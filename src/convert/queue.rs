//! The conversion queue (S8.10.1): what was asked for, in order, and how each is going. At most
//! [`MAX_RUNNING`] convert at once, as each one keeps processor cores busy; the rest wait their
//! turn, so a hundred files don't start together.
//!
//! The jobs asked for while the queue was busy are one batch: when the last of them ends, the
//! batch's [`Summary`] is what the notification says.

use std::path::{Path, PathBuf};

use super::Preset;
use super::output::{self, Destination};
use crate::app::media::Media;

/// How many conversions run at once.
pub const MAX_RUNNING: usize = 2;

/// A conversion's id in the queue.
pub type JobId = u64;

/// How a conversion is going.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Waiting for a turn.
    Waiting,
    /// Converting; how far it got, 0 to 1.
    Converting(f32),
    /// Finished: the output is complete.
    Done,
    /// It couldn't be converted, and why.
    Failed(String),
    /// The user cancelled it.
    Cancelled,
}

impl Status {
    /// Whether it is over, one way or another.
    pub fn is_over(&self) -> bool {
        matches!(self, Self::Done | Self::Failed(_) | Self::Cancelled)
    }
}

/// One conversion.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    /// Its id.
    pub id: JobId,
    /// What it converts.
    pub source: Media,
    /// What it converts to.
    pub preset: Preset,
    /// The file it writes.
    pub output: PathBuf,
    /// How it is going.
    pub status: Status,
}

/// What a batch came to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// The files made.
    pub done: Vec<PathBuf>,
    /// Why each failed one failed.
    pub failed: Vec<String>,
}

/// The conversions asked for this session.
#[derive(Debug, Default)]
pub struct Queue {
    jobs: Vec<Job>,
    next_id: JobId,
    /// The jobs of the batch going on.
    batch: Vec<JobId>,
}

impl Queue {
    /// Adds a conversion of `source` to `preset` under `destination`, its output named so it
    /// writes over neither what `exists` nor another conversion's output. Returns its id.
    pub fn add(
        &mut self,
        source: Media,
        preset: Preset,
        destination: &Destination,
        exists: impl Fn(&Path) -> bool,
    ) -> JobId {
        let planned =
            |path: &Path| self.jobs.iter().any(|job| job.output == path && !job.status.is_over());
        let output =
            output::plan(&source, preset, destination, |path| exists(path) || planned(path));
        let id = self.next_id;
        self.next_id += 1;
        self.jobs.push(Job { id, source, preset, output, status: Status::Waiting });
        self.batch.push(id);
        id
    }

    /// Every conversion, in the order asked for.
    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    /// Whether a conversion is waiting or running.
    pub fn is_busy(&self) -> bool {
        self.jobs.iter().any(|job| !job.status.is_over())
    }

    /// How many conversions are waiting or running.
    pub fn unfinished(&self) -> usize {
        self.jobs.iter().filter(|job| !job.status.is_over()).count()
    }

    /// The waiting conversions that may start now, marked as converting.
    pub fn start_due(&mut self) -> Vec<Job> {
        let running =
            self.jobs.iter().filter(|job| matches!(job.status, Status::Converting(_))).count();
        let free = MAX_RUNNING.saturating_sub(running);
        let due = self.jobs.iter_mut().filter(|job| job.status == Status::Waiting).take(free);
        due.map(|job| {
            job.status = Status::Converting(0.);
            job.clone()
        })
        .collect()
    }

    /// Conversion `id` got `fraction` of the way.
    pub fn set_progress(&mut self, id: JobId, fraction: f32) {
        if let Some(job) =
            self.job_mut(id).filter(|job| matches!(job.status, Status::Converting(_)))
        {
            job.status = Status::Converting(fraction.clamp(0., 1.));
        }
    }

    /// Conversion `id` ended with `result`, unless it was cancelled first.
    pub fn finish(&mut self, id: JobId, result: Result<(), String>) {
        if let Some(job) = self.job_mut(id).filter(|job| !job.status.is_over()) {
            job.status = match result {
                Ok(()) => Status::Done,
                Err(why) => Status::Failed(why),
            };
        }
    }

    /// Cancels conversion `id`. Returns whether it was running, so its work must be stopped.
    pub fn cancel(&mut self, id: JobId) -> bool {
        let Some(job) = self.job_mut(id).filter(|job| !job.status.is_over()) else { return false };
        let running = matches!(job.status, Status::Converting(_));
        job.status = Status::Cancelled;
        running
    }

    /// Forgets the conversions that are over.
    pub fn clear_finished(&mut self) {
        self.jobs.retain(|job| !job.status.is_over());
    }

    /// Once nothing is waiting or running, what the batch came to, and a new batch begins;
    /// `None` while it goes on, or when every job of it was cancelled.
    pub fn take_summary(&mut self) -> Option<Summary> {
        if self.is_busy() || self.batch.is_empty() {
            return None;
        }
        let mut summary = Summary::default();
        for id in std::mem::take(&mut self.batch) {
            match self.jobs.iter().find(|job| job.id == id).map(|job| (&job.status, job)) {
                Some((Status::Done, job)) => summary.done.push(job.output.clone()),
                Some((Status::Failed(why), _)) => summary.failed.push(why.clone()),
                _ => {}
            }
        }
        (!summary.done.is_empty() || !summary.failed.is_empty()).then_some(summary)
    }

    fn job_mut(&mut self, id: JobId) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|job| job.id == id)
    }
}

#[cfg(test)]
mod tests;

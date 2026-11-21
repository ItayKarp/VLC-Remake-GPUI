//! Reading how long queued media is without playing it (S3.2.2), for the queue panel's rows and
//! total. A [`LengthProbe`] works through what it's asked off the UI thread and answers on a
//! channel; [`default_probe`] reads local files' headers with libVLC, and falls back to one that
//! knows no lengths when libVLC is missing.

use std::time::Duration;

use futures::channel::mpsc;

use crate::app::media::Media;

/// One answer: `media` lasts this long, or has no length that could be read.
pub type LengthAnswer = (Media, Option<Duration>);

/// The UI's end of the answer channel.
pub type LengthReceiver = mpsc::UnboundedReceiver<LengthAnswer>;

/// Sends answers to the UI; safe to use from any thread.
#[derive(Clone, Debug)]
pub struct LengthSender(mpsc::UnboundedSender<LengthAnswer>);

impl LengthSender {
    /// Queues `answer` for the UI. Does nothing once the UI is gone.
    pub fn send(&self, answer: LengthAnswer) {
        // An error only means the receiver was dropped; nobody is left to tell.
        let _ = self.0.unbounded_send(answer);
    }
}

/// A new answer channel.
pub fn channel() -> (LengthSender, LengthReceiver) {
    let (sender, receiver) = mpsc::unbounded();
    (LengthSender(sender), receiver)
}

/// Reads media lengths in the background. Every method returns at once.
pub trait LengthProbe {
    /// Adds `media` to what's waiting to be read, in order.
    fn probe(&mut self, media: Vec<Media>);
    /// Drops everything still waiting (the queue changed).
    fn clear(&mut self);
}

/// The probe to use: libVLC, or one that knows no lengths when libVLC can't be loaded.
pub fn default_probe(answers: LengthSender) -> Box<dyn LengthProbe> {
    match super::libvlc::lengths::LibVlcLengths::new(answers.clone()) {
        Ok(probe) => Box::new(probe),
        Err(error) => {
            tracing::warn!("lengths: no length probe: {error:#}");
            Box::new(NoLengths(answers))
        }
    }
}

/// Answers that no length is known: rows then show none and the total ends in "+".
pub struct NoLengths(pub LengthSender);

impl LengthProbe for NoLengths {
    fn probe(&mut self, media: Vec<Media>) {
        for media in media {
            self.0.send((media, None));
        }
    }

    fn clear(&mut self) {}
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use futures::StreamExt;

    use super::*;

    #[test]
    fn without_libvlc_every_length_is_unknown() {
        let (sender, mut receiver) = channel();
        let mut probe = NoLengths(sender);
        let media = Media::File(PathBuf::from("a.mkv"));
        probe.probe(vec![media.clone()]);
        let answer = futures::executor::block_on(receiver.next());
        assert_eq!(answer, Some((media, None)));
    }
}

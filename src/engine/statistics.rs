//! What the backend counted about the media playing, for Show Statistics (S8.13.2): how fast
//! data arrives, how many pictures were decoded, shown and dropped, and whether the GPU may
//! decode the video.

/// Kilobits per second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kbps(pub f32);

/// How the open media's video is decoded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VideoDecoding {
    /// The GPU decodes it where the platform's decoder takes the codec (VideoToolbox, DXVA,
    /// VA-API); the CPU decodes the rest.
    #[default]
    Hardware,
    /// The CPU decodes it: hardware decoding is off in Settings, crashed on this media or GPU
    /// before, or a subtitle, rotation or filter needs software pictures.
    Software,
}

/// The backend's counts for the media open now, since it opened (or last reopened).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Statistics {
    /// How fast data arrives from the file or network.
    pub input_rate: Kbps,
    /// How fast the media's own streams arrive, after the container is unpacked.
    pub stream_rate: Kbps,
    /// Pictures decoded.
    pub decoded_pictures: u64,
    /// Pictures shown.
    pub shown_pictures: u64,
    /// Pictures dropped: decoded too late to show, or lost.
    pub dropped_pictures: u64,
    /// Audio buffers dropped.
    pub dropped_audio: u64,
    /// How the video is decoded.
    pub decoding: VideoDecoding,
}

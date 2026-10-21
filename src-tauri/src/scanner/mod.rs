#[cfg(feature = "gui")]
pub mod hasher;
pub mod metadata;
pub mod models;
#[cfg(feature = "gui")]
pub mod scan;

pub use metadata::{extract_track_info, is_instrumental_lyrics, LyricsInfo, TrackMetadata};
#[cfg(feature = "gui")]
pub use scan::{scan_library, DetectionMethod};

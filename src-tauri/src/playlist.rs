use serde::{Deserialize, Serialize};

/// What happens when the current track ends.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    /// Stop after the queue is exhausted.
    #[default]
    Off,
    /// Replay the current track forever.
    One,
    /// Wrap around to the start of the queue.
    All,
}

/// The play queue.
///
/// Deliberately holds track ids only: resolving an id to playable metadata is
/// the caller's job, which keeps the queue trivially cloneable into a snapshot
/// and free of audio/file-path concerns.
#[derive(Clone, Debug, Default)]
pub struct Playlist {
    /// Ids in play order.
    ///
    /// In shuffle mode this is the *shuffled* order, so `next`/`previous` stay
    /// consistent with each other. Re-shuffling on every advance would make
    /// "previous" land somewhere the user never heard.
    ids: Vec<i64>,
    /// Index into `ids`. `None` when idle and nothing has been loaded.
    cursor: Option<usize>,
    /// Original order, restored when shuffle is turned off.
    original: Vec<i64>,
    shuffle: bool,
    repeat: RepeatMode,
    /// Where the index lands after the current one in `original`, used to keep
    /// the playing track selected when leaving shuffle.
    playing_original_index: Option<usize>,
}

impl Playlist {
    /// Replace the queue and start at `index`.
    pub fn load(&mut self, ids: Vec<i64>, index: usize) {
        self.original = ids.clone();
        self.ids = ids;
        self.shuffle = false;
        self.playing_original_index = Some(index);
        self.cursor = Some(index.min(self.ids.len().saturating_sub(1)));
    }

    /// Play a single track with no surrounding queue.
    pub fn load_single(&mut self, id: i64) {
        self.ids = vec![id];
        self.original = self.ids.clone();
        self.cursor = Some(0);
        self.shuffle = false;
        self.repeat = RepeatMode::Off;
        self.playing_original_index = Some(0);
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.original.clear();
        self.cursor = None;
        self.playing_original_index = None;
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn is_shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    pub fn current_id(&self) -> Option<i64> {
        self.cursor.and_then(|index| self.ids.get(index)).copied()
    }

    pub fn index(&self) -> Option<usize> {
        self.cursor
    }

    /// Whether `advance()` would move somewhere (wraps count under repeat-all).
    pub fn has_next(&self) -> bool {
        match self.cursor {
            None => false,
            Some(cursor) => {
                cursor + 1 < self.ids.len() || matches!(self.repeat, RepeatMode::All)
            }
        }
    }

    /// Whether `rewind()` would move somewhere.
    pub fn has_previous(&self) -> bool {
        match self.cursor {
            None => false,
            Some(cursor) => cursor > 0 || matches!(self.repeat, RepeatMode::All),
        }
    }

    /// Ids in the order they will be played.
    pub fn order(&self) -> &[i64] {
        &self.ids
    }

    /// Toggle shuffle.
    ///
    /// Turning it on shuffles everything except the currently playing track and
    /// re-seats the cursor at the front, so playback continues with the current
    /// song and then wanders. Turning it off restores the original order and
    /// keeps the current song selected.
    pub fn set_shuffle(&mut self, shuffle: bool) {
        if self.shuffle == shuffle {
            return;
        }

        if shuffle {
            let current = self.current_id();
            let mut rest: Vec<i64> = self
                .original
                .iter()
                .copied()
                .filter(|id| Some(*id) != current)
                .collect();
            shuffle_in_place(&mut rest);

            let mut ids = Vec::with_capacity(rest.len() + 1);
            if let Some(current) = current {
                ids.push(current);
            }
            ids.extend(rest);

            self.ids = ids;
            self.cursor = if self.current_id().is_some() { Some(0) } else { None };
            self.shuffle = true;
        } else {
            let current = self.current_id();
            self.ids = self.original.clone();
            self.cursor = current.and_then(|id| self.ids.iter().position(|candidate| *candidate == id));
            self.shuffle = false;
        }

        self.sync_playing_original_index();
    }

    fn sync_playing_original_index(&mut self) {
        self.playing_original_index = self
            .current_id()
            .and_then(|id| self.original.iter().position(|candidate| *candidate == id));
    }

    /// Move to the next track.
    ///
    /// Returns `None` when playback should stop instead (end of queue with
    /// `RepeatMode::Off`). `RepeatMode::One` is handled by the caller, which
    /// replays the current track: this method always advances so that an
    /// explicit "next" press is not swallowed by repeat-one.
    pub fn advance(&mut self) -> Option<i64> {
        let cursor = self.cursor?;
        if self.ids.is_empty() {
            return None;
        }

        let next = cursor + 1;
        if next < self.ids.len() {
            self.cursor = Some(next);
        } else if matches!(self.repeat, RepeatMode::All) {
            self.cursor = Some(0);
        } else {
            return None;
        }

        self.sync_playing_original_index();
        self.current_id()
    }

    /// Move to the previous track, wrapping only under `RepeatMode::All`.
    pub fn rewind(&mut self) -> Option<i64> {
        let cursor = self.cursor?;
        if self.ids.is_empty() {
            return None;
        }

        if cursor > 0 {
            self.cursor = Some(cursor - 1);
        } else if matches!(self.repeat, RepeatMode::All) {
            self.cursor = Some(self.ids.len() - 1);
        } else {
            return None;
        }

        self.sync_playing_original_index();
        self.current_id()
    }

    /// Called when a track ends on its own.
    ///
    /// Unlike `advance`, `RepeatMode::One` replays the current track here,
    /// which is what repeat-one means for natural playback.
    pub fn on_track_ended(&mut self) -> PlaybackDecision {
        match self.repeat {
            RepeatMode::One => match self.current_id() {
                Some(id) => PlaybackDecision::Replay(id),
                None => PlaybackDecision::Stop,
            },
            RepeatMode::Off | RepeatMode::All => match self.advance() {
                Some(id) => PlaybackDecision::Play(id),
                None => PlaybackDecision::Stop,
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackDecision {
    Play(i64),
    Replay(i64),
    Stop,
}

/// Fisher-Yates, seeded from the standard library's `RandomState`.
///
/// `rand` is not a dependency of this crate, and pulling it in for one shuffle
/// is not worth it: `RandomState` is seeded per-process from the OS and mixed
/// through SipHash, which is adequate for picking a track order.
fn shuffle_in_place(ids: &mut [i64]) {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let mut counter: u64 = 0;
    let mut next_u64 = {
        let state = RandomState::new();
        move || {
            let mut hasher = state.build_hasher();
            counter = counter.wrapping_add(0x9E37_79B9_7F4A_7C15);
            hasher.write_u64(counter);
            hasher.finish()
        }
    };

    if ids.len() < 2 {
        return;
    }

    for index in (1..ids.len()).rev() {
        let bound = (index + 1) as u64;
        // Rejection sampling keeps the distribution uniform.
        let limit = u64::MAX - (u64::MAX % bound);
        let mut value = next_u64();
        while value >= limit {
            value = next_u64();
        }
        let swap_with = (value % bound) as usize;
        ids.swap(index, swap_with);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist(ids: &[i64]) -> Playlist {
        let mut playlist = Playlist::default();
        playlist.load(ids.to_vec(), 0);
        playlist
    }

    #[test]
    fn load_sets_cursor_and_current() {
        let playlist = playlist(&[1, 2, 3]);
        assert_eq!(playlist.current_id(), Some(1));
        assert_eq!(playlist.len(), 3);
    }

    #[test]
    fn load_clamps_an_out_of_range_index() {
        let mut playlist = Playlist::default();
        playlist.load(vec![1, 2, 3], 99);
        assert_eq!(playlist.current_id(), Some(3));
    }

    #[test]
    fn load_single_has_no_queue() {
        let mut playlist = Playlist::default();
        playlist.load_single(7);
        assert_eq!(playlist.current_id(), Some(7));
        assert_eq!(playlist.len(), 1);
        assert_eq!(playlist.advance(), None);
    }

    #[test]
    fn advance_walks_the_queue_then_stops() {
        let mut playlist = playlist(&[1, 2, 3]);
        assert_eq!(playlist.advance(), Some(2));
        assert_eq!(playlist.advance(), Some(3));
        assert_eq!(playlist.advance(), None);
        // Cursor stays on the last track rather than falling off the end.
        assert_eq!(playlist.current_id(), Some(3));
    }

    #[test]
    fn advance_wraps_under_repeat_all() {
        let mut playlist = playlist(&[1, 2]);
        playlist.set_repeat(RepeatMode::All);
        assert_eq!(playlist.advance(), Some(2));
        assert_eq!(playlist.advance(), Some(1));
    }

    #[test]
    fn rewind_walks_backwards_and_wraps_only_under_repeat_all() {
        let mut playlist = playlist(&[1, 2, 3]);
        assert_eq!(playlist.rewind(), None);
        playlist.set_repeat(RepeatMode::All);
        assert_eq!(playlist.rewind(), Some(3));
        assert_eq!(playlist.rewind(), Some(2));
    }

    #[test]
    fn track_ended_replays_only_under_repeat_one() {
        let mut playlist = playlist(&[1, 2]);
        assert_eq!(playlist.on_track_ended(), PlaybackDecision::Play(2));
        playlist.set_repeat(RepeatMode::One);
        assert_eq!(playlist.on_track_ended(), PlaybackDecision::Replay(2));
    }

    #[test]
    fn track_ended_stops_at_the_end_of_the_queue() {
        let playlist = {
            let mut playlist = playlist(&[1]);
            playlist.set_repeat(RepeatMode::Off);
            playlist
        };
        assert_eq!(playlist.clone().on_track_ended(), PlaybackDecision::Stop);
    }

    #[test]
    fn track_ended_wraps_under_repeat_all() {
        let mut playlist = playlist(&[1, 2]);
        playlist.set_repeat(RepeatMode::All);
        playlist.advance();
        assert_eq!(playlist.on_track_ended(), PlaybackDecision::Play(1));
    }

    #[test]
    fn shuffle_keeps_the_current_track_first() {
        let mut playlist = playlist(&[1, 2, 3, 4, 5]);
        playlist.advance();
        playlist.advance();
        assert_eq!(playlist.current_id(), Some(3));

        playlist.set_shuffle(true);
        assert_eq!(playlist.current_id(), Some(3), "current track must keep playing");
        assert_eq!(playlist.order()[0], 3);
        assert_eq!(playlist.len(), 5);

        let mut sorted = playlist.order().to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![1, 2, 3, 4, 5], "shuffle must not drop tracks");
    }

    #[test]
    fn unshuffling_restores_the_original_order_and_selection() {
        let mut playlist = playlist(&[10, 20, 30, 40]);
        playlist.advance();
        playlist.set_shuffle(true);
        let playing = playlist.current_id().unwrap();

        playlist.set_shuffle(false);
        assert_eq!(playlist.order(), &[10, 20, 30, 40]);
        assert_eq!(playlist.current_id(), Some(playing));
    }

    #[test]
    fn shuffle_produces_a_different_order_in_practice() {
        // Not a hard guarantee for any single run, but over this many tracks a
        // working shuffle will not keep the input order.
        let ids: Vec<i64> = (1..=40).collect();
        let mut playlist = Playlist::default();
        playlist.load(ids.clone(), 0);
        playlist.set_shuffle(true);
        assert_ne!(playlist.order(), ids.as_slice());
    }

    #[test]
    fn shuffle_toggling_is_idempotent() {
        let mut playlist = playlist(&[1, 2, 3]);
        playlist.set_shuffle(true);
        let once = playlist.order().to_vec();
        playlist.set_shuffle(true);
        assert_eq!(playlist.order(), once.as_slice());
    }

    #[test]
    fn empty_playlist_never_advances() {
        let mut playlist = Playlist::default();
        assert_eq!(playlist.advance(), None);
        assert_eq!(playlist.rewind(), None);
        assert_eq!(playlist.current_id(), None);
        assert_eq!(playlist.on_track_ended(), PlaybackDecision::Stop);
    }

    #[test]
    fn single_item_queue_with_repeat_all_repeats() {
        let mut playlist = playlist(&[5]);
        playlist.set_repeat(RepeatMode::All);
        assert_eq!(playlist.advance(), Some(5));
        assert_eq!(playlist.on_track_ended(), PlaybackDecision::Play(5));
    }

    #[test]
    fn shuffle_preserves_every_id_exactly_once() {
        let ids: Vec<i64> = (0..200).collect();
        let mut playlist = Playlist::default();
        playlist.load(ids.clone(), 0);
        playlist.set_shuffle(true);

        let mut sorted = playlist.order().to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, ids);
    }

    #[test]
    fn has_next_and_previous_track_the_cursor() {
        let mut playlist = playlist(&[1, 2]);
        assert!(playlist.has_next());
        assert!(!playlist.has_previous());

        playlist.advance();
        assert!(!playlist.has_next(), "no wrap without repeat-all");
        assert!(playlist.has_previous());

        playlist.set_repeat(RepeatMode::All);
        assert!(playlist.has_next());
        assert!(playlist.has_previous());
    }

    #[test]
    fn index_reports_the_cursor() {
        let mut playlist = playlist(&[1, 2, 3]);
        assert_eq!(playlist.index(), Some(0));
        playlist.advance();
        assert_eq!(playlist.index(), Some(1));
    }
}

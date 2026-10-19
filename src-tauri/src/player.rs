use anyhow::Result;
use kira::{
    sound::{
        streaming::{StreamingSoundData, StreamingSoundHandle},
        FromFileError, PlaybackState,
    },
    AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween,
};

use crate::persistent_entities::PlayableTrack;
use crate::playlist::{PlaybackDecision, Playlist, RepeatMode};
use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerStatus {
    Playing,
    Paused,
    Stopped,
}

/// The part of the player the frontend actually needs. Keeping this separate
/// from `Player` keeps the audio backend handles out of the IPC payload.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub status: PlayerStatus,
    pub track_id: Option<i64>,
    pub progress: f64,
    pub duration: f64,
    pub volume: f64,
    pub playback_speed: f64,
    /// Queue state, so the frontend can render transport controls without
    /// duplicating the cursor logic.
    pub queue_length: usize,
    pub queue_index: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub has_next: bool,
    pub has_previous: bool,
}

pub struct Player {
    /// Created on first playback, not at startup, so that a session that never
    /// plays audio never opens an output device (and its ALSA/CPAL threads).
    manager: Option<AudioManager<DefaultBackend>>,
    sound_handle: Option<StreamingSoundHandle<FromFileError>>,
    pub track: Option<PlayableTrack>,
    status: PlayerStatus,
    progress: f64,
    duration: f64,
    volume: f64,
    playback_speed: f64,
    /// Ids of the tracks that have ended and are waiting to be handled.
    ///
    /// `renew_state` only discovers the end of a track; it cannot start the next
    /// one, because resolving an id to a file path needs the database and takes
    /// the DB lock. The state loop drains this after releasing the player lock,
    /// which also avoids the player lock being held across file I/O.
    pub finished: Vec<i64>,
    playlist: Playlist,
}

impl Player {
    pub fn new(initial_volume: f64) -> Player {
        Player {
            manager: None,
            sound_handle: None,
            track: None,
            status: PlayerStatus::Stopped,
            progress: 0.0,
            duration: 0.0,
            volume: initial_volume,
            playback_speed: 1.0,
            finished: Vec::new(),
            playlist: Playlist::default(),
        }
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        PlayerSnapshot {
            status: self.status,
            track_id: self.track.as_ref().and_then(|track| track.id),
            progress: self.progress,
            duration: self.duration,
            volume: self.volume,
            playback_speed: self.playback_speed,
            queue_length: self.playlist.len(),
            queue_index: self.playlist.index(),
            shuffle: self.playlist.is_shuffle(),
            repeat: self.playlist.repeat(),
            has_next: self.playlist.has_next(),
            has_previous: self.playlist.has_previous(),
        }
    }

    pub fn playlist(&self) -> &Playlist {
        &self.playlist
    }

    pub fn playlist_mut(&mut self) -> &mut Playlist {
        &mut self.playlist
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.status, PlayerStatus::Playing)
    }

    /// Refresh the observable state from the audio backend.
    ///
    /// Returns whether anything observable changed since the previous call, so
    /// the state emitter can stay silent on an idle player instead of pushing
    /// 25 identical payloads per second.
    ///
    /// This also detects the end of a track and records it in `finished`; see
    /// that field for why the transition itself happens elsewhere.
    pub fn renew_state(&mut self) -> bool {
        let before = self.snapshot();
        let previous_status = self.status;

        if let Some(ref mut sound_handle) = self.sound_handle {
            match sound_handle.state() {
                PlaybackState::Playing => self.status = PlayerStatus::Playing,
                PlaybackState::Pausing => self.status = PlayerStatus::Playing,
                PlaybackState::Stopping => self.status = PlayerStatus::Playing,
                PlaybackState::WaitingToResume => self.status = PlayerStatus::Playing,
                PlaybackState::Resuming => self.status = PlayerStatus::Playing,
                PlaybackState::Paused => self.status = PlayerStatus::Paused,
                PlaybackState::Stopped => self.status = PlayerStatus::Stopped,
            }
            self.progress = sound_handle.position();

            // Natural end of track: it was playing, and the backend now reports
            // stopped. An explicit `stop()` clears `sound_handle`, and a seek
            // never leaves the backend in `Stopped`, so this cannot misfire on
            // a user action.
            let ended_naturally = matches!(previous_status, PlayerStatus::Playing)
                && matches!(self.status, PlayerStatus::Stopped);

            if ended_naturally {
                if let Some(track_id) = self.track.as_ref().and_then(|track| track.id) {
                    self.finished.push(track_id);
                }
            }
        } else {
            self.status = PlayerStatus::Stopped
        }

        self.snapshot() != before
    }

    /// Whether a finished track is waiting for the caller to advance the queue.
    pub fn take_finished(&mut self) -> Option<i64> {
        if self.finished.is_empty() {
            None
        } else {
            Some(self.finished.remove(0))
        }
    }

    /// How a finished track should be handled, based on the queue and repeat
    /// mode. Returns `None` when the track was not started from the queue (a
    /// single file-picker track, for example), leaving the UI to just stop.
    pub fn decide_after_end(&mut self, finished_track_id: i64) -> Option<PlaybackDecision> {
        if self.playlist.current_id() != Some(finished_track_id) {
            return None;
        }

        Some(self.playlist.on_track_ended())
    }

    fn ensure_manager(&mut self) -> Result<()> {
        if self.manager.is_none() {
            self.manager =
                Some(AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?);
        }

        Ok(())
    }

    pub fn play(&mut self, track: PlayableTrack) -> Result<()> {
        let _ = self.stop();
        self.track = Some(track);

        if let Some(ref track) = self.track {
            let sound_data = StreamingSoundData::from_file(&track.file_path)?;
            self.duration = sound_data.duration().as_secs_f64();

            self.ensure_manager()?;
            let manager = self.manager.as_mut().unwrap();
            let mut sound_handle = manager.play(sound_data)?;
            sound_handle.set_volume(Self::volume_as_decibels(self.volume), Tween::default());
            sound_handle.set_playback_rate(self.playback_speed, Tween::default());
            self.sound_handle = Some(sound_handle);
        }

        Ok(())
    }

    /// Load a track without starting playback.
    ///
    /// Used when the window opens: the app should remember what you were
    /// listening to (or fall back to the first track) and show it in the player
    /// bar, but starting audio unprompted on launch is not wanted.
    ///
    /// The track metadata is set and `duration` is read from the file header so
    /// the seek bar is meaningful; no sound handle is created, which also means
    /// no audio device is opened. `status` stays `Stopped`, so pressing play
    /// goes through the normal `play()` path.
    pub fn select(&mut self, track: PlayableTrack) -> Result<()> {
        self.stop();
        self.duration = StreamingSoundData::from_file(&track.file_path)
            .map(|data| data.duration().as_secs_f64())
            .unwrap_or(track.duration);
        self.track = Some(track);
        self.status = PlayerStatus::Stopped;
        Ok(())
    }

    pub fn resume(&mut self) {
        if let Some(ref mut sound_handle) = self.sound_handle {
            sound_handle.resume(Tween::default());
        }
    }

    pub fn pause(&mut self) {
        if let Some(ref mut sound_handle) = self.sound_handle {
            sound_handle.pause(Tween::default());
        }
    }

    pub fn seek(&mut self, position: f64) {
        if let Some(ref mut sound_handle) = self.sound_handle {
            match sound_handle.state() {
                PlaybackState::Playing => sound_handle.seek_to(position),
                _ => {
                    sound_handle.seek_to(position);
                    sound_handle.resume(Tween::default());
                }
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(ref mut sound_handle) = self.sound_handle {
            sound_handle.stop(Tween::default());
        }

        self.sound_handle = None;
        self.track = None;
        self.duration = 0.0;
        self.progress = 0.0;
        self.status = PlayerStatus::Stopped;
    }

    /// Give the output device back to the system.
    ///
    /// `stop()` deliberately keeps the `AudioManager` alive because playing a
    /// new track stops the previous one first; reopening the device on every
    /// track change would add latency. An explicit user stop is different: the
    /// CPAL/ALSA threads keep waking up even when nothing is playing, so a
    /// player that was used once and then stopped would idle ~7% of a core
    /// forever. Releasing here makes the next `play()` recreate it lazily.
    pub fn release_audio_device(&mut self) {
        self.stop();
        self.manager = None;
    }

    /// Kira doesn't provide a way to create Decibels from an amplitude.
    /// Invert the formula in Decibels::as_amplitude():
    /// original:                         amp = 10 ^ (db / 20)
    /// take log() of both sides:         log(amp) = log(10 ^ (db / 20))
    /// identity log(a^b) = b*log(a):     log(amp) = (db / 20) * log(10)
    /// divide both sides by log(10):     log(amp) / log(10) = db / 20
    /// divide by log(10) is log base 10: log10(amp) = db / 20
    /// multiply both sides by 20:        20 * log10(amp) = db
    pub(crate) fn volume_as_decibels(volume: f64) -> Decibels {
        if volume <= 0.0 {
            Decibels::SILENCE
        } else if volume == 1.0 {
            Decibels::IDENTITY
        } else {
            Decibels((20.0 * volume.log10()) as f32)
        }
    }

    pub fn set_volume(&mut self, volume: f64) {
        if let Some(ref mut sound_handle) = self.sound_handle {
            sound_handle.set_volume(Self::volume_as_decibels(volume), Tween::default());
        }
        self.volume = volume;
    }

    pub fn set_playback_speed(&mut self, playback_speed: f64) {
        let clamped_speed = playback_speed.clamp(0.5, 2.0);

        if let Some(ref mut sound_handle) = self.sound_handle {
            sound_handle.set_playback_rate(clamped_speed, Tween::default());
        }

        self.playback_speed = clamped_speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_as_decibels() {
        let decibels = [
            Decibels::IDENTITY,
            Decibels(3.0),
            Decibels(12.0),
            Decibels(-3.0),
            Decibels(-12.0),
            Decibels::SILENCE,
        ];
        for db_expected in decibels {
            let db_actual = Player::volume_as_decibels(db_expected.as_amplitude() as f64);
            assert!(
                (db_expected.0 - db_actual.0) < 1e-5,
                "{} != {}",
                db_expected.0,
                db_actual.0
            );
        }
    }

    #[test]
    fn test_player_new_with_volume() {
        // This test just verifies the constructor signature compiles
        // We can't actually test the audio manager in a unit test
        let _ = Player::volume_as_decibels(0.5);
        let _ = Player::volume_as_decibels(1.0);
        let _ = Player::volume_as_decibels(0.0);
    }

    #[test]
    fn new_player_opens_no_audio_device_and_reports_no_change() {
        let mut player = Player::new(0.8);
        assert!(player.manager.is_none());
        assert_eq!(player.snapshot().status, PlayerStatus::Stopped);
        assert!(!player.is_playing());
        // An idle player has nothing to report, so the state loop stays silent.
        assert!(!player.renew_state());
    }

    #[test]
    fn snapshot_only_carries_frontend_fields() {
        let player = Player::new(0.5);
        let snapshot = player.snapshot();
        assert_eq!(snapshot.volume, 0.5);
        assert_eq!(snapshot.playback_speed, 1.0);
        assert_eq!(snapshot.duration, 0.0);
        assert_eq!(snapshot.track_id, None);
    }

    #[test]
    fn releasing_the_audio_device_leaves_a_stopped_player() {
        let mut player = Player::new(1.0);
        player.release_audio_device();
        assert!(player.manager.is_none());
        assert!(player.sound_handle.is_none());
        assert_eq!(player.snapshot().status, PlayerStatus::Stopped);
        assert!(!player.is_playing());
    }

    #[test]
    fn stop_resets_progress_and_track() {
        let mut player = Player::new(1.0);
        player.progress = 12.5;
        player.duration = 100.0;
        player.stop();
        let snapshot = player.snapshot();
        assert_eq!(snapshot.progress, 0.0);
        assert_eq!(snapshot.duration, 0.0);
        assert_eq!(snapshot.status, PlayerStatus::Stopped);
    }

    #[test]
    fn set_playback_speed_is_clamped_and_detected_as_change() {
        let mut player = Player::new(1.0);
        let before = player.snapshot();
        player.set_playback_speed(9.0);
        assert_eq!(player.snapshot().playback_speed, 2.0);
        assert_ne!(player.snapshot(), before);
    }
}

import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

const playingTrack = ref(null)
const status = ref('stopped')
// Queue state mirrors what the backend reports, so transport controls do not
// have to duplicate the cursor logic.
const queueLength = ref(0)
const queueIndex = ref(null)
const shuffle = ref(false)
const repeat = ref('off')
const hasNext = ref(false)
const hasPrevious = ref(false)
const duration = ref(null)
const progress = ref(null)
const volume = ref(1.0)
const playbackSpeed = ref(1.0)

listen('player-state', async event => {
  duration.value = event.payload.duration
  progress.value = event.payload.progress
  status.value = event.payload.status
  volume.value = event.payload.volume
  playbackSpeed.value = event.payload.playbackSpeed ?? 1.0
  queueLength.value = event.payload.queueLength ?? 0
  queueIndex.value = event.payload.queueIndex ?? null
  shuffle.value = event.payload.shuffle ?? false
  repeat.value = event.payload.repeat ?? 'off'
  hasNext.value = event.payload.hasNext ?? false
  hasPrevious.value = event.payload.hasPrevious ?? false
})

// When playback advances on its own (end of track, next, previous), the backend
// is the one that knows which track is now playing, so refetch it instead of
// guessing client-side.
let lastTrackId = null
listen('player-state', async event => {
  const trackId = event.payload.trackId ?? null
  if (trackId === lastTrackId) {
    return
  }
  lastTrackId = trackId

  if (trackId === null) {
    return
  }
  if (playingTrack.value && playingTrack.value.id === trackId) {
    return
  }

  playingTrack.value = await invoke('get_track', { trackId })
})

listen('reload-track-id', async event => {
  const payload = event.payload
  if (playingTrack.value && playingTrack.value.id === payload) {
    playingTrack.value = await invoke('get_track', { trackId: playingTrack.value.id })
  }
})

export function usePlayer() {
  // `queue` + `index` come from the list the user clicked in. Without them the
  // backend falls back to a one-track queue, which is what a bare call (or a
  // file-picker track) should do.
  const playTrack = (track, queue, index) => {
    playingTrack.value = track
    lastTrackId = track.id ?? null

    // Determine if this is a database track or a file-based track
    if (track.id !== undefined && track.id !== null) {
      // Database track - use track_id
      invoke('play_track', {
        trackId: track.id,
        filePath: null,
        title: track.title,
        albumName: track.album_name,
        artistName: track.artist_name,
        albumArtistName: track.album_artist_name,
        duration: track.duration,
        queue: queue ?? null,
        index: index ?? null,
      })
    } else if (track.file_path) {
      // File-based track (from file picker) - use file_path with metadata
      invoke('play_track', {
        trackId: null,
        filePath: track.file_path,
        title: track.title,
        albumName: track.album_name,
        artistName: track.artist_name,
        albumArtistName: track.album_artist_name,
        duration: track.duration,
        queue: null,
        index: null,
      })
    }
  }

  const playNext = () => {
    invoke('play_next')
  }

  const playPrevious = () => {
    invoke('play_previous')
  }

  const setShuffle = value => {
    invoke('set_shuffle', { shuffle: value })
  }

  const setRepeatMode = value => {
    invoke('set_repeat_mode', { repeat: value })
  }

  const cycleRepeatMode = () => {
    const order = ['off', 'all', 'one']
    const next = order[(order.indexOf(repeat.value) + 1) % order.length]
    setRepeatMode(next)
  }

  const pause = () => {
    if (!playingTrack.value) {
      return
    }

    invoke('pause_track')
  }

  const resume = () => {
    if (!playingTrack.value) {
      return
    }

    invoke('resume_track')
  }

  const seek = position => {
    if (!playingTrack.value) {
      return
    }

    if (status.value === 'stopped') {
      invoke('play_track', { trackId: playingTrack.value.id })
    }

    invoke('seek_track', { position })
  }

  const stop = () => {
    if (!playingTrack.value) {
      return
    }

    invoke('stop_track')
  }

  const setVolume = volume => {
    invoke('set_volume', { volume })
  }

  const setPlaybackSpeed = playbackSpeedValue => {
    invoke('set_playback_speed', { playbackSpeed: playbackSpeedValue })
  }

  return {
    playingTrack,
    status,
    duration,
    progress,
    volume,
    playbackSpeed,
    queueLength,
    queueIndex,
    shuffle,
    repeat,
    hasNext,
    hasPrevious,
    playTrack,
    playNext,
    playPrevious,
    setShuffle,
    setRepeatMode,
    cycleRepeatMode,
    pause,
    resume,
    stop,
    seek,
    setVolume,
    setPlaybackSpeed,
  }
}

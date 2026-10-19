import { computed, markRaw, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const delay = time => new Promise((resolve, reject) => setTimeout(resolve, time))

const downloadQueue = ref([])
const log = ref([])
const successCount = ref(0)
const failureCount = ref(0)
const downloadedCount = ref(0)
const isDownloading = ref(false)
const totalCount = ref(0)

const SUMMARY_KEY = 'sing-along.lastDownloadSummary'

const addLog = logObj => {
  log.value.unshift(markRaw(logObj))
  if (log.value.length > 100) {
    log.value.pop()
  }
}

// Persist a compact summary when a batch finishes, so the user can still see
// what happened after closing the app. Full logs stay in memory only — they are
// diagnostic, not archival data.
const saveSummary = (success, failure, total) => {
  try {
    localStorage.setItem(SUMMARY_KEY, JSON.stringify({
      success,
      failure,
      total,
      finishedAt: new Date().toISOString(),
    }))
  } catch {
    // localStorage full or unavailable; the summary is a nice-to-have
  }
}

const loadSummary = () => {
  try {
    const raw = localStorage.getItem(SUMMARY_KEY)
    return raw ? JSON.parse(raw) : null
  } catch {
    return null
  }
}

const downloadLyrics = async (track, autoExport, lyricScript, syncCover) => {
  try {
    const result = await invoke('download_lyrics', { trackId: track.id, autoExport, lyricScript, syncCover })

    // Cover sync: if enabled, search and embed cover art into the audio file.
    // Runs after lyrics so a lyrics failure doesn't leave a coverless file.
    if (syncCover && track.file_path) {
      try {
        await invoke('sync_cover', { trackPath: track.file_path, artist: track.artist_name, album: track.album_name })
      } catch (coverError) {
        console.warn('Cover sync failed for', track.title, ':', coverError)
      }
    }

    if (!isDownloading.value) {
      return
    }

    addLog({
      status: 'success',
      title: track.title,
      artistName: track.artist_name,
      message: result,
    })
    successCount.value++
  } catch (error) {
    if (!isDownloading.value) {
      return
    }

    addLog({ status: 'failure', title: track.title, artistName: track.artist_name, message: error })
    failureCount.value++
  }

  downloadedCount.value++
}

const downloadNext = async () => {
  while (true) {
    if (downloadQueue.value.length === 0) {
      await delay(1000)
      continue
    }

    const { trackId, autoExport, lyricScript, syncCover } = downloadQueue.value.shift()
    const track = await invoke('get_track', { trackId: trackId })
    await downloadLyrics(track, autoExport, lyricScript, syncCover)

    await delay(1)
  }
}

const downloadProgress = computed(() => {
  if (!isDownloading.value) {
    return 0.0
  }

  if (totalCount.value === 0) {
    return 0.0
  }

  if (downloadedCount.value >= totalCount.value) {
    return 1.0
  }

  return downloadedCount.value / totalCount.value
})

const addToQueue = (trackIds, autoExport = null, lyricScript = 'off', syncCover = false) => {
  const snapshot = autoExport ? Object.freeze({ ...autoExport }) : null
  isDownloading.value = true

  for (let i = 0; i < trackIds.length; i++) {
    downloadQueue.value.push({ trackId: trackIds[i], autoExport: snapshot, lyricScript, syncCover })
  }

  totalCount.value += trackIds.length

  console.log(`Added ${totalCount.value} tracks to download queue`)
}

const startOver = () => {
  downloadQueue.value = []
  log.value = []
  successCount.value = 0
  failureCount.value = 0
  downloadedCount.value = 0
  totalCount.value = 0
  isDownloading.value = false
}

const stopDownloading = () => {
  startOver()
}

export function useDownloader() {
  return {
    isDownloading,
    downloadQueue,
    downloadProgress,
    successCount,
    failureCount,
    totalCount,
    downloadedCount,
    log,
    addToQueue,
    startOver,
    loadSummary,
    stopDownloading,
    downloadNext,
  }
}

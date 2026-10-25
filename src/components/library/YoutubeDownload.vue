<template>
  <BaseModal
    :title="$t('youtube.title')"
    content-class="w-full h-[80vh] max-w-screen-sm"
    background="non-interactive"
    :lock-scroll="true"
    @close="emit('close')"
  >
    <div class="grow overflow-hidden flex flex-col gap-4">
      <!-- The search. The catalogue is searched, not a URL pasted: the point
           is to pick songs, and a pasted URL is a link to one of them. -->
      <form class="flex flex-col flex-none gap-2" @submit.prevent="search">
        <div class="flex gap-2">
          <input
            id="youtube-query"
            v-model="query"
            type="text"
            class="input w-full py-1.5 px-2"
            :placeholder="$t('youtube.searchPlaceholder')"
            :disabled="busy"
          />
          <button
            class="button button-primary rounded-full text-xs px-5 py-1.5"
            :class="{ 'button-disabled': busy }"
            :disabled="busy"
            type="submit"
          >
            {{ $t('youtube.search') }}
          </button>
        </div>
        <div class="text-xs text-neutral-500 dark:text-neutral-400">
          {{ $t('youtube.searchHint', { region: regionName }) }}
        </div>
      </form>

      <div class="grow overflow-hidden">
        <div v-if="busy" class="flex justify-center items-center h-full">
          <Loading class="animate-spin text-xl text-neutral-800 dark:text-neutral-400" />
        </div>

        <div
          v-else-if="problem"
          class="flex flex-col gap-3 justify-center items-center h-full text-sm"
        >
          <!-- The plain-language cause first, because the message from
               `yt-dlp` names the symptom rather than the fix, and the fix is
               almost always the cookies or the proxy. -->
          <div class="text-red-700 dark:text-red-400 text-center break-words">
            {{ problemHint }}
          </div>
          <div
            class="text-xs text-neutral-500 dark:text-neutral-400 text-center break-words max-w-md"
          >
            {{ problem }}
          </div>
          <button class="button button-normal px-4 py-1.5 text-xs rounded-full" @click="search">
            {{ $t('actions.retry') }}
          </button>
        </div>

        <div v-else class="flex flex-col h-full gap-3 overflow-auto">
          <!-- The songs. Each one is a row with a checkbox, so a user can
               pick as many as they want. -->
          <div class="flex flex-col gap-1">
            <label
              v-for="song in songs"
              :key="song.videoId"
              class="flex items-start gap-2 rounded px-2 py-1.5 hover:bg-neutral-100 dark:hover:bg-neutral-800 cursor-pointer"
            >
              <input
                v-model="selected"
                type="checkbox"
                :value="song.url"
                class="mt-1"
              />
              <span class="flex flex-col grow min-w-0">
                <span class="font-bold text-sm text-neutral-800 dark:text-neutral-200 truncate">
                  {{ song.title }}
                </span>
                <span class="text-xs text-neutral-600 dark:text-neutral-400 truncate">
                  {{ song.artist }}
                  <span v-if="song.album"> · {{ song.album }}</span>
                  <span v-if="song.duration"> · {{ humanDuration(song.duration) }}</span>
                </span>
              </span>
            </label>
          </div>

          <div v-if="!songs.length" class="flex justify-center items-center h-full text-sm text-neutral-400">
            {{ $t('youtube.noResults') }}
          </div>

          <!-- The options. -->
          <div class="flex flex-col gap-2">
            <label class="flex items-center gap-2 text-xs text-neutral-800 dark:text-neutral-200 cursor-pointer">
              <input v-model="showProgress" type="checkbox" class="mt-0.5" />
              <span>{{ $t('youtube.showProgress') }}</span>
            </label>
            <label class="flex items-center gap-2 text-xs text-neutral-800 dark:text-neutral-200 cursor-pointer">
              <input v-model="playNow" type="checkbox" class="mt-0.5" />
              <span>{{ $t('youtube.playAfterDownload') }}</span>
            </label>
          </div>

          <!-- The progress, when it is asked for. -->
          <div
            v-if="showProgress"
            class="rounded bg-white dark:bg-neutral-900 border border-neutral-200 dark:border-neutral-800 px-3 py-2 flex flex-col gap-1"
          >
            <div class="text-xs font-bold text-neutral-800 dark:text-neutral-200">
              {{ progressHeading }}
            </div>
            <div
              v-if="progress.percent !== null"
              class="h-1.5 w-full rounded bg-neutral-200 dark:bg-neutral-700 overflow-hidden"
            >
              <div
                class="h-full bg-hoa-1100 rounded transition-all"
                :style="{ width: `${Math.min(100, progress.percent || 0)}%` }"
              ></div>
            </div>
            <div class="text-xs text-neutral-600 dark:text-neutral-400 break-words font-mono">
              {{ progress.message }}
            </div>
          </div>

          <div class="flex justify-center gap-2">
            <button
              class="button rounded-full text-xs px-6 py-2"
              :class="{ 'button-disabled': downloading || !selected.length, 'button-primary': !downloading && selected.length }"
              :disabled="downloading || !selected.length"
              @click="download"
            >
              {{
                downloading
                  ? $t('youtube.downloading')
                  : $t('youtube.downloadSelected', { count: selected.length })
              }}
            </button>
          </div>

          <!-- What the download did. -->
          <div v-if="results.length" class="flex flex-col gap-1">
            <div
              v-for="result in results"
              :key="result.path || result.title"
              class="text-xs text-neutral-600 dark:text-neutral-400"
            >
              <span class="font-bold text-neutral-800 dark:text-neutral-200">
                {{ result.title || $t('youtube.unknownTitle') }}
              </span>
              <span v-if="result.path">
                — {{ result.album || $t('youtube.unknownAlbum') }}
                <span v-if="result.lyricsFound"> · {{ $t('youtube.hasLyrics') }}</span>
              </span>
              <span v-if="result.lyricsNote" class="block text-yellow-700 dark:text-yellow-400">
                {{ result.lyricsNote }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </BaseModal>
</template>

<script setup>
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import Loading from '~icons/mdi/loading'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import BaseModal from '@/components/common/BaseModal.vue'
import { classifyDownloadError } from '@/utils/youtube-errors.js'
import { humanDuration } from '@/utils/human-duration.js'
import i18n from '@/i18n/index.js'

// The cause names are the keys of the translations, so the first letter has
// to be a capital: 'cookies' becomes 'errorCookies'.
const capitalize = text => text.charAt(0).toUpperCase() + text.slice(1)

const emit = defineEmits(['close'])

const { t } = useI18n()
const toast = useToast()

const query = ref('')
const songs = ref([])
const selected = ref([])
const problem = ref('')
const busy = ref(false)
const downloading = ref(false)
const results = ref([])

// Whether the progress of the download is reported.
const showProgress = ref(false)
// Whether the track plays once it is downloaded.
const playNow = ref(false)

// The last progress the backend reported.
const progress = ref({ message: '', percent: null, speed: null, eta: null, total: null })

// The answer the user gave to the download prompt, if any.
const lyricScript = ref('ask')

// The name of the region the catalogue is searched in, so the user knows
// which songs they are being offered.
const regionName = computed(() => t(`youtube.regions.${i18n.global.locale.value}`))

// A heading for the progress, with the numbers a person wants.
const progressHeading = computed(() => {
  const parts = []
  if (progress.value.percent !== null) {
    parts.push(`${progress.value.percent.toFixed(1)}%`)
  }
  if (progress.value.speed) {
    parts.push(`${(progress.value.speed / 1024 / 1024).toFixed(2)} MiB/s`)
  }
  if (progress.value.eta) {
    parts.push(
      `ETA ${Math.floor(progress.value.eta / 60)}:${String(progress.value.eta % 60).padStart(2, '0')}`
    )
  }
  return parts.length ? parts.join(' · ') : t('youtube.progressWaiting')
})

/**
 * The plain-language cause of a failure.
 *
 * `yt-dlp` names the symptom, not the fix. The fix is almost always the
 * cookies or the proxy, and a user who is told "Sign in to confirm you're
 * not a bot" has no way to know that the file on disk is the problem.
 */
const problemHint = computed(() =>
  t(`youtube.error${capitalize(classifyDownloadError(problem.value))}`)
)

const search = async () => {
  busy.value = true
  problem.value = ''
  try {
    songs.value = await invoke('search_youtube_music', {
      query: query.value.trim(),
      locale: i18n.global.locale.value,
    })
  } catch (error) {
    console.error(error)
    problem.value = String(error)
    songs.value = []
  } finally {
    busy.value = false
  }
}

const download = async () => {
  downloading.value = true
  results.value = []
  try {
    results.value = await invoke('download_youtube_track', {
      urls: selected.value,
      duplicateChoice: 'replace',
      playNow: playNow.value,
      showProgress: showProgress.value,
      lyricScript: lyricScript.value,
      locale: i18n.global.locale.value,
    })

    const done = results.value.filter(result => result.path).length
    const withLyrics = results.value.filter(result => result.lyricsFound).length

    if (done) {
      toast.success(
        t('youtube.downloadedCount', {
          count: done,
          lyrics: withLyrics,
        })
      )
    }
  } catch (error) {
    console.error(error)
    toast.error(String(error))
  } finally {
    downloading.value = false
  }
}

let unlistenProgress = null

onMounted(async () => {
  const config = await invoke('get_config')
  lyricScript.value = config.lyric_script ?? 'ask'

  // The progress arrives as an event. The listener is removed when the
  // dialog closes, so that a closed dialog does not keep updating.
  unlistenProgress = await listen('youtube-download-progress', event => {
    progress.value = event.payload
  })

  // The songs the region is listening to are offered first: a user who opens
  // the dialog without a query wants to see what is popular, not an empty
  // list.
  search()
})

onUnmounted(() => {
  unlistenProgress?.()
})
</script>

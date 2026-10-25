<template>
  <BaseModal
    :title="$t('youtube.title')"
    content-class="w-full h-[80vh] max-w-screen-sm"
    background="non-interactive"
    :lock-scroll="true"
    @close="emit('close')"
  >
    <div class="grow overflow-hidden flex flex-col gap-6">
      <form class="flex flex-col flex-none gap-4" @submit.prevent="probe">
        <div>
          <label for="youtube-url" class="group-label mb-1">{{ $t('youtube.url') }}</label>
          <input
            id="youtube-url"
            v-model="url"
            type="text"
            class="input w-full py-1.5 px-2"
            :placeholder="$t('youtube.urlPlaceholder')"
            :disabled="busy"
          />
        </div>

        <div class="flex justify-center gap-2">
          <button
            class="button rounded-full text-xs px-6 py-2"
            :class="{ 'button-disabled': busy || !url, 'button-primary': !busy && url }"
            :disabled="busy || !url"
            type="submit"
          >
            {{ $t('youtube.lookUp') }}
          </button>
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
          <button class="button button-normal px-4 py-1.5 text-xs rounded-full" @click="probe">
            {{ $t('actions.retry') }}
          </button>
        </div>

        <div v-else-if="info" class="flex flex-col gap-4 h-full overflow-auto">
          <div
            class="rounded bg-white dark:bg-neutral-900 border border-neutral-200 dark:border-neutral-800 px-3 py-2"
          >
            <div class="font-bold text-neutral-800 dark:text-neutral-200">{{ info.title }}</div>
            <div class="text-sm text-neutral-900 dark:text-neutral-400">
              <span>{{ info.artist || $t('youtube.unknownArtist') }}</span>
            </div>
            <div class="text-sm text-neutral-900 dark:text-neutral-400">
              <span>{{ $t('youtube.album') }}: </span>
              <span>{{ info.correctedAlbum || info.album || $t('youtube.unknownAlbum') }}</span>
            </div>
            <div
              v-if="info.correctedAlbum && info.correctedAlbum !== info.album"
              class="text-xs text-neutral-500 dark:text-neutral-400"
            >
              {{ $t('youtube.albumCorrected') }}
            </div>
          </div>

          <!-- The library already has this recording. What to do about it is
               the user's decision: the file on disk may be a better copy, or
               the library entry may be a rip they would rather keep. -->
          <div
            v-if="info.existingTrack"
            class="rounded bg-yellow-50 dark:bg-neutral-900 border border-yellow-300 dark:border-neutral-800 px-3 py-2 flex flex-col gap-2"
          >
            <div class="text-sm font-bold text-neutral-800 dark:text-neutral-200">
              {{ $t('youtube.duplicateTitle') }}
            </div>
            <div class="text-xs text-neutral-700 dark:text-neutral-400 break-words">
              {{ $t('youtube.duplicateBody') }}
            </div>
            <div
              class="text-xs text-neutral-600 dark:text-neutral-400 break-words"
            >
              {{ $t('youtube.duplicateExisting') }}:
              <span class="font-bold">{{ info.existingTrack.title }}</span>
              <span> — {{ info.existingTrack.artist }}</span>
              <span v-if="info.existingTrack.hasLyrics"> · {{ $t('youtube.hasLyrics') }}</span>
            </div>

            <div class="flex flex-col gap-1 mt-1">
              <label
                v-for="choice in duplicateChoices"
                :key="choice.value"
                class="flex items-start gap-2 text-xs text-neutral-800 dark:text-neutral-200 cursor-pointer"
              >
                <input
                  v-model="duplicateChoice"
                  type="radio"
                  name="youtube-duplicate"
                  :value="choice.value"
                  class="mt-0.5"
                />
                <span>
                  <span class="font-bold">{{ choice.label }}</span>
                  <span class="block text-neutral-600 dark:text-neutral-400">{{
                    choice.hint
                  }}</span>
                </span>
              </label>
            </div>
          </div>

          <div class="text-xs text-neutral-500 dark:text-neutral-400">
            {{ $t('youtube.downloadExplanation') }}
          </div>

          <!-- The options. The progress is opt-in: a download of a large file
               takes minutes, and a window that says nothing for three
               minutes looks broken — but the report is a lot of text, and a
               user who does not need it would rather not read it. -->
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
            <div
              class="text-xs text-neutral-600 dark:text-neutral-400 break-words font-mono"
            >
              {{ progress.message }}
            </div>
          </div>

          <div class="flex justify-center">
            <button
              class="button rounded-full text-xs px-6 py-2"
              :class="{ 'button-disabled': downloading, 'button-primary': !downloading }"
              :disabled="downloading"
              @click="download"
            >
              {{ downloading ? $t('youtube.downloading') : $t('youtube.download') }}
            </button>
          </div>
        </div>

        <div v-else class="flex justify-center items-center h-full text-sm text-neutral-400">
          {{ $t('youtube.hint') }}
        </div>
      </div>
    </div>
  </BaseModal>
</template>

<script setup>
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import Loading from '~icons/mdi/loading'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import BaseModal from '@/components/common/BaseModal.vue'
import { classifyDownloadError } from '@/utils/youtube-errors.js'
import i18n from '@/i18n/index.js'

// The cause names are the keys of the translations, so the first letter has
// to be a capital: 'cookies' becomes 'errorCookies'.
const capitalize = text => text.charAt(0).toUpperCase() + text.slice(1)

const emit = defineEmits(['close'])

const { t } = useI18n()
const toast = useToast()

const url = ref('')
const info = ref(null)
const problem = ref('')
const busy = ref(false)
const downloading = ref(false)
const duplicateChoice = ref('replace')

// Whether the progress of the download is reported.
const showProgress = ref(false)
// Whether the track plays once it is downloaded.
const playNow = ref(false)

// The last progress the backend reported.
const progress = ref({ message: '', percent: null, speed: null, eta: null, total: null })

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
    parts.push(`ETA ${Math.floor(progress.value.eta / 60)}:${String(progress.value.eta % 60).padStart(2, '0')}`)
  }
  return parts.length ? parts.join(' · ') : t('youtube.progressWaiting')
})

// The answer the user gave to the download prompt, if any.
const lyricScript = ref('ask')

// What can be done about a track the library already holds. The order is the
// order a person would think in: the common case first, the way out last.
const duplicateChoices = computed(() => [
  {
    value: 'replace',
    label: t('youtube.duplicateReplace'),
    hint: t('youtube.duplicateReplaceHint'),
  },
  {
    value: 'keepBoth',
    label: t('youtube.duplicateKeepBoth'),
    hint: t('youtube.duplicateKeepBothHint'),
  },
  {
    value: 'cancel',
    label: t('youtube.duplicateCancel'),
    hint: t('youtube.duplicateCancelHint'),
  },
])

// A new track means a new decision: the previous answer was about a
// different song.
watch(info, () => {
  duplicateChoice.value = 'replace'
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

/**
 * The Chinese script the metadata should end up in.
 *
 * The answer the user gave to the download prompt wins, and the interface
 * language is the fallback: a Traditional-Chinese reader wants the metadata
 * in Traditional, and a Simplified-Chinese reader wants it in Simplified.
 * YouTube publishes in whatever script the uploader chose, so without this
 * a reader would get a file whose title they cannot type back into a search.
 */
let unlistenProgress = null

onMounted(async () => {
  const config = await invoke('get_config')
  lyricScript.value = config.lyric_script ?? 'ask'

  // The progress arrives as an event. The listener is removed when the
  // dialog closes, so that a closed dialog does not keep updating.
  unlistenProgress = await listen('youtube-download-progress', event => {
    progress.value = event.payload
  })
})

onUnmounted(() => {
  unlistenProgress?.()
})

const probe = async () => {
  busy.value = true
  problem.value = ''
  info.value = null
  try {
    info.value = await invoke('probe_youtube_track', {
      url: url.value.trim(),
      lyricScript: lyricScript.value,
      locale: i18n.global.locale.value,
    })
  } catch (error) {
    console.error(error)
    problem.value = String(error)
  } finally {
    busy.value = false
  }
}

const download = async () => {
  downloading.value = true
  try {
    const result = await invoke('download_youtube_track', {
      url: url.value.trim(),
      duplicateChoice: duplicateChoice.value,
      playNow: playNow.value,
      showProgress: showProgress.value,
      lyricScript: lyricScript.value,
      locale: i18n.global.locale.value,
    })
    // The lyrics are part of the outcome, so they are part of the message: a
    // download that succeeded but produced a song without lyrics is a
    // different thing from one that produced a complete song, and the user
    // is the one who has to know which they got.
    if (result.lyricsFound) {
      toast.success(
        t('youtube.downloaded', {
          title: result.title,
          album: result.album || t('youtube.unknownAlbum'),
        })
      )
    } else {
      toast.info(
        t('youtube.downloadedWithoutLyrics', {
          title: result.title,
          note: result.lyricsNote || '',
        })
      )
    }
    emit('close')
  } catch (error) {
    console.error(error)
    toast.error(String(error))
  } finally {
    downloading.value = false
  }
}
</script>

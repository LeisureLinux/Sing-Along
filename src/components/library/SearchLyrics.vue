<template>
  <BaseModal
    :title="$t('lyricsSearch.title')"
    content-class="w-full h-[80vh] max-w-screen-sm"
    background="non-interactive"
    :lock-scroll="true"
    @close="emit('close')"
  >
    <div class="grow overflow-hidden flex flex-col gap-6">
      <form class="flex flex-col flex-none gap-4" @submit.prevent="doSearchLyrics">
        <!-- Album is deliberately absent: it never helps the search and, when
             it is a folder name, actively prevents it. -->
        <div class="grid grid-cols-2 gap-2">
          <div>
            <label for="title" class="group-label mb-1">{{ $t('search.title') }}</label>
            <input
              id="title"
              v-model="title"
              type="text"
              class="input w-full py-1.5 px-2"
              :placeholder="$t('search.title')"
              :disabled="loading"
            />
          </div>

          <div>
            <label for="artistName" class="group-label mb-1">{{ $t('search.artist') }}</label>
            <input
              id="artistName"
              v-model="artistName"
              type="text"
              class="input w-full py-1.5 px-2"
              :placeholder="$t('search.artist')"
              :disabled="loading"
            />
          </div>
        </div>

        <div class="col-span-2 flex justify-center">
          <button
            class="button rounded-full text-xs px-6 py-2"
            :class="{ 'button-disabled': loading, 'button-primary': !loading }"
            :disabled="loading"
          >
            {{ $t('actions.search') }}
          </button>
        </div>
      </form>

      <div class="grow overflow-hidden">
        <div v-if="loading" class="flex justify-center items-center h-full">
          <Loading class="animate-spin text-xl text-neutral-800 dark:text-neutral-400" />
        </div>

        <div v-else class="flex flex-col h-full gap-2 overflow-auto">
          <div v-if="normalizedSearchResult.length" class="flex flex-col gap-1 overflow-auto">
            <div
              v-for="result in normalizedSearchResult"
              :key="result.item.id"
              class="rounded bg-white dark:bg-neutral-900 hover:bg-neutral-50 dark:hover:bg-neutral-800 border border-neutral-200 dark:border-neutral-800 hover:border-neutral-300 dark:hover:border-neutral-700 transition px-2 py-1 flex gap-2"
            >
              <div class="h-full overflow-hidden grow">
                <div class="font-bold flex gap-1">
                  <span class="mr-1 text-sm text-neutral-800 dark:text-neutral-200">{{
                    result.item.name
                  }}</span>
                  <template v-if="showLineCount === true">
                    <span
                      v-if="result.lyrics.syncedLyrics"
                      class="text-neutral-800 dark:text-neutral-300 font-bold text-[0.65rem] bg-neutral-200 dark:bg-neutral-700 rounded px-1 py-0.5"
                      >{{ $t('lyricsSearch.lines', { count: countLines(result.lyrics.syncedLyrics) }) }}</span
                    >
                    <span
                      v-else-if="result.lyrics.plainLyrics"
                      class="text-neutral-800 dark:text-neutral-300 font-bold text-[0.65rem] bg-neutral-200 dark:bg-neutral-700 rounded px-1 py-0.5"
                      >{{ $t('lyricsSearch.lines', { count: countLines(result.lyrics.plainLyrics) }) }}</span
                    >
                  </template>
                  <span
                    v-if="result.lyrics.syncedLyrics"
                    class="text-white font-bold text-[0.65rem] bg-hoa-1100 rounded px-1 py-0.5"
                    >{{ $t('track.synced') }}</span
                  >
                  <span
                    v-else-if="result.lyrics.plainLyrics"
                    class="text-neutral-800 dark:text-neutral-300 font-bold text-[0.65rem] bg-neutral-200 dark:bg-neutral-700 rounded px-1 py-0.5"
                    >{{ $t('track.plain') }}</span
                  >
                  <span
                    v-else-if="result.lyrics.instrumental"
                    class="text-neutral-900 dark:text-neutral-200 font-bold text-[0.65rem] bg-neutral-300 dark:bg-neutral-600 rounded px-1 py-0.5"
                    >{{ $t('track.instrumental') }}</span
                  >
                  <span
                    v-if="
                      Math.round(result.item.duration) - Math.round(searchingTrack.duration) > 2
                    "
                    class="text-neutral-500 dark:text-neutral-400 text-[0.75rem]"
                  >
                    +{{
                      humanDuration(
                        Math.abs(result.item.duration - Math.round(searchingTrack.duration))
                      )
                    }}
                  </span>
                  <span
                    v-else-if="
                      Math.round(result.item.duration) - Math.round(searchingTrack.duration) < -2
                    "
                    class="text-neutral-500 dark:text-neutral-400 text-[0.75rem]"
                  >
                    -{{
                      humanDuration(
                        Math.abs(result.item.duration - Math.round(searchingTrack.duration))
                      )
                    }}
                  </span>
                </div>
                <div class="text-sm text-neutral-900 dark:text-neutral-400 truncate">
                  <span>{{ result.item.albumName }}</span> |
                  <span>{{ result.item.artistName }}</span>
                </div>
              </div>

              <div class="flex gap-2 items-center">
                <button
                  class="button-tiny"
                  :title="$t('lyricsSearch.preview')"
                  @click="preview(result.item)"
                >
                  <Eye />
                </button>
                <button class="button-tiny" :title="$t('lyricsSearch.apply')" @click="apply(result.item)">
                  <ContentSave />
                </button>
              </div>
            </div>
          </div>

          <div
            v-else-if="searchError"
            class="flex flex-col gap-2 justify-center items-center h-full text-sm"
          >
            <div class="text-red-700 dark:text-red-400 text-center break-words">
              {{ $t('lyricsSearch.errorSearch') }}
            </div>
            <div class="text-xs text-neutral-500 dark:text-neutral-400 break-words text-center">
              {{ searchError }}
            </div>
            <button class="button button-normal px-4 py-1.5 text-xs rounded-full" @click="doSearchLyrics">
              {{ $t('actions.retry') }}
            </button>
          </div>

          <div v-else class="flex justify-center items-center h-full text-sm text-neutral-400">
            {{ $t('lyricsSearch.noResults') }}
          </div>
        </div>
      </div>
    </div>
  </BaseModal>
</template>

<script setup>
import { invoke } from '@tauri-apps/api/core'
import { computed, ref, onMounted } from 'vue'
import Loading from '~icons/mdi/loading'
import Eye from '~icons/mdi/eye'
import ContentSave from '~icons/mdi/content-save'
import { useToast } from 'vue-toastification'
import Preview from './search-lyrics/Preview.vue'
import { useModal } from 'vue-final-modal'
import { countLines } from '@/utils/count-lines.js'
import { normalizeLrclibLyrics } from '@/utils/lyricsfile.js'
import { convertLyricScript, scriptForLocale } from '@/utils/script-conversion.js'
import { useI18n } from 'vue-i18n'

const toast = useToast()
const { t, i18n } = useI18n()
const props = defineProps(['searchingTrack'])
const emit = defineEmits(['close'])

const loading = ref(true)
const searchResult = ref(null)
const searchError = ref('')
const previewingLyrics = ref(null)
const previewingTrack = ref(null)

const title = ref('')
const artistName = ref('')
const showLineCount = ref(true)

const { open: openPreviewModal, close: closePreviewModal } = useModal({
  component: Preview,
  attrs: {
    track: previewingTrack,
    lyrics: previewingLyrics,
    onClose() {
      closePreviewModal()
    },
    onClosed() {
      previewingTrack.value = null
      previewingLyrics.value = null
    },
  },
})

const normalizedSearchResult = computed(() => {
  if (!Array.isArray(searchResult.value)) {
    return []
  }

  return searchResult.value.map(item => ({
    item,
    lyrics: normalizeLrclibLyrics(item),
  }))
})

const humanDuration = seconds => {
  return new Date(seconds * 1000).toISOString().slice(14, 19)
}

/**
 * Search LRCLIB by free text.
 *
 * `album_name` is never sent. In a personal library it is whatever the folder or
 * tag happened to be — "KXT", "Unknown Album" — and LRCLIB ANDs the fields, so
 * including it turns a search that would return twenty hits into zero. Only the
 * title and artist are meaningful, and they go through `q` because that is the
 * free-text search the Search tab uses and the one that actually matches.
 */
const doSearchLyrics = async () => {
  loading.value = true
  searchError.value = ''
  const query = [title.value, artistName.value]
    .map(v => (v || '').trim())
    .filter(Boolean)
    .join(' ')

  try {
    searchResult.value = query
      ? await invoke('search_lyrics', {
          title: '',
          albumName: '',
          artistName: '',
          q: query,
        })
      : []
  } catch (error) {
    // A transport failure is not the same as "no match": saying the latter when
    // LRCLIB is unreachable sends the user looking for a different search term.
    console.error(error)
    searchResult.value = null
    searchError.value = String(error)
  } finally {
    loading.value = false
  }
}

const preview = lyricsItem => {
  previewingTrack.value = props.searchingTrack
  previewingLyrics.value = lyricsItem
  openPreviewModal()
}

const apply = async lyricsItem => {
  try {
    // Pick up the configured script before saving. A manual search is the same
    // user intent as a download, so a track set to Simplified should not end up
    // with the Traditional text LRCLIB happens to serve.
    const config = await invoke('get_config')
    const target =
      config.lyric_script === 'ask' || !config.lyric_script
        ? scriptForLocale(i18n.global.locale.value)
        : config.lyric_script

    const item =
      target === 'off' || target === 'ask'
        ? lyricsItem
        : {
            ...lyricsItem,
            plainLyrics: convertLyricScript(lyricsItem.plainLyrics, target),
            syncedLyrics: convertLyricScript(lyricsItem.syncedLyrics, target),
            lyricsfile: convertLyricScript(lyricsItem.lyricsfile, target),
          }

    const result = await invoke('apply_lyrics', {
      trackId: props.searchingTrack.id,
      lrclibResponse: item,
    })

    // The backend emits `reload-track-id` synchronously before returning, so the
    // row and the player bar have already picked up the new lyrics by now.
    // Closing the dialog is what makes that visible: it was covering the list
    // the whole time, which read as "nothing happened".
    toast.success(result)
    emit('close')
  } catch (error) {
    console.error(error)
    toast.error(String(error))
  }
}

const initialize = async () => {
  if (!props.searchingTrack) {
    return
  }

  const config = await invoke('get_config')
  searchResult.value = null
  loading.value = true

  title.value = props.searchingTrack.title
  artistName.value = props.searchingTrack.artist_name
  showLineCount.value = config.show_line_count
}

onMounted(async () => {
  await initialize()
  doSearchLyrics()
})
</script>

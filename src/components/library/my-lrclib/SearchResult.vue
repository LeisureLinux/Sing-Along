<template>
  <div class="secondary-page">
    <div>
      <div class="mb-4">
        <button class="secondary-page-back-button" @click="$emit('back')">
          <ArrowLeft />
        </button>
      </div>
    </div>

    <div v-if="loading" class="flex justify-center items-center h-full">
      <Loading class="animate-spin" />
    </div>

    <div v-else class="mx-auto max-w-screen-sm">
      <div class="flex flex-col mb-8">
        <div class="text-thin text-xl">
          {{ $t('lyricsSearch.searchingFor') }}
          <span class="font-bold">{{ keyword }}</span>
        </div>
        <div class="flex items-center gap-2">
          <div
            class="text-sm text-neutral-800 dark:text-neutral-500 group-hover:text-neutral-800 transition"
          >
            {{ $t('lyricsSearch.foundTracks', { count: hits.length }) }}
          </div>
        </div>

        <!-- A source can fail without failing the search; say which one, so an
             empty list is not mistaken for "no lyrics exist". -->
        <div v-if="errors.length" class="mt-2 flex flex-col gap-1">
          <div
            v-for="message in errors"
            :key="message"
            class="text-xs text-red-700 dark:text-red-400 break-words"
          >
            {{ message }}
          </div>
        </div>
      </div>

      <div class="flex flex-col gap-2">
        <div
          v-for="result in normalizedTracks"
          :key="`${result.hit.source}:${result.hit.id}`"
          class="rounded bg-white hover:bg-neutral-50 dark:bg-neutral-900 dark:hover:bg-neutral-800 transition px-2 py-1 flex gap-2 justify-between items-center"
        >
          <div class="flex flex-col gap-1">
            <div class="flex gap-2 items-center">
              <div class="text-sm font-bold text-neutral-800 dark:text-neutral-200">
                {{ result.hit.name }}
              </div>
              <div class="text-[0.65rem] font-bold flex gap-1">
                <span
                  class="bg-neutral-100 text-neutral-800 dark:bg-neutral-800 dark:text-neutral-200 px-1 py-0.5 rounded"
                  >{{ humanDuration(result.hit.duration) }}</span
                >
                <!-- Which service the row came from. Both are searched by
                     default, so without this the list is ambiguous. -->
                <span
                  class="px-1 py-0.5 rounded"
                  :class="
                    result.hit.source === 'lrclib'
                      ? 'bg-blue-800 text-blue-200'
                      : 'bg-amber-700 text-amber-100'
                  "
                  >{{ result.hit.source === 'lrclib' ? 'LRCLIB' : 'NetEase' }}</span
                >
                <template v-if="showLineCount === true">
                  <span
                    v-if="!!result.lyrics.syncedLyrics"
                    class="bg-blue-800 text-blue-200 px-1 py-0.5 rounded"
                    >{{ $t('lyricsSearch.lines', { count: countLines(result.lyrics.syncedLyrics) }) }}</span
                  >
                  <span
                    v-else-if="!!result.lyrics.plainLyrics"
                    class="bg-blue-800 text-blue-200 px-1 py-0.5 rounded"
                    >{{ $t('lyricsSearch.lines', { count: countLines(result.lyrics.plainLyrics) }) }}</span
                  >
                </template>
                <span
                  v-if="!!result.lyrics.syncedLyrics"
                  class="bg-green-800 text-green-200 px-1 py-0.5 rounded"
                  >{{ $t('track.synced') }}</span
                >
                <span
                  v-else-if="!!result.lyrics.plainLyrics"
                  class="bg-gray-800 text-gray-200 px-1 py-0.5 rounded"
                  >{{ $t('track.plain') }}</span
                >
                <span
                  v-else-if="!!result.lyrics.instrumental"
                  class="bg-gray-300 text-gray-600 dark:bg-neutral-700 dark:text-neutral-300 px-1 py-0.5 rounded"
                  >{{ $t('track.instrumental') }}</span
                >
              </div>
            </div>
            <div class="text-sm text-neutral-900 dark:text-neutral-500">
              {{ result.hit.albumName }} - {{ result.hit.artistName }}
            </div>
          </div>
          <div class="flex gap-1 items-center">
            <button
              class="button-tiny"
              type="button"
              :title="$t('lyricsSearch.preview')"
              @click="setShowingTrack(result.hit)"
            >
              <Eye />
            </button>

            <button
              class="button-tiny"
              type="button"
              :title="$t('lyricsSearch.edit')"
              @click="setEditingTrack(result.hit)"
            >
              <PlaylistEdit />
            </button>
          </div>
        </div>
      </div>
    </div>

    <div
      v-if="isOpeningTrack"
      class="flex items-center justify-center w-full h-full fixed top-0 left-0 bg-white/50 dark:bg-neutral-900/50 transition"
    >
      <Loading class="animate-spin" />
    </div>
  </div>
</template>

<script setup>
import ArrowLeft from '~icons/mdi/arrow-left'
import Loading from '~icons/mdi/loading'
import Eye from '~icons/mdi/eye'
import PlaylistEdit from '~icons/mdi/playlist-edit'
import { computed, ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { countLines } from '@/utils/count-lines.js'
import { humanDuration } from '@/utils/human-duration.js'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import { normalizeLrclibLyrics } from '@/utils/lyricsfile.js'
import PreviewLyrics from './PreviewLyrics.vue'
import AssociateTrackModal from './AssociateTrackModal.vue'
import LyricsfileConflictModal from './LyricsfileConflictModal.vue'
import EditLyricsV2 from '../EditLyricsV2.vue'
import { useModal } from 'vue-final-modal'

const toast = useToast()
const { t } = useI18n()

const props = defineProps({
  keyword: {
    type: String,
    required: true,
  },
  useLrclib: {
    type: Boolean,
    default: true,
  },
  useNetease: {
    type: Boolean,
    default: true,
  },
})

const emit = defineEmits(['back'])

const hits = ref([])
const errors = ref([])
const loading = ref(false)
const isOpeningTrack = ref(false)
const showingTrack = ref(null)
const editingAudioSource = ref(null)
const editingLyricsfile = ref(null)
const editingTrackId = ref(null)
const showLineCount = ref(true)

const normalizedTracks = computed(() =>
  hits.value.map(hit => ({
    hit,
    lyrics: normalizeLrclibLyrics(hit),
  }))
)

const { open: openPreviewModal, close: closePreviewModal } = useModal({
  component: PreviewLyrics,
  attrs: {
    track: showingTrack,
    onClose() {
      closePreviewModal()
    },
    onClosed() {
      showingTrack.value = null
    },
  },
})

const { open: openEditLyricsV2Modal, close: closeEditLyricsV2Modal } = useModal({
  component: EditLyricsV2,
  attrs: {
    audioSource: editingAudioSource,
    lyricsfile: editingLyricsfile,
    trackId: editingTrackId,
    onClose() {
      closeEditLyricsV2Modal()
    },
    onClosed() {
      editingAudioSource.value = null
      editingLyricsfile.value = null
      editingTrackId.value = null
    },
  },
})

const associateModalHit = ref(null)
const associateModalLyricsfileId = ref(null)
const associateModalInitialLyricsfile = ref(null)

const { open: openAssociateTrackModal, close: closeAssociateTrackModal } = useModal({
  component: AssociateTrackModal,
  attrs: {
    lrclibTrack: associateModalHit,
    onClose() {
      closeAssociateTrackModal()
    },
    onClosed() {
      associateModalHit.value = null
      associateModalLyricsfileId.value = null
      associateModalInitialLyricsfile.value = null
    },
    onSelectTrack(track) {
      openEditLyricsV2WithTrack(track)
    },
    onSelectFile(fileMetadata) {
      openEditLyricsV2WithFile(fileMetadata)
    },
  },
})

// Conflict modal state
const conflictModalHit = ref(null)
const conflictModalId = ref(null)
const conflictModalExistingResult = ref(null)

const { open: openConflictModal, close: closeConflictModal } = useModal({
  component: LyricsfileConflictModal,
  attrs: {
    lrclibId: conflictModalId,
    existingResult: conflictModalExistingResult,
    onClose() {
      closeConflictModal()
    },
    onClosed() {
      conflictModalHit.value = null
      conflictModalId.value = null
      conflictModalExistingResult.value = null
    },
    onRedownload(result) {
      proceedToAssociateTrackModal(conflictModalHit.value, result)
    },
    onContinue(result) {
      proceedToAssociateTrackModal(conflictModalHit.value, result)
    },
  },
})

onMounted(async () => {
  const config = await invoke('get_config')
  showLineCount.value = config.show_line_count
  loading.value = true
  try {
    const outcome = await invoke('search_lyrics_multi', {
      keyword: props.keyword,
      useLrclib: props.useLrclib,
      useNetease: props.useNetease,
    })
    hits.value = outcome.hits
    errors.value = outcome.errors ?? []

    if (!hits.value.length && errors.value.length) {
      // Every selected source failed. The per-source lines above carry the
      // technical detail; this is the one-line summary a user can act on.
      toast.error(t('lyricsSearch.errorSearch'))
    }
  } catch (error) {
    toast.error(t('lyricsSearch.errorSearch'))
    console.error(error)
  } finally {
    loading.value = false
  }
})

const setShowingTrack = async hit => {
  isOpeningTrack.value = true
  try {
    // Both sources are normalised through the same command, so preview does not
    // need to know which one the row came from.
    const prepared = await invoke('prepare_search_hit_lyricsfile', {
      source: hit.source,
      id: hit.id,
      name: hit.name,
      artistName: hit.artistName,
      albumName: hit.albumName,
      duration: hit.duration,
    })
    showingTrack.value = {
      ...hit,
      plainLyrics: prepared.plainLyrics,
      syncedLyrics: prepared.syncedLyrics,
      instrumental: prepared.isInstrumental,
      lyricsfile: prepared.lyricsfile,
    }
    openPreviewModal()
  } catch (error) {
    toast.error(t('lyricsSearch.errorOpen'))
    console.error(error)
  } finally {
    isOpeningTrack.value = false
  }
}

const proceedToAssociateTrackModal = (hit, result) => {
  associateModalHit.value = hit
  associateModalLyricsfileId.value = result.lyricsfileId
  associateModalInitialLyricsfile.value = result.lyricsfile
  openAssociateTrackModal()
}

const setEditingTrack = async hit => {
  isOpeningTrack.value = true
  try {
    const result = await invoke('prepare_search_hit_lyricsfile', {
      source: hit.source,
      id: hit.id,
      name: hit.name,
      artistName: hit.artistName,
      albumName: hit.albumName,
      duration: hit.duration,
    })

    if (result.existsInDb) {
      conflictModalHit.value = hit
      conflictModalId.value = hit.id
      conflictModalExistingResult.value = result
      openConflictModal()
    } else {
      proceedToAssociateTrackModal(hit, result)
    }
  } catch (error) {
    toast.error(t('lyricsSearch.errorOpen'))
    console.error(error)
  } finally {
    isOpeningTrack.value = false
  }
}

const openEditLyricsV2WithTrack = localTrack => {
  editingAudioSource.value = {
    type: 'library',
    id: localTrack.id,
    file_path: localTrack.file_path,
    duration: localTrack.duration,
    title: localTrack.title,
    artist_name: localTrack.artist_name,
    album_name: localTrack.album_name,
  }
  editingLyricsfile.value = {
    id: associateModalLyricsfileId.value,
    content: associateModalInitialLyricsfile.value,
  }
  // trackId is null for temporary associations - the lyricsfile should not be
  // associated with this track.
  editingTrackId.value = null
  openEditLyricsV2Modal()
}

const openEditLyricsV2WithFile = fileMetadata => {
  editingAudioSource.value = {
    type: 'file',
    file_path: fileMetadata.file_path,
    duration: fileMetadata.duration,
    title: fileMetadata.title,
    artist_name: fileMetadata.artist_name,
    album_name: fileMetadata.album_name,
  }
  editingLyricsfile.value = {
    id: associateModalLyricsfileId.value,
    content: associateModalInitialLyricsfile.value,
  }
  editingTrackId.value = null
  openEditLyricsV2Modal()
}
</script>

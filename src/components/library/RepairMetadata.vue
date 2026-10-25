<template>
  <BaseModal
    :title="$t('repair.title')"
    content-class="w-full h-[80vh] max-w-screen-sm"
    background="non-interactive"
    :lock-scroll="true"
    @close="emit('close')"
  >
    <div class="grow overflow-hidden flex flex-col gap-4">
      <!-- What the repair does, and what it does not. -->
      <div class="text-xs text-neutral-600 dark:text-neutral-400">
        {{ $t('repair.introduction') }}
      </div>

      <!-- The options. -->
      <div class="flex flex-col gap-2 flex-none">
        <label class="flex items-center gap-2 text-xs text-neutral-800 dark:text-neutral-200 cursor-pointer">
          <input v-model="fixCover" type="checkbox" class="mt-0.5" />
          <span>{{ $t('repair.fixCover') }}</span>
        </label>

        <div class="flex flex-col">
          <label class="group-label mb-1" for="repair-script">{{ $t('repair.script') }}</label>
          <select id="repair-script" v-model="script" class="select px-2 h-8">
            <option value="off">{{ $t('repair.scriptOff') }}</option>
            <option value="simplified">{{ $t('repair.scriptSimplified') }}</option>
            <option value="traditional">{{ $t('repair.scriptTraditional') }}</option>
          </select>
          <span class="mt-1 text-xs text-neutral-500 dark:text-neutral-400">{{
            $t('repair.scriptHint')
          }}</span>
        </div>
      </div>

      <div class="grow overflow-auto flex flex-col gap-4">
        <!-- The albums whose name came from a folder. -->
        <div v-if="albums.length" class="flex flex-col gap-2">
          <div class="font-bold text-sm text-neutral-800 dark:text-neutral-200">
            {{ $t('repair.albumsTitle', { count: albums.length }) }}
          </div>
          <div class="text-xs text-neutral-500 dark:text-neutral-400">
            {{ $t('repair.albumsBody') }}
          </div>

          <div class="flex flex-col gap-2">
            <div
              v-for="album in albums"
              :key="album.id"
              class="rounded bg-white dark:bg-neutral-900 border border-neutral-200 dark:border-neutral-800 px-2 py-1.5"
            >
              <div class="flex items-center gap-2">
                <span class="font-bold text-sm text-neutral-800 dark:text-neutral-200">
                  {{ album.name }}
                </span>
                <span class="text-xs text-neutral-500 dark:text-neutral-400">
                  {{ $t('repair.albumSummary', { artists: album.artists, tracks: album.tracks }) }}
                </span>
                <button
                  class="button button-normal px-2 py-0.5 text-xs rounded ml-auto"
                  @click="openAlbum(album)"
                >
                  {{ $t('repair.lookUp') }}
                </button>
              </div>

              <!-- The tracks of this album, with what the release database
                   says about each. -->
              <div v-if="openAlbumId === album.id" class="mt-2 flex flex-col gap-1">
                <div v-if="lookingUp" class="text-xs text-neutral-500">
                  {{ $t('repair.lookingUp') }}
                </div>
                <label
                  v-for="track in tracksOf(album.id)"
                  :key="track.id"
                  class="flex items-start gap-2 text-xs text-neutral-800 dark:text-neutral-200 cursor-pointer"
                >
                  <input
                    v-model="selectedTracks"
                    type="checkbox"
                    :value="track.id"
                    :disabled="!track.realAlbum"
                    class="mt-0.5"
                  />
                  <span class="flex flex-col">
                    <span>
                      <span class="font-bold">{{ track.title }}</span>
                      <span> — {{ track.artist }}</span>
                    </span>
                    <span v-if="track.realAlbum" class="text-neutral-500 dark:text-neutral-400">
                      {{ $t('repair.willBecome') }}: {{ track.realAlbum }}
                      <span v-if="track.year"> ({{ track.year }})</span>
                      <span v-if="track.hasCover"> · {{ $t('repair.hasCover') }}</span>
                    </span>
                    <span v-else class="text-neutral-500 dark:text-neutral-400">
                      {{ $t('repair.notFound') }}
                    </span>
                  </span>
                </label>
              </div>
            </div>
          </div>
        </div>

        <!-- The files whose text is not in UTF-8. -->
        <div v-if="nonUtf8.length" class="flex flex-col gap-2">
          <div class="font-bold text-sm text-neutral-800 dark:text-neutral-200">
            {{ $t('repair.encodingTitle', { count: nonUtf8.length }) }}
          </div>
          <div class="text-xs text-neutral-500 dark:text-neutral-400">
            {{ $t('repair.encodingBody') }}
          </div>
          <div class="flex flex-col gap-1 max-h-48 overflow-auto">
            <label
              v-for="track in nonUtf8"
              :key="track.id"
              class="flex items-start gap-2 text-xs text-neutral-800 dark:text-neutral-200 cursor-pointer"
            >
              <input v-model="selectedTracks" type="checkbox" :value="track.id" class="mt-0.5" />
              <span>
                <span class="font-bold">{{ track.title }}</span>
                <span> — {{ track.artist }}</span>
              </span>
            </label>
          </div>
        </div>

        <div
          v-if="!albums.length && !nonUtf8.length && !loading"
          class="text-sm text-neutral-500"
        >
          {{ $t('repair.nothingToDo') }}
        </div>

        <!-- What the repair did. -->
        <div v-if="outcomes.length" class="flex flex-col gap-2">
          <div class="font-bold text-sm text-neutral-800 dark:text-neutral-200">
            {{ $t('repair.resultTitle', { count: outcomes.length }) }}
          </div>
          <div class="flex flex-col gap-1 max-h-48 overflow-auto">
            <div
              v-for="outcome in outcomes"
              :key="outcome.path"
              class="text-xs text-neutral-600 dark:text-neutral-400"
            >
              <span class="font-bold text-neutral-800 dark:text-neutral-200">
                {{ outcome.title }}
              </span>
              <span v-if="outcome.albumChanged">
                — {{ $t('repair.albumChanged') }}: {{ outcome.oldAlbum }} →
                {{ outcome.newAlbum }}
              </span>
              <span v-if="outcome.coverWritten"> · {{ $t('repair.coverWritten') }}</span>
              <span v-if="outcome.scriptChanged"> · {{ $t('repair.scriptChanged') }}</span>
              <span v-if="outcome.encodingChanged">
                · {{ $t('repair.encodingChanged') }}
              </span>
              <span v-if="outcome.note" class="block text-yellow-700 dark:text-yellow-400">
                {{ outcome.note }}
              </span>
            </div>
          </div>
        </div>
      </div>

      <div class="flex-none flex justify-end gap-2">
        <button class="button button-normal px-4 py-1.5 text-xs rounded-full" @click="emit('close')">
          {{ $t('actions.cancel') }}
        </button>
        <button
          class="button button-primary px-4 py-1.5 text-xs rounded-full"
          :class="{ 'button-disabled': !selectedTracks.length || working }"
          :disabled="!selectedTracks.length || working"
          @click="repair"
        >
          {{ working ? $t('repair.working') : $t('repair.repairSelected', { count: selectedTracks.length }) }}
        </button>
      </div>
    </div>
  </BaseModal>
</template>

<script setup>
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import BaseModal from '@/components/common/BaseModal.vue'

const emit = defineEmits(['close'])

const { t } = useI18n()
const toast = useToast()

const albums = ref([])
const nonUtf8 = ref([])
const selectedTracks = ref([])
const outcomes = ref([])
const openAlbumId = ref(null)
const tracks = ref([])
const lookingUp = ref(false)
const working = ref(false)
const loading = ref(true)
const fixCover = ref(true)
const script = ref('off')

// The tracks of the album that is open, and the ones that were looked up.
const tracksOf = albumId => tracks.value.filter(track => track.albumId === albumId)

const load = async () => {
  loading.value = true
  try {
    const [suspicious, encodings] = await Promise.all([
      invoke('find_suspicious_albums'),
      invoke('find_non_utf8_tracks'),
    ])
    albums.value = suspicious
    nonUtf8.value = encodings
  } catch (error) {
    console.error('Failed to look at the library:', error)
    toast.error(String(error))
  } finally {
    loading.value = false
  }
}

const openAlbum = async album => {
  if (openAlbumId.value === album.id) {
    openAlbumId.value = null
    return
  }
  openAlbumId.value = album.id
  lookingUp.value = true
  try {
    const found = await invoke('look_up_album_tracks', { albumId: album.id })
    tracks.value = [
      ...tracks.value.filter(track => track.albumId !== album.id),
      ...found.map(track => ({ ...track, albumId: album.id })),
    ]
  } catch (error) {
    console.error('Failed to look the album up:', error)
    toast.error(String(error))
  } finally {
    lookingUp.value = false
  }
}

const repair = async () => {
  working.value = true
  try {
    outcomes.value = await invoke('repair_tracks', {
      trackIds: selectedTracks.value,
      lyricScript: script.value,
      fixCover: fixCover.value,
    })
    toast.success(t('repair.done', { count: outcomes.value.length }))
  } catch (error) {
    console.error('The repair failed:', error)
    toast.error(String(error))
  } finally {
    working.value = false
  }
}

load()
</script>

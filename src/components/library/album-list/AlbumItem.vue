<template>
  <div
    class="flex w-full group hover:bg-neutral-50 hover:shadow hover:shadow-neutral-100/50 border border-transparent hover:border-neutral-100 transition rounded cursor-default dark:hover:bg-neutral-900 dark:hover:border-neutral-700 dark:hover:shadow-black/50"
  >
    <div v-if="album" class="p-1 flex items-center gap-3 grow" @click="$emit('openAlbum', album)">
      <!-- Thumbnail. Albums without art keep the placeholder so every row has
           the same leading edge and the text never shifts sideways. -->
      <div class="flex-none">
        <img
          v-if="coverSrc"
          :src="coverSrc"
          alt=""
          class="w-10 h-10 rounded object-cover shadow-sm"
          draggable="false"
        />
        <div
          v-else
          class="w-10 h-10 rounded bg-neutral-100 dark:bg-neutral-800 flex justify-center items-center text-neutral-400 dark:text-neutral-600"
        >
          <AlbumIcon class="text-xl" />
        </div>
      </div>

      <div class="flex flex-col min-w-0">
        <div class="font-bold text-sm text-neutral-800 dark:text-neutral-200 truncate">
          {{ album.name }}
        </div>

        <div class="flex items-center gap-2 min-w-0">
          <div
            class="text-sm text-neutral-500 group-hover:text-neutral-500 transition dark:text-neutral-400 dark:group-hover:text-neutral-400 flex-none"
          >
            {{ album.tracks_count }} {{ $t('album.tracks') }}
          </div>
          <div class="border-r border-neutral-300 h-3 flex-none dark:border-neutral-600" />
          <div
            class="text-sm text-neutral-500 group-hover:text-neutral-500 transition dark:text-neutral-400 dark:group-hover:text-neutral-400 truncate"
          >
            {{ album.artist_name }}
          </div>
        </div>
      </div>
    </div>

    <div class="flex items-center gap-2 p-1">
      <div v-if="album" class="transition gap-1">
        <button
          class="text-neutral-800 hover:bg-hoa-1100 hover:text-white rounded p-2 transition dark:text-white dark:hover:bg-hoa-1100 dark:hover:text-white"
          @click.stop.prevent="downloadLyricsMultiple"
          :disabled="!album || album.id !== props.albumId"
          :aria-label="`Download lyrics for ${album?.name ?? ''}`"
        >
          <DownloadMultiple />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import DownloadMultiple from '~icons/mdi/download-multiple'
import AlbumIcon from '~icons/mdi/album'
import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useDownloadTrigger } from '@/composables/download-options.js'

const props = defineProps(['albumId'])
defineEmits(['openAlbum'])

const album = ref(null)
const coverSrc = ref(null)

const downloadLyricsMultiple = useDownloadTrigger(() =>
  album.value?.id === props.albumId
    ? { type: 'album', id: album.value.id, name: album.value.name }
    : null
)

const loadAlbum = async id => {
  album.value = null
  coverSrc.value = null
  const loaded = await invoke('get_album', { albumId: id })
  if (id !== props.albumId) return

  album.value = loaded

  // The cover comes back as a data URL rather than a file path: with
  // `useHttpsScheme` the webview is an https origin, and WebKitGTK will not
  // decode an `asset:` source from there. A separate lookup keeps a missing
  // cover from failing the whole row — no art is a normal state.
  try {
    const url = await invoke('get_album_cover', { albumId: id })
    if (url && id === props.albumId) {
      coverSrc.value = url
    }
  } catch {
    coverSrc.value = null
  }
}

watch(
  () => props.albumId,
  async newId => {
    if (newId) {
      await loadAlbum(newId)
    }
  }
)

onMounted(async () => {
  await loadAlbum(props.albumId)
})
</script>

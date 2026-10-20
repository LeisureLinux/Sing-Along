<template>
  <div ref="parentRef" class="secondary-page">
    <div :style="{ height: `${totalSize}px`, width: '100%', position: 'relative' }">
      <div class="mb-4">
        <button class="button button-normal transition rounded-full p-4" @click="$emit('back')">
          <ArrowLeft />
        </button>
      </div>

      <!-- Album header: large cover, then the identifying text. The cover keeps
           a fixed footprint when missing so the track table below does not jump
           between albums that do and do not have art. -->
      <div class="flex gap-5 mb-8">
        <div class="flex-none">
          <img
            v-if="coverSrc"
            :src="coverSrc"
            :alt="album.name"
            class="w-40 h-40 rounded-lg object-cover shadow-md"
            draggable="false"
          />
          <div
            v-else
            class="w-40 h-40 rounded-lg bg-neutral-100 dark:bg-neutral-800 flex justify-center items-center text-neutral-400 dark:text-neutral-600"
          >
            <AlbumIcon class="text-6xl" />
          </div>
        </div>

        <div class="flex flex-col justify-between grow min-w-0">
          <div>
            <div class="text-thin text-2xl text-neutral-900 dark:text-white truncate">
              {{ album.name }}
            </div>
            <div class="flex items-center gap-2 mt-1">
              <div class="text-sm text-neutral-800 dark:text-neutral-300 truncate">
                {{ album.artist_name }}
              </div>
              <div class="border-r border-neutral-300 h-3 flex-none" />
              <div class="text-sm text-neutral-800 dark:text-neutral-300 flex-none">
                {{ $t('album.trackCount', { count: album.tracks_count }) }}
              </div>
            </div>
          </div>

          <div>
            <button
              class="button button-normal px-4 py-1.5 text-xs rounded-full inline-flex items-center gap-1"
              @click.stop.prevent="downloadAlbumLyrics"
            >
              <DownloadMultiple class="text-sm" />
              <span>{{ $t('album.downloadAlbumLyrics') }}</span>
            </button>
          </div>
        </div>
      </div>

      <div class="w-full">
        <div class="w-full flex">
          <div class="text-xs text-neutral-800/70 font-bold flex w-full dark:text-neutral-500">
            <div class="text-right flex-none w-[5%] p-1 pr-2">#</div>
            <div class="text-left flex-none w-[60%] p-1">{{ $t('track.track') }}</div>
            <!-- Adjusted width percentage -->
            <div class="text-right flex-none w-[10%] p-1">{{ $t('track.duration') }}</div>
            <div class="text-center flex-none w-[10%] p-1">{{ $t('track.lyrics') }}</div>
            <div class="text-right flex-none w-[15%] p-1" />
          </div>
        </div>
        <div class="w-full flex flex-col">
          <div
            v-for="virtualRow in virtualRows"
            :key="virtualRow.key"
            class="group flex flex-col w-full"
            :style="{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              height: `${virtualRow.size}px`,
              transform: `translateY(${virtualRow.start}px)`,
            }"
          >
            <TrackItem
              :track-id="virtualRow.key"
              :is-show-track-number="true"
              :queue="trackIds"
              :queue-index="virtualRow.index"
              @play-track="playTrack"
              @play-with-queue="playWithQueue"
              @download-lyrics="downloadLyrics"
            />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import ArrowLeft from '~icons/mdi/arrow-left'
import AlbumIcon from '~icons/mdi/album'
import DownloadMultiple from '~icons/mdi/download-multiple'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { ref, computed, watch, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import TrackItem from '../track-list/TrackItem.vue'
import { useDownloadTrigger } from '@/composables/download-options.js'

const props = defineProps(['album'])
const emit = defineEmits(['back', 'playTrack', 'playWithQueue', 'downloadLyrics'])

const trackIds = ref([])
const parentRef = ref(null)
const coverSrc = ref(null)

const rowVirtualizer = useVirtualizer(
  computed(() => ({
    count: trackIds.value.length,
    getScrollElement: () => parentRef.value,
    estimateSize: () => 52,
    overscan: 5,
    paddingStart: 175,
    getItemKey: index => trackIds.value[index],
  }))
)

const virtualRows = computed(() => rowVirtualizer.value.getVirtualItems())

const totalSize = computed(() => rowVirtualizer.value.getTotalSize())

const playTrack = track => {
  emit('playTrack', track)
}

const playWithQueue = ({ track, index }) => {
  emit('playWithQueue', { track, queue: trackIds.value, index })
}

const downloadLyrics = track => {
  emit('downloadLyrics', track)
}

const downloadAlbumLyrics = useDownloadTrigger(() => ({
  type: 'album',
  id: props.album.id,
  name: props.album.name,
}))

const loadCover = async albumId => {
  coverSrc.value = null
  if (!albumId) return
  try {
    const url = await invoke('get_album_cover', { albumId })
    // Guard against a stale response landing after the user opened another
    // album: only apply it if this component still shows that album.
    if (url && albumId === props.album?.id) {
      coverSrc.value = url
    }
  } catch (error) {
    console.error('Failed to load album cover:', error)
    coverSrc.value = null
  }
}

onMounted(async () => {
  trackIds.value = await invoke('get_album_track_ids', { albumId: props.album.id })
  await loadCover(props.album.id)
})

watch(
  () => props.album?.id,
  async newId => {
    if (newId) {
      trackIds.value = await invoke('get_album_track_ids', { albumId: newId })
      await loadCover(newId)
    }
  }
)
</script>

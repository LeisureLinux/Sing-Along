<template>
  <div v-show="props.isActive" ref="parentRef" class="p-4 overflow-y-auto h-full">
    <div :style="{ height: `${totalSize}px`, width: '100%', position: 'relative' }">
      <div class="w-full">
        <div class="w-full flex">
          <div class="text-xs text-neutral-500 font-bold flex w-full dark:text-neutral-500">
            <div class="text-left flex-none w-[63%] p-1">{{ $t('track.track') }}</div>
            <!-- Adjusted width percentage -->
            <div class="text-right flex-none w-[10%] p-1">{{ $t('track.duration') }}</div>
            <!-- The lyric-status column doubles as the filter. Kept in the row
                 header rather than the toolbar so the control sits with the data
                 it filters and cannot push the header's menu button off-screen. -->
            <div class="text-center flex-none w-[12%] p-1">
              <!-- Filled in the theme colour when idle, because a column header
                   that is just text does not read as something you can press.
                   Once the filter is on it goes quiet and shows the active state
                   instead, so the table reads normally while still saying that a
                   subset is being shown. -->
              <button
                class="uppercase font-bold cursor-pointer select-none rounded px-1.5 py-0.5 transition text-[0.65rem] w-full truncate"
                :class="
                  pendingOnly
                    ? 'bg-hoa-1400 text-white hover:bg-hoa-1500'
                    : 'bg-hoa-1100 text-white hover:bg-hoa-1200'
                "
                :title="$t('filters.syncStatusHint')"
                @click.stop.prevent="togglePendingOnly"
              >
                {{ pendingOnly ? $t('filters.pendingOnly') : $t('filters.syncStatus') }}
              </button>
            </div>
            <div class="text-right flex-none w-[15%] p-1" />
          </div>
        </div>
        <div class="w-full flex flex-col">
          <div
            v-for="virtualRow in virtualRows"
            :key="virtualRow.key"
            class="group flex flex-col"
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
import TrackItem from './track-list/TrackItem.vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { ref, computed, watch, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useSearchLibrary } from '@/composables/search-library.js'

const props = defineProps(['isActive'])
const emit = defineEmits(['playTrack', 'playWithQueue', 'downloadLyrics'])

const trackIds = ref([])
const parentRef = ref(null)

const rowVirtualizer = useVirtualizer(
  computed(() => ({
    count: trackIds.value.length,
    getScrollElement: () => parentRef.value,
    estimateSize: () => 52,
    overscan: 5,
    paddingStart: 32,
    getItemKey: index => trackIds.value[index],
  }))
)

const { searchValue, setSearch, filters } = useSearchLibrary()

// "Pending" means the track has no synced lyrics yet. The library filter is
// phrased the other way round ("include tracks that already have synced
// lyrics"), so pending is its negation. Derived from the shared filter rather
// than a local flag, so the dropdown and this header stay in step.
const pendingOnly = computed(() => !filters.value.syncedLyricsTracks)

const togglePendingOnly = () => {
  const next = { ...filters.value, syncedLyricsTracks: pendingOnly.value }
  setSearch(searchValue.value, next)
}

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

const getTrackIds = async () => {
  trackIds.value = []
  try {
    trackIds.value = await invoke('get_track_ids', {
      searchQuery: searchValue.value,
      syncedLyricsTracks: filters.value.syncedLyricsTracks,
      plainLyricsTracks: filters.value.plainLyricsTracks,
      instrumentalTracks: filters.value.instrumentalTracks,
      noLyricsTracks: filters.value.noLyricsTracks,
    })
  } catch (error) {
    console.error(error)
  }
}

onMounted(async () => {
  if (props.isActive) {
    await getTrackIds()
  }
})

watch(
  () => props.isActive,
  async () => {
    if (props.isActive) {
      await getTrackIds()
    }
  }
)

watch(searchValue, async () => {
  try {
    await getTrackIds()
  } catch (error) {
    console.error(error)
  }
})

watch(
  filters,
  async () => {
    try {
      await getTrackIds()
    } catch (error) {
      console.error(error)
    }
  },
  { deep: true }
)
</script>

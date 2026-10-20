<template>
  <div
    v-show="props.isActive"
    class="flex flex-col justify-center gap-6 items-center w-full h-full p-4"
  >
    <div class="text-xl font-bold text-neutral-800 dark:text-neutral-200">
      {{ $t('lyricsSearch.title') }}
    </div>

    <form
      class="flex items-center rounded-full w-full max-w-screen-sm h-auto overflow-hidden bg-white dark:bg-neutral-900 border border-neutral-300 dark:border-neutral-700 transition"
      :class="{ 'ring-2 ring-hoa-1100/40': inputActive }"
      @submit.prevent="onSubmit"
    >
      <input
        v-model="keyword"
        type="text"
        class="outline-none grow h-12 px-6 bg-transparent text-neutral-800 dark:text-neutral-200 placeholder:text-neutral-400 dark:placeholder:text-neutral-500"
        :placeholder="$t('lyricsSearch.placeholder')"
        :autofocus="props.isActive"
        @focus="inputActive = true"
        @blur="inputActive = false"
      />
      <button class="rounded-full button button-normal h-10 w-10 mr-1">
        <Magnify />
      </button>
    </form>

    <!-- Source selection. Both are public services with different catalogues,
         so searching both is often how a missing lyric gets found; letting the
         user uncheck one keeps a slow or blocked source out of the wait. -->
    <div class="flex flex-wrap gap-4 justify-center text-sm">
      <label class="flex items-center gap-1 cursor-pointer">
        <input v-model="useLrclib" type="checkbox" class="accent-hoa-1100" />
        <span class="text-neutral-800 dark:text-neutral-300">LRCLIB</span>
      </label>
      <label class="flex items-center gap-1 cursor-pointer">
        <input v-model="useNetease" type="checkbox" class="accent-hoa-1100" />
        <span class="text-neutral-800 dark:text-neutral-300">NetEase</span>
      </label>
    </div>

    <Transition name="slide-fade">
      <SearchResult
        v-if="searchingKeyword"
        :keyword="searchingKeyword"
        :use-lrclib="submittedLrclib"
        :use-netease="submittedNetease"
        @back="searchingKeyword = null"
      />
    </Transition>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import Magnify from '~icons/mdi/magnify'
import SearchResult from './my-lrclib/SearchResult.vue'

const props = defineProps({
  isActive: {
    type: Boolean,
    default: false,
  },
})

const searchingKeyword = ref('')
const keyword = ref('')
const inputActive = ref(false)

const useLrclib = ref(true)
const useNetease = ref(true)

// The result view keeps searching the sources that were ticked when the search
// was submitted, not whatever the boxes say later: changing a checkbox while
// results are on screen should not silently re-run the query underneath.
const submittedLrclib = ref(true)
const submittedNetease = ref(true)

const onSubmit = () => {
  if (!keyword.value.trim()) {
    return
  }
  submittedLrclib.value = useLrclib.value
  submittedNetease.value = useNetease.value
  searchingKeyword.value = keyword.value
}
</script>

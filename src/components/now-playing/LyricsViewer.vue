<template>
  <transition name="slide-fade" mode="out-in">
    <div
      v-if="syncedLines.length > 0 && duration != null && progress != null"
      class="flex flex-col gap-1 relative"
    >
      <transition name="slide-fade" mode="out-in">
        <div
          v-if="expanded"
          class="full-viewer absolute bottom-0 left-0 w-full h-[40vh] bg-neutral-50 dark:bg-neutral-900 border-t border-neutral-200/50 dark:border-neutral-800/50 overflow-hidden"
        >
          <div class="relative h-full">
            <div class="flex justify-center items-center h-6 w-full relative z-10">
              <button
                class="text-xl text-neutral-800 dark:text-neutral-400 w-full flex justify-center"
                type="button"
                @click="expand"
              >
                <DragHorizontal />
              </button>
            </div>

            <div
              id="full-lyrics-container"
              class="h-full text-center transition"
              :style="{ transform: fullViewTransform }"
            >
              <p
                v-for="(line, index) in syncedLines"
                :key="index"
                class="transition"
                :class="{
                  'font-bold text-neutral-800 dark:text-neutral-200': isLinePlaying(line, index),
                  'text-neutral-800/70 hover:text-neutral-800/80 hover:cursor-pointer dark:text-neutral-400/70 dark:hover:text-neutral-300/80':
                    !isLinePlaying(line, index),
                }"
                @click="onLineClick(line)"
              >
                <template v-if="isLinePlaying(line, index)">
                  <span
                    v-for="(word, wordIndex) in getKaraokeWords(line, index)"
                    :key="wordIndex"
                    class="karaoke"
                    :style="karaokeWordStyle(line, index, wordIndex)"
                  >
                    <span class="karaoke-base">{{ word.text }}</span>
                    <span class="karaoke-fill" aria-hidden="true">{{ word.text }}</span>
                  </span>
                </template>
                <template v-else>
                  {{ line.text }}
                </template>
              </p>
            </div>

            <button
              class="z-10 absolute bottom-2 right-2 flex items-center gap-1 px-3 py-1 rounded text-xs font-bold bg-neutral-100 text-neutral-800 dark:bg-neutral-800 dark:text-neutral-300 hover:bg-neutral-200 dark:hover:bg-neutral-700 shadow"
              type="button"
              :aria-label="copied ? 'Copied' : 'Copy'"
              @click.stop="onCopy"
            >
              <ContentCopy class="w-4 h-4" />
              <span>{{ copied ? 'Copied' : 'Copy' }}</span>
            </button>

            <div
              class="absolute top-0 left-0 w-full h-10 bg-gradient-to-b from-neutral-50 dark:from-neutral-900 pointer-events-none"
            />
            <div
              class="absolute bottom-0 left-0 w-full h-10 bg-gradient-to-t from-neutral-50 dark:from-neutral-900 pointer-events-none"
            />
          </div>
        </div>
      </transition>

      <div
        class="mini-viewer transition cursor-pointer top-0 left-0 w-full h-12 bg-neutral-50 dark:bg-neutral-900 border-t border-neutral-200/50 dark:border-neutral-800/50 flex flex-col"
        :class="{ 'invisible opacity-0': expanded }"
        @click="expand"
      >
        <div class="flex justify-center items-center h-4 w-full">
          <button
            class="text-xl text-neutral-800 dark:text-neutral-400 w-full flex justify-center"
            type="button"
          >
            <DragHorizontal />
          </button>
        </div>

        <transition name="slide-fade" mode="out-in">
          <div
            :key="currentLyrics"
            class="flex w-full justify-center items-center text-neutral-800 dark:text-neutral-200 text-sm grow font-bold"
          >
            <template
              v-if="
                primaryPlayingLineIndex >= 0 &&
                isLinePlaying(syncedLines[primaryPlayingLineIndex], primaryPlayingLineIndex)
              "
            >
              <span
                v-for="(word, wordIndex) in getKaraokeWords(
                  syncedLines[primaryPlayingLineIndex],
                  primaryPlayingLineIndex
                )"
                :key="wordIndex"
                class="karaoke"
                :style="karaokeWordStyle(
                  syncedLines[primaryPlayingLineIndex],
                  primaryPlayingLineIndex,
                  wordIndex
                )"
              >
                <span class="karaoke-base">{{ word.text }}</span>
                <span class="karaoke-fill" aria-hidden="true">{{ word.text }}</span>
              </span>
            </template>
            <template v-else>
              {{ currentLyrics }}
            </template>
          </div>
        </transition>
      </div>
    </div>
  </transition>
</template>

<script setup>
import DragHorizontal from '~icons/mdi/drag-horizontal'
import ContentCopy from '~icons/mdi/content-copy'
import { ref, watch, nextTick } from 'vue'
import { computed } from 'vue'
import { parseLyricsfile } from '@/utils/lyricsfile.js'
import { ensureLineWords } from '@/utils/word-tokenizer.js'

const props = defineProps(['duration', 'progress', 'lyricsfile'])
const emit = defineEmits(['lyricsClicked'])

const expanded = ref(false)
const currentLineElementOffset = ref(null)
const copied = ref(false)

const parsedLyricsfile = computed(() => {
  if (!props.lyricsfile) {
    return null
  }
  return parseLyricsfile(props.lyricsfile)
})

const syncedLines = computed(() => {
  if (!parsedLyricsfile.value) {
    return []
  }
  return parsedLyricsfile.value.syncedLines || []
})

const progressMs = computed(() => (props.progress ?? 0) * 1000)

// Check if a line is currently playing based on its own time range
const isLinePlaying = (line, index) => {
  if (!line || !Number.isFinite(line.start_ms)) {
    return false
  }
  const lineStart = line.start_ms
  const lineEnd = Number.isFinite(line.end_ms)
    ? line.end_ms
    : (syncedLines.value[index + 1]?.start_ms ?? Infinity)
  return progressMs.value >= lineStart && progressMs.value < lineEnd
}

// For minimal view: find the primary playing line (prioritize by start time if multiple overlap)
const primaryPlayingLineIndex = computed(() => {
  const lines = syncedLines.value
  if (!lines || lines.length === 0) {
    return -1
  }

  // Find all lines that are currently playing
  const playingIndices = []
  for (let i = 0; i < lines.length; i++) {
    if (isLinePlaying(lines[i], i)) {
      playingIndices.push(i)
    }
  }

  if (playingIndices.length === 0) {
    // No line is playing, find the most recent past line
    for (let i = lines.length - 1; i >= 0; i--) {
      if (progressMs.value >= lines[i].start_ms) {
        return i
      }
    }
    return -1
  }

  // Return the line with the latest start time (most recently started)
  return playingIndices[playingIndices.length - 1]
})

const currentLyrics = computed(() => {
  const index = primaryPlayingLineIndex.value
  if (index === null || index < 0 || index >= syncedLines.value.length) {
    return '…'
  }
  const line = syncedLines.value[index]
  return line?.text || '…'
})

const hasWordSync = (lines, index) => {
  if (!lines || index === null || index < 0) {
    return false
  }
  const line = lines[index]
  return line && line.words && Array.isArray(line.words) && line.words.length > 0
}

/*
 * Karaoke highlight.
 *
 * Lines that carry real word timings sweep word by word. Lines that only have
 * line-level timings get their words filled in by `ensureLineWords` (the same
 * helper the word-timing editor uses), so the highlight sweeps for every song
 * instead of snapping between whole lines.
 *
 * Tokenizing is expensive and this component re-renders on every playback tick
 * (~25fps), so the per-line word lists are derived once per lyricsfile below
 * rather than recomputed during render.
 */
const karaokeWordsByLine = computed(() =>
  syncedLines.value.map((line, index) => {
    if (!line) {
      return []
    }
    if (hasWordSync(syncedLines.value, index)) {
      return line.words
    }
    if (!line.text) {
      return []
    }
    return ensureLineWords(line, syncedLines.value, index)?.words ?? []
  })
)

const getKaraokeWords = (line, index) => karaokeWordsByLine.value[index] ?? []

// Resolve the end time of a line: explicit end_ms, else the next line's start.
const getLineEndMs = (line, index) => {
  if (Number.isFinite(line?.end_ms)) {
    return line.end_ms
  }
  const next = syncedLines.value[index + 1]
  return Number.isFinite(next?.start_ms) ? next.start_ms : line?.start_ms + 4000
}

// Resolve a word's end time: explicit end_ms, else the next word's start, else
// the end of the line it belongs to.
const getWordEndMs = (words, line, lineIndex, wordIndex) => {
  const word = words?.[wordIndex]
  if (!word) {
    return null
  }
  if (Number.isFinite(word.end_ms)) {
    return word.end_ms
  }
  const next = words[wordIndex + 1]
  return Number.isFinite(next?.start_ms) ? next.start_ms : getLineEndMs(line, lineIndex)
}

// Fraction of a span [start, end] already elapsed at the current playback time.
const fillRatio = (start, end) => {
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) {
    return progressMs.value >= start ? 1 : 0
  }
  const ratio = (progressMs.value - start) / (end - start)
  return Math.min(1, Math.max(0, ratio))
}

// Karaoke fill for a single word. Finished words stay fully filled while the
// active one sweeps continuously, so the yellow grows rather than jumping.
const karaokeWordStyle = (line, lineIndex, wordIndex) => {
  const words = getKaraokeWords(line, lineIndex)
  const word = words[wordIndex]
  if (!word || !Number.isFinite(word.start_ms)) {
    return { '--karaoke-fill': '0%' }
  }
  const end = getWordEndMs(words, line, lineIndex, wordIndex)
  const ratio = fillRatio(word.start_ms, end)
  return { '--karaoke-fill': `${(ratio * 100).toFixed(2)}%` }
}

// For expanded view scrolling: use the primary playing line for scroll position
const updateCurrentLineElementOffset = () => {
  const newCurrentIndex = primaryPlayingLineIndex.value

  if (newCurrentIndex === -1) {
    currentLineElementOffset.value = null
    return
  }

  const fullLyricsContainerEl = document.getElementById('full-lyrics-container')
  if (fullLyricsContainerEl) {
    const currentLyricsLine = Array.from(fullLyricsContainerEl.children)[newCurrentIndex]
    if (currentLyricsLine) {
      currentLineElementOffset.value = currentLyricsLine.offsetTop
    }
  }
}

const fullViewTransform = computed(() => {
  if (!currentLineElementOffset.value) {
    return `translateY(calc(50% - 2.5em))`
  }

  return `translateY(calc(50% - 2.5em - ${currentLineElementOffset.value}px))`
})

const expand = () => {
  expanded.value = !expanded.value
  nextTick(() => {
    updateCurrentLineElementOffset()
  })
}

const onLineClick = line => {
  emit('lyricsClicked', { timestamp: line.start_ms / 1000 })
}

const onCopy = async () => {
  try {
    const text = syncedLines.value.map(l => l.text).join('\n')
    if (!text) return
    await navigator.clipboard.writeText(text)
    copied.value = true
    setTimeout(() => (copied.value = false), 1500)
  } catch (e) {
    // swallow
  }
}

watch(
  () => props.progress,
  () => {
    updateCurrentLineElementOffset()
  },
  { immediate: true }
)
</script>

<style scoped>
/*
 * Karaoke fill: the base layer paints the plain text, and the fill layer on top
 * is clipped to --karaoke-fill (0-100%) so the highlight sweeps across instead of
 * snapping word by word.
 */
.karaoke {
  position: relative;
  display: inline-block;
  white-space: pre-wrap;
  --karaoke-fill: 0%;
  --karaoke-color: #eab308; /* tailwind yellow-500 */
}

.dark .karaoke {
  --karaoke-color: #facc15; /* tailwind yellow-400 */
}

.karaoke-base {
  white-space: pre-wrap;
}

.karaoke-fill {
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  white-space: pre-wrap;
  overflow: hidden;
  pointer-events: none;
  color: var(--karaoke-color);
  clip-path: inset(0 calc(100% - var(--karaoke-fill)) 0 0);
}

.slide-fade-enter-active {
  transition: all 0.05s ease-out;
}

.slide-fade-leave-active {
  transition: all 0.05s cubic-bezier(1, 0.5, 0.8, 1);
}

.slide-fade-leave-to {
  transform: translateY(-20px);
  opacity: 0;
}

.slide-fade-enter-from {
  transform: translateY(20px);
  opacity: 0;
}
</style>

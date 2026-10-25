<template>
  <BaseModal
    :title="$t('libraryProblems.title')"
    content-class="w-full h-[80vh] max-w-screen-sm"
    background="non-interactive"
    :lock-scroll="true"
    @close="emit('close')"
  >
    <div class="grow overflow-hidden flex flex-col gap-6">
      <div class="text-xs text-neutral-600 dark:text-neutral-400">
        {{ $t('libraryProblems.introduction') }}
      </div>

      <div class="grow overflow-auto flex flex-col gap-6">
        <!-- The entries whose file is not on the disk. -->
        <div v-if="missing.length" class="flex flex-col gap-2">
          <div class="font-bold text-sm text-neutral-800 dark:text-neutral-200">
            {{ $t('libraryProblems.missingTitle', { count: missing.length }) }}
          </div>
          <div class="text-xs text-neutral-500 dark:text-neutral-400">
            {{ $t('libraryProblems.missingBody') }}
          </div>
          <div class="flex flex-col gap-1 max-h-64 overflow-auto">
            <label
              v-for="track in missing"
              :key="track.id"
              class="flex items-start gap-2 text-xs text-neutral-800 dark:text-neutral-200 cursor-pointer rounded px-1 py-0.5 hover:bg-neutral-100 dark:hover:bg-neutral-800"
            >
              <input
                v-model="selectedMissing"
                type="checkbox"
                :value="track.id"
                class="mt-0.5"
              />
              <span class="flex flex-col">
                <span>
                  <span class="font-bold">{{ track.title }}</span>
                  <span> — {{ track.artist }}</span>
                </span>
                <span class="text-neutral-500 dark:text-neutral-400 break-all">
                  {{ track.filePath }}
                </span>
                <span v-if="track.hasLyrics" class="text-neutral-500 dark:text-neutral-400">
                  {{ $t('libraryProblems.hasLyrics') }}
                </span>
              </span>
            </label>
          </div>
        </div>

        <!-- The pairs that look like the same recording, grouped by how sure
             the library is. -->
        <div v-if="identicalPairs.length" class="flex flex-col gap-2">
          <div class="font-bold text-sm text-neutral-800 dark:text-neutral-200">
            {{ $t('libraryProblems.identicalTitle', { count: identicalPairs.length }) }}
          </div>
          <div class="text-xs text-neutral-500 dark:text-neutral-400">
            {{ $t('libraryProblems.identicalBody') }}
          </div>
          <div class="flex flex-col gap-1 max-h-48 overflow-auto">
            <label
              v-for="pair in identicalPairs"
              :key="`${pair.firstId}-${pair.secondId}`"
              class="flex items-start gap-2 text-xs text-neutral-800 dark:text-neutral-200 cursor-pointer rounded px-1 py-0.5 hover:bg-neutral-100 dark:hover:bg-neutral-800"
            >
              <input
                v-model="selectedDuplicates"
                type="checkbox"
                :value="pairKey(pair)"
                class="mt-0.5"
              />
              <span class="flex flex-col">
                <span>
                  <span class="font-bold">{{ pair.firstTitle }}</span>
                  <span> — {{ pair.firstArtist }}</span>
                </span>
                <span class="text-neutral-500 dark:text-neutral-400 break-all">
                  {{ pair.firstPath }}
                </span>
                <span class="text-neutral-500 dark:text-neutral-400 break-all">
                  {{ pair.secondPath }}
                </span>
              </span>
            </label>
          </div>
        </div>

        <div v-if="sameRecordingPairs.length" class="flex flex-col gap-2">
          <div class="font-bold text-sm text-neutral-800 dark:text-neutral-200">
            {{
              $t('libraryProblems.sameRecordingTitle', {
                count: sameRecordingPairs.length,
              })
            }}
          </div>
          <div class="text-xs text-neutral-500 dark:text-neutral-400">
            {{ $t('libraryProblems.sameRecordingBody') }}
          </div>
          <div class="flex flex-col gap-1 max-h-48 overflow-auto">
            <label
              v-for="pair in sameRecordingPairs"
              :key="`${pair.firstId}-${pair.secondId}`"
              class="flex items-start gap-2 text-xs text-neutral-800 dark:text-neutral-200 cursor-pointer rounded px-1 py-0.5 hover:bg-neutral-100 dark:hover:bg-neutral-800"
            >
              <input
                v-model="selectedDuplicates"
                type="checkbox"
                :value="pairKey(pair)"
                class="mt-0.5"
              />
              <span class="flex flex-col">
                <span>
                  <span class="font-bold">{{ pair.firstTitle }}</span>
                  <span> — {{ pair.firstArtist }}</span>
                </span>
                <span class="text-neutral-500 dark:text-neutral-400 break-all">
                  {{ pair.firstPath }}
                </span>
                <span class="text-neutral-500 dark:text-neutral-400 break-all">
                  {{ pair.secondPath }}
                </span>
              </span>
            </label>
          </div>
        </div>

        <div v-if="differentRecordingPairs.length" class="flex flex-col gap-2">
          <div class="font-bold text-sm text-neutral-800 dark:text-neutral-200">
            {{
              $t('libraryProblems.differentRecordingTitle', {
                count: differentRecordingPairs.length,
              })
            }}
          </div>
          <div class="text-xs text-neutral-500 dark:text-neutral-400">
            {{ $t('libraryProblems.differentRecordingBody') }}
          </div>
          <div class="flex flex-col gap-1 max-h-48 overflow-auto">
            <div
              v-for="pair in differentRecordingPairs"
              :key="`${pair.firstId}-${pair.secondId}`"
              class="text-xs text-neutral-600 dark:text-neutral-400 rounded px-1 py-0.5"
            >
              <span class="font-bold">{{ pair.firstTitle }}</span>
              <span> — {{ pair.firstArtist }}</span>
              <span class="block text-neutral-500 dark:text-neutral-400 break-all">
                {{ pair.firstPath }} · {{ humanDuration(pair.firstDuration) }}
              </span>
              <span class="block text-neutral-500 dark:text-neutral-400 break-all">
                {{ pair.secondPath }} · {{ humanDuration(pair.secondDuration) }}
              </span>
            </div>
          </div>
        </div>

        <div v-if="!missing.length && !duplicates.length" class="text-sm text-neutral-500">
          {{ $t('libraryProblems.nothingToDo') }}
        </div>
      </div>

      <div class="flex-none flex justify-end gap-2">
        <button class="button button-normal px-4 py-1.5 text-xs rounded-full" @click="emit('close')">
          {{ $t('actions.cancel') }}
        </button>
        <button
          class="button button-primary px-4 py-1.5 text-xs rounded-full"
          :class="{ 'button-disabled': !removalCount }"
          :disabled="!removalCount"
          @click="remove"
        >
          {{ $t('libraryProblems.removeSelected', { count: removalCount }) }}
        </button>
      </div>
    </div>
  </BaseModal>
</template>

<script setup>
import { invoke } from '@tauri-apps/api/core'
import { computed, onMounted, ref } from 'vue'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import BaseModal from '@/components/common/BaseModal.vue'
import { humanDuration } from '@/utils/human-duration.js'

const emit = defineEmits(['close'])

const { t } = useI18n()
const toast = useToast()

const missing = ref([])
const duplicates = ref([])
const selectedMissing = ref([])
const selectedDuplicates = ref([])

const identicalPairs = computed(() =>
  duplicates.value.filter(pair => pair.certainty === 'identical')
)
const sameRecordingPairs = computed(() =>
  duplicates.value.filter(pair => pair.certainty === 'sameRecording')
)
const differentRecordingPairs = computed(() =>
  duplicates.value.filter(pair => pair.certainty === 'differentRecording')
)

// The key a pair is selected by.
const pairKey = pair => `${pair.firstId}-${pair.secondId}`

// How many entries the selection removes.
//
// A pair of duplicates is one entry too many, so a selected pair removes one
// of the two — the one whose lyrics are not kept, or the second one.
const removalCount = computed(() => {
  let count = selectedMissing.value.length
  for (const key of selectedDuplicates.value) {
    const pair = duplicates.value.find(p => pairKey(p) === key)
    if (pair) {
      count += 1
    }
  }
  return count
})

const load = async () => {
  try {
    const problems = await invoke('get_library_problems')
    missing.value = problems.missing
    duplicates.value = problems.duplicates
  } catch (error) {
    console.error('Failed to read the library problems:', error)
    toast.error(String(error))
  }
}

const remove = async () => {
  try {
    const ids = [...selectedMissing.value]

    for (const key of selectedDuplicates.value) {
      const pair = duplicates.value.find(p => pairKey(p) === key)
      if (pair) {
        // The second of the two goes: the first is the one the user has
        // probably been playing.
        ids.push(pair.secondId)
      }
    }

    const removed = await invoke('remove_library_entries', { trackIds: ids })
    toast.success(t('libraryProblems.removed', { count: removed }))
    emit('close')
  } catch (error) {
    console.error(error)
    toast.error(String(error))
  }
}

onMounted(load)
</script>

<template>
  <div class="flex flex-col w-full h-full justify-center items-center">
    <div class="px-4 py-2 flex flex-col gap-4 flex-none">
      <div class="text-thin text-xl text-neutral-900 dark:text-neutral-200">
        {{ $t('setup.selectDirectories') }}
      </div>
    </div>

    <div class="grow flex flex-col items-center justify-center gap-8 w-full max-w-screen-sm">
      <div class="flex flex-col gap-2 w-full justify-center items-center">
        <div
          v-for="(directory, index) in directories"
          :key="directory"
          class="w-full bg-neutral-100 dark:bg-neutral-900 text-neutral-900 dark:text-neutral-200 font-bold p-4 rounded-lg flex items-center"
        >
          <div class="grow">
            {{ directory }}
          </div>
          <button
            class="flex-none button button-normal p-2 rounded-full"
            @click.prevent="removeDirectory(index)"
          >
            <Close />
          </button>
        </div>

        <button
          class="w-full bg-neutral-50 dark:bg-neutral-900 hover:bg-neutral-100 hover:dark:bg-neutral-800 active:bg-neutral-200/50 transition text-neutral-900 dark:text-neutral-300 font-bold p-4 rounded-lg flex items-center border border-dashed border-neutral-400 dark:border-neutral-600"
          @click.prevent="chooseDirectory"
        >
          <Plus />
          <div>{{ $t('config.addDirectory') }}</div>
        </button>
      </div>

      <div class="flex gap-3 w-full">
        <!-- Only in manage mode: on first setup there is nothing to go back to. -->
        <button
          v-if="isManaging"
          class="button button-normal flex-1 p-4 rounded-lg"
          @click.prevent="cancel"
        >
          {{ $t('actions.cancel') }}
        </button>
        <button
          class="button button-primary p-4 rounded-lg"
          :class="isManaging ? 'flex-1' : 'w-full'"
          @click.prevent="progressStep"
        >
          {{ $t('actions.continueLabel') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { open } from '@tauri-apps/plugin-dialog'
import { audioDir } from '@tauri-apps/api/path'
import { invoke } from '@tauri-apps/api/core'
import { ref, onMounted } from 'vue'
import Close from '~icons/mdi/close'
import Plus from '~icons/mdi/plus'
// True when opened from "Manage directories" rather than first-run setup.
// Read from the template only, so no `props` binding is needed here.
defineProps({
  isManaging: { type: Boolean, default: false },
})
const emit = defineEmits(['progressStep', 'directoriesChanged', 'cancel'])

// Leave without writing: the directories the user was editing are discarded.
const cancel = () => emit('cancel')

const directories = ref([])
const originalDirectories = ref([])

const progressStep = async () => {
  await invoke('set_directories', { directories: directories.value })

  // Check if directories actually changed
  const hasChanged =
    JSON.stringify(originalDirectories.value.sort()) !== JSON.stringify(directories.value.sort())

  if (hasChanged) {
    emit('directoriesChanged')
  }

  emit('progressStep')
}

onMounted(async () => {
  const directoriesFromDB = await invoke('get_directories')
  if (directoriesFromDB && directoriesFromDB.length > 0) {
    directories.value = [...directoriesFromDB]
    originalDirectories.value = [...directoriesFromDB]
  } else {
    const dirPath = await audioDir()
    directories.value.push(dirPath)
    originalDirectories.value = [dirPath]
  }
})

const chooseDirectory = async () => {
  const selected = await open({
    directory: true,
    recursive: true,
  })

  if (selected && !directories.value.includes(selected)) {
    directories.value.push(selected)
  }
}

const removeDirectory = index => {
  if (index < 0) {
    return
  }
  directories.value.splice(index, 1)
}
</script>

<template>
  <div class="h-screen w-screen flex flex-col select-none">
    <div
      v-if="!isProd"
      class="fixed top-0 left-0 flex justify-end items-start text-sm flex-none z-50"
    >
      <div
        class="p-0.5 m-1 rounded-full text-sm text-hoa-1400 hover:bg-hoa-600 active:bg-hoa-800 transition"
        @click="openDevtools"
      >
        <Bug />
      </div>
    </div>
    <div v-if="!loading" class="grow overflow-hidden bg-white dark:bg-neutral-950">
      <ChooseDirectory
        v-if="!init"
        :is-managing="isManagingDirectories"
        @progress-step="onProgressStep"
        @directories-changed="onDirectoriesChanged"
        @cancel="onDirectoryCancel"
      />
      <Library
        v-else
        :should-scan="shouldScan"
        @uninitialize-library="uninitializeLibrary"
        @manage-directories="manageDirectories"
        @scan-complete="onScanComplete"
      />
    </div>
  </div>

  <ModalsContainer />
</template>

<script setup>
import Bug from '~icons/mdi/bug'
import WindowMinimize from '~icons/mdi/window-minimize'
import WindowMaximize from '~icons/mdi/window-maximize'
import WindowClose from '~icons/mdi/window-close'
import ChooseDirectory from './components/ChooseDirectory.vue'
import Library from './components/Library.vue'
import { ref, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { invoke } from '@tauri-apps/api/core'
import { ModalsContainer } from 'vue-final-modal'
import { useGlobalState } from './composables/global-state'
import { useDownloader } from '@/composables/downloader.js'
import { useExporter } from '@/composables/export.js'
import { usePlayer } from '@/composables/player.js'
import { useToast } from 'vue-toastification'

const appWindow = getCurrentWebviewWindow()
const toast = useToast()
const { themeMode, setThemeMode, setLrclibInstance } = useGlobalState()
const { downloadNext } = useDownloader()
const { exportNext } = useExporter()
const { setVolume } = usePlayer()

const loading = ref(true)
const init = ref(false)
const shouldScan = ref(false)
const isProd = ref(import.meta.env.PROD)

const uninitializeLibrary = async () => {
  loading.value = true
  await invoke('uninitialize_library')
  init.value = await invoke('get_init')
  loading.value = false
}

// Remember whether the chooser was opened from "Manage directories" (which can
// be cancelled) or is the first-run setup (which cannot).
const isManagingDirectories = ref(false)

const manageDirectories = () => {
  // Show the directory chooser without clearing the database; the incremental
  // scan handles added or removed directories.
  isManagingDirectories.value = true
  init.value = false
}

const onDirectoryCancel = () => {
  // Nothing was written, so just go back to the library view.
  isManagingDirectories.value = false
  init.value = true
}

const onProgressStep = () => {
  isManagingDirectories.value = false
  init.value = true
}

const onDirectoriesChanged = () => {
  // Directories were changed, trigger a scan in Library
  shouldScan.value = true
}

const onScanComplete = () => {
  // Reset the scan flag after completion
  shouldScan.value = false
}

const { t } = useI18n()

onMounted(async () => {
  init.value = await invoke('get_init')
  loading.value = false
  await loadGlobalState()
  darkModeHandle(themeMode.value)
  downloadNext()
  exportNext()
  await startNotifications()
  checkForUpdates()
})

// A single startup check, off by default in the sense that the user can turn it
// off in About. Silent when there is nothing to say: an app that reports "you
// are up to date" on every launch is noise, and one that reports a network
// failure on boot is worse.
const checkForUpdates = async () => {
  try {
    const update = await invoke('check_update_on_startup')
    if (!update) return

    toast(
      `${t('about.updateAvailableVersion', { version: update.version })} ${update.url}`,
      { type: 'info', timeout: 12000 }
    )
    // Recorded so the same release is not announced again on the next launch.
    await invoke('mark_version_announced', { version: update.version })
  } catch (error) {
    console.info('Startup update check skipped:', error)
  }
}

const loadGlobalState = async () => {
  const config = await invoke('get_config')
  setThemeMode(config.theme_mode)
  setLrclibInstance(config.lrclib_instance)
  // Set initial volume from config (default to 1.0 if not set)
  const volume = config.volume !== undefined ? config.volume : 1.0
  setVolume(volume)
}

// Notifications arrive as backend events. The queue is only drained once at
// boot to pick up messages produced before this listener existed; polling it on
// a timer (the previous implementation hit the backend every 100ms forever)
// avoids nothing and costs an IPC round-trip 10 times a second.
const startNotifications = async () => {
  await listen('notification', event => {
    const notification = event.payload
    toast(notification.message, { type: notification.type })
  })

  const queued = await invoke('drain_notifications')
  queued.forEach(notification => {
    toast(notification.message, { type: notification.type })
  })
}

const darkModeHandle = async themeMode => {
  if (themeMode !== 'dark' && themeMode !== 'light') {
    themeMode = await appWindow.theme()
  }
  // check and insert the `dark` class to html tag
  if (themeMode === 'dark') {
    document.documentElement.classList.add('dark')
  } else {
    document.documentElement.classList.remove('dark')
  }
}

const openDevtools = () => {
  invoke('open_devtools')
}

const minimizeWindow = () => {
  appWindow.minimize()
}

const maximizeWindow = () => {
  appWindow.toggleMaximize()
}

const closeWindow = () => {
  appWindow.close()
}

appWindow.onThemeChanged(({ payload: theme }) => {
  if (themeMode.value === 'auto') {
    darkModeHandle(theme)
  }
})

watch(themeMode, () => {
  darkModeHandle(themeMode.value)
})
</script>

<style></style>

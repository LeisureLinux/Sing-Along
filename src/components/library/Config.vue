<template>
  <BaseModal
    :title="$t('config.title')"
    body-class="flex flex-col h-full justify-between overflow-y-auto"
    @before-open="beforeOpenHandler"
    @close="emit('close')"
  >
    <div class="flex flex-col gap-8">
      <div>
        <label class="group-label mb-4">{{ $t('config.appearance') }}</label>

        <div class="flex flex-col mb-4">
          <label class="block mb-2 child-label">{{ $t('config.theme') }}</label>

          <div class="flex gap-4">
            <RadioButton id="theme-auto" v-model="editingThemeMode" name="theme-mode" value="auto">
              {{ $t('config.themeAuto') }}
            </RadioButton>

            <RadioButton
              id="theme-light"
              v-model="editingThemeMode"
              name="theme-mode"
              value="light"
            >
              {{ $t('config.themeLight') }}
            </RadioButton>

            <RadioButton id="theme-dark" v-model="editingThemeMode" name="theme-mode" value="dark">
              {{ $t('config.themeDark') }}
            </RadioButton>
          </div>
        </div>

        <!-- Language. Applies immediately rather than on save: a language picker
             that only takes effect after clicking Save leaves the user staring at
             a UI they cannot read. -->
        <div class="flex flex-col mb-4">
          <label class="block mb-2 child-label" for="locale-select">{{
            $t('config.language')
          }}</label>
          <select
            id="locale-select"
            v-model="editingLocale"
            class="select px-2 h-8"
            @change="applyLocale"
          >
            <option value="auto">{{ $t('config.languageAuto') }}</option>
            <option v-for="option in locales" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </div>

        <div class="flex flex-col mb-4">
          <label class="block mb-2 child-label" for="lrclib-instance">{{
            $t('config.lrclibInstance')
          }}</label>
          <input
            id="lrclib-instance"
            v-model="editingLrclibInstance"
            type="text"
            placeholder="https://"
            class="input px-4 h-8"
          />
        </div>

        <!-- Explicit proxy for the lyric services. Empty means connect
             directly; an unset value would otherwise inherit HTTP_PROXY from
             whatever shell launched the app. -->
        <div class="flex flex-col">
          <label class="block mb-2 child-label" for="proxy-url">{{ $t('config.proxy') }}</label>
          <input
            id="proxy-url"
            v-model="editingProxyUrl"
            type="text"
            placeholder="http://127.0.0.1:8888"
            class="input px-4 h-8"
          />
          <span class="mt-1 text-xs text-neutral-500 dark:text-neutral-400">{{
            $t('config.proxyHint')
          }}</span>
        </div>

        <!-- Where the word-level lyrics database is read from. Empty means the
             built-in default; it is a setting because that database is one
             volunteer-maintained repository and any of its forks serves the same
             paths. -->
        <div class="flex flex-col mt-4">
          <label class="block mb-2 child-label" for="amll-db-url">{{
            $t('config.amllDbUrl')
          }}</label>
          <input
            id="amll-db-url"
            v-model="editingAmllDbUrl"
            type="text"
            placeholder="https://"
            class="input px-4 h-8"
          />
          <span class="mt-1 text-xs text-neutral-500 dark:text-neutral-400">{{
            $t('config.amllDbUrlHint')
          }}</span>
        </div>
      </div>

      <div>
        <label class="group-label mb-4">{{ $t('config.coverSync') }}</label>

        <div class="flex items-start">
          <CheckboxButton id="sync-cover" v-model="syncCover" name="sync-cover">
            <div class="flex flex-col">
              <span class="mb-0.5">{{ $t('config.syncCoverLabel') }}</span>
              <span class="text-xs text-neutral-500 dark:text-neutral-400">{{
                $t('config.syncCoverHint')
              }}</span>
              <span class="text-xs text-yellow-700 dark:text-yellow-400 mt-1">{{
                $t('config.fileWriteWarning')
              }}</span>
            </div>
          </CheckboxButton>
        </div>
      </div>

      <div class="flex flex-col gap-1">
        <a href="#" class="link hidden" @click="refreshLibrary">{{
          $t('config.scanIncremental')
        }}</a>
        <a href="#" class="link" @click="fullScanLibrary">{{ $t('config.scanFull') }}</a>
        <a href="#" class="link" @click="manageDirectories">{{ $t('config.directoriesHint') }}</a>
      </div>
    </div>

    <template #footer>
      <button class="button button-primary px-8 py-2 rounded-full" @click="save">
        {{ $t('actions.save') }}
      </button>
    </template>
  </BaseModal>
</template>

<script setup>
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import { useToast } from 'vue-toastification'
import { useGlobalState } from '../../composables/global-state'
import { AVAILABLE_LOCALES, readLocalePreference, setLocale } from '@/i18n/index.js'
import { usePlayer } from '@/composables/player.js'
import RadioButton from '@/components/common/RadioButton.vue'
import CheckboxButton from '@/components/common/CheckboxButton.vue'

const { setThemeMode, setLrclibInstance } = useGlobalState()
const toast = useToast()

// Language switching is applied on change, not on save, so the rest of the form
// is already in the newly chosen language while the user continues editing.
const locales = AVAILABLE_LOCALES
const editingLocale = ref(readLocalePreference())
const applyLocale = () => setLocale(editingLocale.value)
const { volume } = usePlayer()

const emit = defineEmits(['close', 'refreshLibrary', 'fullScanLibrary', 'manageDirectories'])

const syncCover = ref(false)
const editingThemeMode = ref('auto')
const editingLrclibInstance = ref('')
const editingProxyUrl = ref('')
const editingAmllDbUrl = ref('')

// Fields this dialog no longer edits (the per-batch skip rules and the
// experimental embed flag) still exist in config: the download dialog reads
// them. Holding the loaded values lets save() write them back unchanged
// instead of resetting them to a hardcoded default.
const preserved = ref({
  skipTracksWithSyncedLyrics: true,
  skipTracksWithPlainLyrics: false,
  showLineCount: true,
  tryEmbedLyrics: false,
  updateCheckEnabled: true,
})

const save = async () => {
  // A failure here used to be silent: the await threw, the function stopped,
  // and the dialog simply stayed open with the change not applied. Nothing on
  // screen said that anything had gone wrong.
  try {
    await invoke('set_config', {
      // Which tracks a batch should skip is chosen per download, in the download
      // dialog — it is not a global preference, so it is not edited here. The
      // loaded values are written back untouched.
      skipTracksWithSyncedLyrics: preserved.value.skipTracksWithSyncedLyrics,
      skipTracksWithPlainLyrics: preserved.value.skipTracksWithPlainLyrics,
      showLineCount: preserved.value.showLineCount,
      // Embedding lyrics into audio files is offered by the download dialog; this
      // dialog does not toggle it, so keep whatever is already stored.
      tryEmbedLyrics: preserved.value.tryEmbedLyrics,
      themeMode: editingThemeMode.value,
      lrclibInstance: editingLrclibInstance.value,
      volume: volume.value,
      syncCover: syncCover.value,
      proxyUrl: editingProxyUrl.value,
      amllDbUrl: editingAmllDbUrl.value,
      // The backend's `set_config` takes every setting, not only the ones this
      // dialog edits, so the ones it does not edit are written back untouched.
      // Leaving this one out made the whole call fail, and with it every change
      // in this dialog.
      updateCheckEnabled: preserved.value.updateCheckEnabled,
    })
  } catch (error) {
    console.error('Failed to save the settings:', error)
    toast.error($t('config.saveFailed'))
    return
  }
  setThemeMode(editingThemeMode.value)
  setLrclibInstance(editingLrclibInstance.value)
  emit('close')
}

const refreshLibrary = () => {
  emit('refreshLibrary')
  emit('close')
}

const fullScanLibrary = () => {
  emit('fullScanLibrary')
  emit('close')
}

const manageDirectories = () => {
  emit('manageDirectories')
  emit('close')
}

const beforeOpenHandler = async () => {
  const config = await invoke('get_config')
  preserved.value = {
    skipTracksWithSyncedLyrics: config.skip_tracks_with_synced_lyrics,
    skipTracksWithPlainLyrics: config.skip_tracks_with_plain_lyrics,
    showLineCount: config.show_line_count,
    tryEmbedLyrics: config.try_embed_lyrics,
    updateCheckEnabled: config.update_check_enabled ?? true,
  }
  syncCover.value = config.sync_cover ?? false
  editingThemeMode.value = config.theme_mode
  editingLrclibInstance.value = config.lrclib_instance
  editingProxyUrl.value = config.proxy_url ?? ''
  editingAmllDbUrl.value = config.amll_db_url ?? ''
}
</script>

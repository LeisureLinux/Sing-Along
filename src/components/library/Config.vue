<template>
  <BaseModal
    title="Configuration"
    body-class="flex flex-col h-full justify-between overflow-y-auto"
    @before-open="beforeOpenHandler"
    @close="emit('close')"
  >
    <div class="flex flex-col gap-8">
      <div>
        <label class="group-label mb-4">Common</label>

        <div class="flex flex-col mb-4">
          <label class="block mb-2 child-label">{{ $t('config.downloadLyricsFor') }}</label>

          <RadioButton
            id="download-lyrics-for-all"
            v-model="downloadLyricsFor"
            class="mb-1"
            name="download-lyrics-for"
            value="all"
          >
            All tracks (overwrite existing lyrics)
          </RadioButton>

          <RadioButton
            id="skip-synced"
            v-model="downloadLyricsFor"
            class="mb-1"
            name="download-lyrics-for"
            value="skipSynced"
          >
            Only tracks without synced lyrics
          </RadioButton>

          <RadioButton
            id="skip-plain"
            v-model="downloadLyricsFor"
            class="mb-1"
            name="download-lyrics-for"
            value="skipPlain"
          >
            Only tracks without any lyrics
          </RadioButton>
        </div>

        <!-- Total lines number should always show in search result, this configuration is not necessary -->
        <!-- <div class="flex flex-col mb-4">
          <label class="block mb-2 child-label">Search settings</label>

          <CheckboxButton id="show-line-count" v-model="showLineCount" name="show-line-count">
            Show the number of lines a lyric file has in the search menu
          </CheckboxButton>
        </div> -->

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

        <div class="flex flex-col">
          <label class="block mb-2 child-label" for="lrclib-instance">LRCLIB instance</label>
          <input
            id="lrclib-instance"
            v-model="editingLrclibInstance"
            type="text"
            placeholder="https://"
            class="input px-4 h-8"
          />
        </div>
      </div>

      <div>
        <label class="group-label mb-4">{{ $t('config.embedExperimental') }}</label>

        <div class="flex items-start">
          <CheckboxButton id="try-embed-lyrics" v-model="tryEmbedLyrics" name="try-embed-lyrics">
            <div class="flex flex-col">
              <span class="mb-0.5">Enable embed lyrics option</span>
              <span class="text-xs text-yellow-700 dark:text-yellow-400"
                >This option could corrupt your track files. Make sure to backup your library before
                enabling it.</span
              >
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
import { ref, watch } from 'vue'
import { useGlobalState } from '../../composables/global-state'
import { AVAILABLE_LOCALES, readLocalePreference, setLocale } from '@/i18n/index.js'
import { usePlayer } from '@/composables/player.js'
import RadioButton from '@/components/common/RadioButton.vue'
import CheckboxButton from '@/components/common/CheckboxButton.vue'

const { setThemeMode, setLrclibInstance } = useGlobalState()

// Language switching is applied on change, not on save, so the rest of the form
// is already in the newly chosen language while the user continues editing.
const locales = AVAILABLE_LOCALES
const editingLocale = ref(readLocalePreference())
const applyLocale = () => setLocale(editingLocale.value)
const { volume } = usePlayer()

const emit = defineEmits(['close', 'refreshLibrary', 'fullScanLibrary', 'manageDirectories'])

const downloadLyricsFor = ref('all')
const skipTracksWithSyncedLyrics = ref(true)
const skipTracksWithPlainLyrics = ref(false)
const showLineCount = ref(true)
const tryEmbedLyrics = ref(false)
const editingThemeMode = ref('auto')
const editingLrclibInstance = ref('')

const save = async () => {
  await invoke('set_config', {
    skipTracksWithSyncedLyrics: skipTracksWithSyncedLyrics.value,
    skipTracksWithPlainLyrics: skipTracksWithPlainLyrics.value,
    showLineCount: showLineCount.value,
    tryEmbedLyrics: tryEmbedLyrics.value,
    themeMode: editingThemeMode.value,
    lrclibInstance: editingLrclibInstance.value,
    volume: volume.value,
  })
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
  skipTracksWithSyncedLyrics.value = config.skip_tracks_with_synced_lyrics
  skipTracksWithPlainLyrics.value = config.skip_tracks_with_plain_lyrics

  console.log(skipTracksWithSyncedLyrics.value, skipTracksWithPlainLyrics.value)

  if (skipTracksWithSyncedLyrics.value && !skipTracksWithPlainLyrics.value) {
    downloadLyricsFor.value = 'skipSynced'
  } else if (skipTracksWithPlainLyrics.value) {
    downloadLyricsFor.value = 'skipPlain'
  } else {
    downloadLyricsFor.value = 'all'
  }

  showLineCount.value = config.show_line_count
  tryEmbedLyrics.value = config.try_embed_lyrics
  editingThemeMode.value = config.theme_mode
  editingLrclibInstance.value = config.lrclib_instance
}

watch(downloadLyricsFor, newVal => {
  if (newVal === 'skipSynced') {
    skipTracksWithSyncedLyrics.value = true
    skipTracksWithPlainLyrics.value = false
  } else if (newVal === 'skipPlain') {
    skipTracksWithSyncedLyrics.value = true
    skipTracksWithPlainLyrics.value = true
  } else {
    skipTracksWithSyncedLyrics.value = false
    skipTracksWithPlainLyrics.value = false
  }
})
</script>

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
        <label class="group-label mb-4">{{ $t('youtube.downloader') }}</label>

        <!-- The downloader. Everything here is about the user's own machine:
             where their yt-dlp is, which cookies it should use, and where the
             files should land. -->
        <div class="flex flex-col mb-4">
          <label class="block mb-2 child-label" for="ytdlp-binary">{{
            $t('youtube.binary')
          }}</label>
          <input
            id="ytdlp-binary"
            v-model="editingYtdlpBinary"
            type="text"
            :placeholder="$t('youtube.binaryPlaceholder')"
            class="input px-4 h-8"
          />
          <span class="mt-1 text-xs text-neutral-500 dark:text-neutral-400">{{
            $t('youtube.binaryHint')
          }}</span>
        </div>

        <div class="flex flex-col mb-4">
          <label class="block mb-2 child-label" for="ytdlp-cookies">{{
            $t('youtube.cookies')
          }}</label>
          <div class="flex gap-2">
            <input
              id="ytdlp-cookies"
              v-model="editingYtdlpCookies"
              type="text"
              :placeholder="$t('youtube.cookiesPlaceholder')"
              class="input px-4 h-8 grow"
            />
            <button class="button button-normal px-3 py-1 text-xs rounded" @click="chooseCookieFile">
              {{ $t('youtube.browse') }}
            </button>
          </div>
          <span class="mt-1 text-xs text-neutral-500 dark:text-neutral-400">{{
            $t('youtube.cookiesHint')
          }}</span>
          <span class="mt-1 text-xs text-neutral-500 dark:text-neutral-400">{{
            $t('youtube.cookiesHowTo')
          }}</span>
          <span
            v-if="ytdlpStatus && ytdlpStatus.cookiesFound"
            class="mt-1 text-xs"
            :class="
              ytdlpStatus.cookiesExpired > 0
                ? 'text-yellow-700 dark:text-yellow-400'
                : 'text-neutral-500 dark:text-neutral-400'
            "
          >
            {{ cookieSummary }}
          </span>
        </div>

        <div v-if="ytdlpStatus && ytdlpStatus.problem" class="mb-4">
          <span class="text-xs text-yellow-700 dark:text-yellow-400">{{
            ytdlpStatus.problem
          }}</span>
        </div>

        <!-- The proxy. It is set in the panel above, and the download uses it:
             in some countries YouTube is unreachable without one, and the
             failure looks like a broken downloader rather than a missing
             route. -->
        <div class="flex flex-col mb-4">
          <span class="text-xs text-neutral-500 dark:text-neutral-400">{{
            $t('youtube.proxyHint')
          }}</span>
        </div>

        <div class="flex flex-col mb-4">
          <label class="block mb-2 child-label" for="ytdlp-output-dir">{{
            $t('youtube.outputDir')
          }}</label>
          <div class="flex gap-2">
            <input
              id="ytdlp-output-dir"
              v-model="editingYtdlpOutputDir"
              type="text"
              :placeholder="$t('youtube.outputDirPlaceholder')"
              class="input px-4 h-8 grow"
            />
            <button class="button button-normal px-3 py-1 text-xs rounded" @click="chooseOutputDir">
              {{ $t('youtube.browse') }}
            </button>
          </div>
          <span class="mt-1 text-xs text-neutral-500 dark:text-neutral-400">{{
            $t('youtube.outputDirHint')
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
import { computed, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import { useGlobalState } from '../../composables/global-state'
import { AVAILABLE_LOCALES, readLocalePreference, setLocale } from '@/i18n/index.js'
import { usePlayer } from '@/composables/player.js'
import RadioButton from '@/components/common/RadioButton.vue'
import CheckboxButton from '@/components/common/CheckboxButton.vue'

const { setThemeMode, setLrclibInstance } = useGlobalState()
const toast = useToast()
const { t } = useI18n()

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
const editingYtdlpBinary = ref('')
const editingYtdlpCookies = ref('')
const editingYtdlpOutputDir = ref('')

// What the downloader can and cannot do right now, so that a missing
// yt-dlp or an expired cookie is reported here rather than as a failed
// download later.
const ytdlpStatus = ref(null)

const cookieSummary = computed(() => {
  const status = ytdlpStatus.value
  if (!status) {
    return ''
  }
  const parts = [t('youtube.cookieCount', { count: status.cookieCount })]
  if (status.cookiesExpired > 0) {
    parts.push(t('youtube.cookieExpired', { count: status.cookiesExpired }))
  }
  if (!status.cookiesSignedIn) {
    parts.push(t('youtube.cookieNotSignedIn'))
  }
  return parts.join(' · ')
})

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
      ytdlpBinary: editingYtdlpBinary.value,
      ytdlpCookiesJson: editingYtdlpCookies.value,
      ytdlpOutputDir: editingYtdlpOutputDir.value,
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

const refreshYtdlpStatus = async () => {
  try {
    ytdlpStatus.value = await invoke('get_ytdlp_status')
  } catch (error) {
    console.error('Failed to read the downloader status:', error)
    ytdlpStatus.value = null
  }
}

// Picking the cookie export.
//
// The JSON the browser extension writes is picked directly, and converted
// right away: the user should not have to run a second program to turn it
// into something yt-dlp can read.
const chooseCookieFile = async () => {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: 'Cookie export', extensions: ['json'] }],
  })

  if (!selected || typeof selected !== 'string') {
    return
  }

  editingYtdlpCookies.value = selected

  try {
    const status = await invoke('convert_cookie_export', { jsonPath: selected })
    toast.success(t('youtube.cookiesConverted', { count: status.count }))
  } catch (error) {
    console.error('Failed to convert the cookie export:', error)
    toast.error(String(error))
  }

  await refreshYtdlpStatus()
}

const chooseOutputDir = async () => {
  const selected = await open({
    directory: true,
    recursive: false,
  })

  if (selected && typeof selected === 'string') {
    editingYtdlpOutputDir.value = selected
  }
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
  editingYtdlpBinary.value = config.ytdlp_binary ?? ''
  editingYtdlpCookies.value = config.ytdlp_cookies_json ?? ''
  editingYtdlpOutputDir.value = config.ytdlp_output_dir ?? ''

  // The status is read after the values are loaded, so that it describes what
  // is on screen.
  await refreshYtdlpStatus()
}
</script>

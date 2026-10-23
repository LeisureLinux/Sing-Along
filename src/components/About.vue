<template>
  <BaseModal :title="$t('about.title')" @opened="openedHandler" @close="emit('close')">
    <div class="overflow-auto">
      <div class="mb-6">
        <label class="group-label mb-2">{{ $t('about.update') }}</label>

        <div class="text-sm">
          <div class="mb-2">
            {{ $t('about.currentVersion') }}: <span class="font-bold">{{ version }}</span>
          </div>

          <!-- Nothing is fetched until the dialog is opened and the user either
               has the setting on or presses the button. Opening About must not
               by itself make a network call. -->
          <div class="flex flex-wrap items-center gap-2 mb-2">
            <button
              class="button button-normal px-3 py-1 text-xs rounded-full inline-flex items-center gap-1"
              :disabled="isCheckingForUpdate"
              @click="checkNow"
            >
              <Loading v-if="isCheckingForUpdate" class="animate-spin text-sm" />
              <Refresh v-else class="text-sm" />
              {{ isCheckingForUpdate ? $t('about.checking') : $t('about.checkNow') }}
            </button>

            <label class="flex items-center gap-1 cursor-pointer select-none">
              <CheckboxButton
                id="update-check-on-startup"
                :model-value="updateCheckEnabled"
                name="update-check-on-startup"
                @update:model-value="setUpdateCheckEnabled"
              />
              <span class="text-xs">{{ $t('about.checkOnStartup') }}</span>
            </label>
          </div>

          <template v-if="!isCheckingForUpdate && checked">
            <div v-if="isUpdateAvailable" class="font-bold text-yellow-600 flex flex-wrap gap-1 items-center">
              <Alert class="text-lg" />
              <span>{{ $t('about.updateAvailableVersion', { version: newestVersion }) }}</span>
              <a :href="newestReleaseUrl" class="link" target="_blank" @click="downloadUpdate">{{
                $t('about.releasePage')
              }}</a>
            </div>
            <div v-else-if="newestVersion" class="font-bold text-green-600 flex flex-wrap gap-1 items-center">
              <CheckCircle class="text-lg" />
              {{ $t('about.upToDate') }}
            </div>
            <div v-else>{{ $t('about.updateFailed') }}</div>
          </template>
        </div>
      </div>

      <div>
        <label class="group-label mb-2">{{ $t('about.homePage') }}</label>

        <div class="text-sm">
          <div class="mb-1">
            Github URL:
            <a href="https://github.com/LeisureLinux/Sing-Along" class="link" target="_blank"
              >github.com/LeisureLinux/Sing-Along</a
            >
          </div>

          <div>
            Email:
            <a href="mailto:albertxu@freelamp.com" class="link" target="_blank"
              >albertxu@freelamp.com</a
            >
          </div>
        </div>
      </div>

      <!-- Command line and agent skill. Both ship with the package, but nothing
           in the GUI tells you they exist, so the About dialog is where someone
           would look. -->
      <div class="mt-6">
        <label class="group-label mb-2">{{ $t('about.commandLine') }}</label>

        <div class="text-sm">
          <div class="mb-2">{{ $t('about.commandLineHint') }}</div>
          <pre
            class="px-2 py-1.5 rounded bg-neutral-100 dark:bg-neutral-800 text-xs overflow-x-auto select-text"
          >sing-along fetch ~/Music
sing-along export ~/Music --for steve
sing-along status ~/Music</pre>
          <div class="mt-2 text-xs text-neutral-500 dark:text-neutral-400">
            {{ $t('about.manPageHint') }}
            <code class="select-text">man sing-along</code>
          </div>

          <div class="mt-3">{{ $t('about.skillHint') }}</div>
          <pre
            class="mt-1 px-2 py-1.5 rounded bg-neutral-100 dark:bg-neutral-800 text-xs overflow-x-auto select-text"
          >/usr/share/sing-along/skills/sing-along/SKILL.md</pre>
        </div>
      </div>

      <div class="mt-6">
        <label class="group-label mb-2">{{ $t('about.credits') }}</label>

        <div class="text-sm">
          <!-- The copyright line itself stays in English: it is the legal notice
               required by the MIT license, and translating it would change what
               the license asks to be reproduced. Only the surrounding sentence
               is localised. -->
          <div class="mb-2">
            {{ $t('about.forkOf') }}
            <a href="https://github.com/tranxuanthang/lrcget" class="link" target="_blank">LRCGET</a
            >{{ $t('about.copyrightSuffix') }}
          </div>
          <div class="mb-2">
            {{ $t('about.contributors') }}: Victor Seiji Hariki, M23SNEZ3, SollyBunny, Micah Sze,
            Guilherme Silva{{ $t('about.contributorsSuffix') }}
          </div>
          <div>
            {{ $t('about.lyricsFrom') }}
            <a href="https://lrclib.net" class="link" target="_blank">LRCLIB</a>.
          </div>
        </div>
      </div>

      <div class="mt-6">
        <label class="group-label mb-2">{{ $t('about.donate') }}</label>

        <div class="text-sm text-neutral-800 dark:text-neutral-300">
          <!-- Both lines are shown in every UI language: the payment app is
               Chinese, so an English-only reader still needs to recognise what
               the code is for, and a Chinese reader should not have to infer it
               from a translation of an English sentence. -->
          <div class="text-center mb-3 leading-relaxed">
            <div class="font-bold">Thanks for your donation!</div>
            <div class="font-bold">感谢您的赞赏！</div>
          </div>
          <img
            :src="donateQr"
            :alt="$t('about.donateAlt')"
            class="w-56 mx-auto"
            draggable="false"
          />
        </div>
      </div>
    </div>
  </BaseModal>
</template>

<script setup>
import Alert from '~icons/mdi/alert'
import CheckCircle from '~icons/mdi/check-circle'
import Loading from '~icons/mdi/loading'
import Refresh from '~icons/mdi/refresh'
import CheckboxButton from '@/components/common/CheckboxButton.vue'
import { invoke } from '@tauri-apps/api/core'
import { ref, computed } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import semver from 'semver'
import BaseModal from '@/components/common/BaseModal.vue'
import donateQr from '@/assets/donate-qr.png'

const emit = defineEmits(['close'])

const version = ref(null)
const newestVersion = ref(null)
const newestReleaseUrl = ref(null)
const isUpdateAvailable = computed(() => {
  if (!version.value || !newestVersion.value) {
    return false
  }
  return semver.gt(newestVersion.value, version.value)
})
// Starts false: nothing is being checked until the user presses the button.
// Defaulting to true made the dialog render the "checking" state on open, which
// read as an automatic fetch that the user had not asked for.
const isCheckingForUpdate = ref(false)

async function getLatestReleaseInfo() {
  const repo = 'LeisureLinux/Sing-Along'
  const apiUrl = `https://api.github.com/repos/${repo}/releases/latest`

  const response = await fetch(apiUrl)
  if (!response.ok) {
    throw new Error(`Error: ${response.status}`)
  }
  const data = await response.json()
  return {
    version: data.tag_name,
    releaseUrl: data.html_url,
  }
}

// Whether the user has asked for a check in this session. Distinguishes "not
// checked yet" from "checked and found nothing".
const checked = ref(false)
const updateCheckEnabled = ref(true)

const runCheck = async () => {
  isCheckingForUpdate.value = true
  newestVersion.value = null
  try {
    const latest = await getLatestReleaseInfo()
    newestVersion.value = latest.version
    newestReleaseUrl.value = latest.releaseUrl
  } catch (error) {
    // A failed check is normal (offline, rate-limited) and already shows as
    // "could not check" — logging it as an error would suggest a bug.
    console.info('Update check failed:', error)
    newestVersion.value = null
  } finally {
    isCheckingForUpdate.value = false
    checked.value = true
  }
}

const checkNow = () => runCheck()

const setUpdateCheckEnabled = async enabled => {
  updateCheckEnabled.value = enabled
  const config = await invoke('get_config')
  await invoke('set_config', {
    skipTracksWithSyncedLyrics: config.skip_tracks_with_synced_lyrics,
    skipTracksWithPlainLyrics: config.skip_tracks_with_plain_lyrics,
    showLineCount: config.show_line_count,
    tryEmbedLyrics: config.try_embed_lyrics,
    themeMode: config.theme_mode,
    lrclibInstance: config.lrclib_instance,
    volume: config.volume,
    syncCover: config.sync_cover,
    proxyUrl: config.proxy_url ?? '',
    updateCheckEnabled: enabled,
    amllDbUrl: config.amll_db_url ?? '',
  })
}

// Opening About reads the stored preference but does not fetch anything on its
// own: the previous behaviour checked GitHub every time the dialog opened, which
// is a network call the user never asked for.
const openedHandler = async () => {
  version.value = await getVersion()
  checked.value = false
  newestVersion.value = null
  try {
    const config = await invoke('get_config')
    updateCheckEnabled.value = config.update_check_enabled ?? true
  } catch (error) {
    console.error(error)
  }
}
</script>

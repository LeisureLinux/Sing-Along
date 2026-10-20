<template>
  <BaseModal :title="$t('about.title')" @opened="openedHandler" @close="emit('close')">
    <div class="overflow-auto">
      <div class="mb-6">
        <label class="group-label mb-2">{{ $t('about.update') }}</label>

        <div class="text-sm">
          <div class="mb-1">
            {{ $t('about.currentVersion') }}: <span class="font-bold">{{ version }}</span>
          </div>
          <template v-if="!isCheckingForUpdate && newestVersion">
            <div class="mb-1">
              {{ $t('about.newestVersion') }}: <span class="font-bold">{{ newestVersion }}</span>
            </div>
            <div
              v-if="isUpdateAvailable"
              class="font-bold text-yellow-600 flex flex-wrap gap-1 items-center"
            >
              <Alert class="text-lg" />
              {{ $t('about.updateAvailable') }}
              <span
                ><a :href="newestReleaseUrl" class="link" target="_blank" @click="downloadUpdate">{{
                  $t('about.releasePage')
                }}</a
                >!</span
              >
            </div>
            <div v-else class="font-bold text-green-600 flex flex-wrap gap-1 items-center">
              <CheckCircle class="text-lg" />
              {{ $t('about.upToDate') }}
            </div>
          </template>
          <div v-else-if="!isCheckingForUpdate && !newestVersion">
            {{ $t('about.updateFailed') }}
          </div>
          <div v-else>{{ $t('about.checking') }}</div>
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
const isCheckingForUpdate = ref(true)

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

const openedHandler = async () => {
  version.value = await getVersion()
  isCheckingForUpdate.value = true
  try {
    const latestReleaseInfo = await getLatestReleaseInfo()
    newestVersion.value = latestReleaseInfo.version
    newestReleaseUrl.value = latestReleaseInfo.releaseUrl
  } catch (error) {
    console.error(error)
  } finally {
    isCheckingForUpdate.value = false
  }
}
</script>

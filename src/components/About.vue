<template>
  <BaseModal title="About" @opened="openedHandler" @close="emit('close')">
    <div class="overflow-auto">
      <div class="mb-6">
        <label class="group-label mb-2">Update</label>

        <div class="text-sm">
          <div class="mb-1">
            Current version: <span class="font-bold">{{ version }}</span>
          </div>
          <template v-if="!isCheckingForUpdate && newestVersion">
            <div class="mb-1">
              Newest version: <span class="font-bold">{{ newestVersion }}</span>
            </div>
            <div
              v-if="isUpdateAvailable"
              class="font-bold text-yellow-600 flex flex-wrap gap-1 items-center"
            >
              <Alert class="text-lg" />
              New update is available. See what's new and download at
              <span
                ><a :href="newestReleaseUrl" class="link" target="_blank" @click="downloadUpdate"
                  >Sing-Along release page</a
                >!</span
              >
            </div>
            <div v-else class="font-bold text-green-600 flex flex-wrap gap-1 items-center">
              <CheckCircle class="text-lg" />
              You are using newest version ;-)
            </div>
          </template>
          <div v-else-if="!isCheckingForUpdate && !newestVersion">
            Failed to check update from GitHub.
          </div>
          <div v-else>Fetching newest version data from GitHub...</div>
        </div>
      </div>

      <div>
        <label class="group-label mb-2">Home page, contact and support</label>

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

      <div class="mt-6">
        <label class="group-label mb-2">Credits and license</label>

        <div class="text-sm">
          <div class="mb-2">
            Sing-Along is a fork of
            <a href="https://github.com/tranxuanthang/lrcget" class="link" target="_blank">LRCGET</a
            >, Copyright (c) 2023 tranxuanthang, used under the MIT License.
          </div>
          <div class="mb-2">
            Previously contributed code by Victor Seiji Hariki, M23SNEZ3, SollyBunny, Micah Sze and
            Guilherme Silva, also under the MIT License.
          </div>
          <div>
            Lyrics are provided by
            <a href="https://lrclib.net" class="link" target="_blank">LRCLIB</a>.
          </div>
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

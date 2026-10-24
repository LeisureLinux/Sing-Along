<template>
  <BaseModal
    :title="$t('youtube.title')"
    content-class="w-full h-[80vh] max-w-screen-sm"
    background="non-interactive"
    :lock-scroll="true"
    @close="emit('close')"
  >
    <div class="grow overflow-hidden flex flex-col gap-6">
      <form class="flex flex-col flex-none gap-4" @submit.prevent="probe">
        <div>
          <label for="youtube-url" class="group-label mb-1">{{ $t('youtube.url') }}</label>
          <input
            id="youtube-url"
            v-model="url"
            type="text"
            class="input w-full py-1.5 px-2"
            :placeholder="$t('youtube.urlPlaceholder')"
            :disabled="busy"
          />
        </div>

        <div class="flex justify-center gap-2">
          <button
            class="button rounded-full text-xs px-6 py-2"
            :class="{ 'button-disabled': busy || !url, 'button-primary': !busy && url }"
            :disabled="busy || !url"
            type="submit"
          >
            {{ $t('youtube.lookUp') }}
          </button>
        </div>
      </form>

      <div class="grow overflow-hidden">
        <div v-if="busy" class="flex justify-center items-center h-full">
          <Loading class="animate-spin text-xl text-neutral-800 dark:text-neutral-400" />
        </div>

        <div v-else-if="problem" class="flex flex-col gap-3 justify-center items-center h-full text-sm">
          <div class="text-red-700 dark:text-red-400 text-center break-words">{{ problem }}</div>
          <button class="button button-normal px-4 py-1.5 text-xs rounded-full" @click="probe">
            {{ $t('actions.retry') }}
          </button>
        </div>

        <div v-else-if="info" class="flex flex-col gap-4 h-full overflow-auto">
          <div class="rounded bg-white dark:bg-neutral-900 border border-neutral-200 dark:border-neutral-800 px-3 py-2">
            <div class="font-bold text-neutral-800 dark:text-neutral-200">{{ info.title }}</div>
            <div class="text-sm text-neutral-900 dark:text-neutral-400">
              <span>{{ info.artist || $t('youtube.unknownArtist') }}</span>
            </div>
            <div class="text-sm text-neutral-900 dark:text-neutral-400">
              <span>{{ $t('youtube.album') }}: </span>
              <span>{{ info.correctedAlbum || info.album || $t('youtube.unknownAlbum') }}</span>
            </div>
            <div
              v-if="info.correctedAlbum && info.correctedAlbum !== info.album"
              class="text-xs text-neutral-500 dark:text-neutral-400"
            >
              {{ $t('youtube.albumCorrected') }}
            </div>
          </div>

          <div class="text-xs text-neutral-500 dark:text-neutral-400">
            {{ $t('youtube.downloadExplanation') }}
          </div>

          <div class="flex justify-center">
            <button
              class="button rounded-full text-xs px-6 py-2"
              :class="{ 'button-disabled': downloading, 'button-primary': !downloading }"
              :disabled="downloading"
              @click="download"
            >
              {{ downloading ? $t('youtube.downloading') : $t('youtube.download') }}
            </button>
          </div>
        </div>

        <div v-else class="flex justify-center items-center h-full text-sm text-neutral-400">
          {{ $t('youtube.hint') }}
        </div>
      </div>
    </div>
  </BaseModal>
</template>

<script setup>
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import Loading from '~icons/mdi/loading'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import BaseModal from '@/components/common/BaseModal.vue'

const emit = defineEmits(['close'])

const { t } = useI18n()
const toast = useToast()

const url = ref('')
const info = ref(null)
const problem = ref('')
const busy = ref(false)
const downloading = ref(false)

const probe = async () => {
  busy.value = true
  problem.value = ''
  info.value = null
  try {
    info.value = await invoke('probe_youtube_track', { url: url.value.trim() })
  } catch (error) {
    console.error(error)
    problem.value = String(error)
  } finally {
    busy.value = false
  }
}

const download = async () => {
  downloading.value = true
  try {
    const result = await invoke('download_youtube_track', { url: url.value.trim() })
    toast.success(
      t('youtube.downloaded', {
        title: result.title,
        album: result.album || t('youtube.unknownAlbum'),
      })
    )
    emit('close')
  } catch (error) {
    console.error(error)
    toast.error(String(error))
  } finally {
    downloading.value = false
  }
}
</script>

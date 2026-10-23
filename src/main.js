import { createApp } from 'vue'
import App from './App.vue'
import Toast, { POSITION } from 'vue-toastification'
import { createVfm } from 'vue-final-modal'
import FloatingVue from 'floating-vue'

import 'vue-toastification/dist/index.css'
import 'vue-final-modal/style.css'
import 'floating-vue/dist/style.css'
import './style.css'

import { VueFinalModal } from 'vue-final-modal'
import BaseModal from '@/components/common/BaseModal.vue'
import i18n, { initLocale } from './i18n/index.js'

const vfm = createVfm()

const app = createApp(App)
  .component('VueFinalModal', VueFinalModal)
  .component('BaseModal', BaseModal)

app.use(Toast, {
  position: POSITION.BOTTOM_RIGHT,
  timeout: 5000,
  transition: 'Vue-Toastification__fade',
  toastClassName: 'singalong-toast',
  bodyClassName: ['toast-body-1', 'toast-body-2'],
  hideProgressBar: true,
  closeButton: false,
  draggablePercent: 0.4,
  closeOnClick: false,
})
app.use(FloatingVue, {
  themes: {
    'singalong-tooltip': {
      $extend: 'tooltip',
      delay: {
        show: 50,
        hide: 50,
      },
    },
    'singalong-dropdown': {
      triggers: ['click'],
      autoHide: true,
      placement: 'bottom',
    },
  },
})
app.use(i18n)
initLocale()
app.use(vfm)

// Mount with a visible failure.
//
// A mount that throws leaves an empty `#app` and a blank window: nothing on
// screen says the application failed to start, and nothing in the window can be
// used to find out why. That is what a bad stored locale used to produce —
// a white window, no message, no clue.
//
// Rendering the error instead turns a silent blank page into something the user
// can screenshot and report, and gives them a way back: the button clears the
// stored preferences, which is where the bad value lives.
try {
  app.mount('#app')
} catch (error) {
  console.error('Failed to start:', error)
  const root = document.getElementById('app')
  if (root) {
    root.innerHTML = `
      <div style="font: 14px/1.6 system-ui, sans-serif; color: #1f2328; padding: 32px; max-width: 640px; margin: 0 auto;">
        <h1 style="font-size: 18px; margin: 0 0 12px;">Sing-Along could not start</h1>
        <p style="margin: 0 0 16px; color: #59636e;">
          Something in the saved settings stopped the interface from loading.
        </p>
        <pre style="background: #f6f8fa; border: 1px solid #d1d9e0; border-radius: 6px; padding: 12px; overflow: auto; font-size: 12px;">${String(
          error
        ).replace(/[<>&]/g, c => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' })[c])}</pre>
        <button id="reset-settings" style="margin-top: 16px; padding: 8px 16px; border-radius: 6px; border: 1px solid #d1d9e0; background: #0969da; color: #fff; font: inherit; cursor: pointer;">
          Reset the saved settings and reload
        </button>
        <p style="margin: 16px 0 0; color: #59636e; font-size: 12px;">
          This clears the remembered language and last track. Your library and lyrics are not touched.
        </p>
      </div>
    `
    document.getElementById('reset-settings')?.addEventListener('click', () => {
      for (const key of Object.keys(localStorage)) {
        if (key.startsWith('sing-along.')) localStorage.removeItem(key)
      }
      location.reload()
    })
  }
}

document.addEventListener(
  'contextmenu',
  event => {
    event.preventDefault()
    return false
  },
  { capture: true }
)

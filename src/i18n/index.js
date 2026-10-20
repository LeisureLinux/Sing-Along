import { createI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import en from './locales/en.js'
import zhHans from './locales/zh-Hans.js'
import zhHant from './locales/zh-Hant.js'

// Persisted choice; 'auto' follows the system language.
const STORAGE_KEY = 'sing-along.locale'

/** Locales the UI ships with. Order is the order shown in the picker. */
export const AVAILABLE_LOCALES = [
  { value: 'en', label: 'English' },
  { value: 'zh-Hans', label: '简体中文' },
  { value: 'zh-Hant', label: '繁體中文' },
]

/**
 * Resolve the locale to use.
 *
 * 'auto' (the default) reads `navigator.languages` and falls back to English.
 * Chinese needs care here: the same tag family covers Simplified and
 * Traditional, and the distinguishing part is the script or region subtag, so
 * `zh-TW`/`zh-HK`/`zh-MO` and anything with `Hant` map to Traditional while
 * `zh-CN`/`zh-SG`/plain `zh` map to Simplified.
 */
export function resolveLocale(preference) {
  if (preference && preference !== 'auto') {
    return preference
  }

  const candidates = navigator.languages?.length
    ? navigator.languages
    : [navigator.language || 'en']

  for (const tag of candidates) {
    const lower = tag.toLowerCase()
    if (lower.startsWith('zh')) {
      if (/hant|tw|hk|mo/.test(lower)) {
        return 'zh-Hant'
      }
      return 'zh-Hans'
    }
    if (lower.startsWith('en')) {
      return 'en'
    }
  }
  return 'en'
}

export function readLocalePreference() {
  return localStorage.getItem(STORAGE_KEY) || 'auto'
}

export function storeLocalePreference(preference) {
  if (preference === 'auto') {
    localStorage.removeItem(STORAGE_KEY)
  } else {
    localStorage.setItem(STORAGE_KEY, preference)
  }
}

// No locale is read at import time: `readLocalePreference` touches
// localStorage, which does not exist in Node test environments, and any module
// that imports this one would break. Instead the module starts in English and
// `initLocale()` is called from the app entry point, after the DOM is available.
const i18n = createI18n({
  legacy: false,
  globalInjection: true,
  fallbackLocale: 'en',
  messages: {
    en,
    'zh-Hans': zhHans,
    'zh-Hant': zhHant,
  },
})

/** Read the stored preference and set the locale. Called once from the entry point. */
export function initLocale() {
  i18n.global.locale.value = resolveLocale(readLocalePreference())
  document.documentElement.setAttribute('lang', i18n.global.locale.value)
  applyWindowTitle()
}

/**
 * Keep the OS window title in the UI language.
 *
 * The title is set from tauri.conf.json at launch, which is fixed English. The
 * window manager reads that, not the HTML <title>, so switching language has to
 * write it through the Tauri window API as well.
 */
function applyWindowTitle() {
  const title = i18n.global.t('app.windowTitle')
  document.title = title
  // Static import, not `await import(...)`: Vite does not resolve a bare
  // specifier inside a dynamic import here, so the call failed and the window
  // manager kept showing the title from tauri.conf.json. A rejected promise is
  // swallowed because tests run outside Tauri, where there is no window API.
  getCurrentWindow()
    .setTitle(title)
    .catch(() => {})
}

/** Switch language and remember the choice. */
export function setLocale(preference) {
  storeLocalePreference(preference)
  setLocaleValue(preference)
}

function setLocaleValue(preference) {
  i18n.global.locale.value = resolveLocale(preference)
  document.documentElement.setAttribute('lang', i18n.global.locale.value)
  applyWindowTitle()
}

export default i18n

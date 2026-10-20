import { createI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import en from './locales/en.js'
import zhHans from './locales/zh-Hans.js'
import zhHant from './locales/zh-Hant.js'
import ja from './locales/ja.js'
import ko from './locales/ko.js'
import es from './locales/es.js'
import fr from './locales/fr.js'
import de from './locales/de.js'
import vi from './locales/vi.js'
import th from './locales/th.js'
import ru from './locales/ru.js'

// Persisted choice; 'auto' follows the system language.
const STORAGE_KEY = 'sing-along.locale'

/** Locales the UI ships with. Order is the order shown in the picker. */
export const AVAILABLE_LOCALES = [
  { value: 'en', label: 'English' },
  { value: 'zh-Hans', label: '简体中文' },
  { value: 'zh-Hant', label: '繁體中文' },
  { value: 'ja', label: '日本語' },
  { value: 'ko', label: '한국어' },
  { value: 'es', label: 'Español' },
  { value: 'fr', label: 'Français' },
  { value: 'de', label: 'Deutsch' },
  { value: 'vi', label: 'Tiếng Việt' },
  { value: 'th', label: 'ไทย' },
  { value: 'ru', label: 'Русский' },
]

/**
 * Map a BCP 47 tag onto one of the shipped locales, or null if none match.
 *
 * Chinese needs care: the same tag family covers Simplified and Traditional, and
 * the distinguishing part is the script or region subtag, so
 * `zh-TW`/`zh-HK`/`zh-MO` and anything with `Hant` map to Traditional while
 * `zh-CN`/`zh-SG`/plain `zh` map to Simplified. Every other language maps on its
 * primary subtag alone, which is what `navigator.languages` gives us.
 */
const matchLocale = tag => {
  const lower = tag.toLowerCase()
  const primary = lower.split('-')[0]

  if (primary === 'zh') {
    return /hant|tw|hk|mo/.test(lower) ? 'zh-Hant' : 'zh-Hans'
  }

  return AVAILABLE_LOCALES.some(option => option.value === primary) ? primary : null
}

/**
 * Resolve the locale to use.
 *
 * 'auto' (the default) walks `navigator.languages` and takes the first tag that
 * matches a shipped locale, so a user with `['fr-CA', 'en']` gets French rather
 * than English. Falls back to English when nothing matches.
 */
export function resolveLocale(preference) {
  if (preference && preference !== 'auto') {
    return preference
  }

  const candidates = navigator.languages?.length
    ? navigator.languages
    : [navigator.language || 'en']

  for (const tag of candidates) {
    const matched = matchLocale(tag)
    if (matched) {
      return matched
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
    ja,
    ko,
    es,
    fr,
    de,
    vi,
    th,
    ru,
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

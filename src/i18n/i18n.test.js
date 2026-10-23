import { beforeEach, describe, expect, it } from 'vitest'
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
import ptBR from './locales/pt-BR.js'
import itLocale from './locales/it.js'
import tr from './locales/tr.js'
import id from './locales/id.js'
import pl from './locales/pl.js'
import uk from './locales/uk.js'
import {
  AVAILABLE_LOCALES,
  discardUnusablePreference,
  isUsablePreference,
  resolveLocale,
} from './index.js'

const LOCALES = {
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
  'pt-BR': ptBR,
  it: itLocale,
  tr,
  id,
  pl,
  uk,
}

/** Flatten to `group.key` paths so a missing or extra key is easy to spot. */
const flat = (object, prefix = '') =>
  Object.entries(object).flatMap(([key, value]) =>
    typeof value === 'object' && value !== null
      ? flat(value, `${prefix}${key}.`)
      : [[`${prefix}${key}`, value]]
  )

const reference = new Map(flat(en))
const placeholders = text => (String(text).match(/\{\w+\}/g) || []).sort().join(',')

describe('Locale files', () => {
  it('ships a file for every advertised locale', () => {
    expect(Object.keys(LOCALES).sort()).toEqual(AVAILABLE_LOCALES.map(l => l.value).sort())
  })

  it.each(Object.keys(LOCALES))('%s has exactly the English key set', locale => {
    const keys = new Map(flat(LOCALES[locale]))
    const missing = [...reference.keys()].filter(key => !keys.has(key))
    const extra = [...keys.keys()].filter(key => !reference.has(key))

    expect(missing).toEqual([])
    expect(extra).toEqual([])
  })

  it.each(Object.keys(LOCALES))('%s keeps every interpolation placeholder', locale => {
    const keys = new Map(flat(LOCALES[locale]))
    // A dropped `{count}` renders the literal placeholder at runtime, which is
    // the kind of translation bug that only shows up in the one screen someone
    // happens to open in that language.
    const mismatched = [...reference.entries()]
      .filter(([key, value]) => keys.has(key) && placeholders(value) !== placeholders(keys.get(key)))
      .map(([key]) => key)

    expect(mismatched).toEqual([])
  })

  it.each(Object.keys(LOCALES))('%s has no empty strings', locale => {
    const empty = [...new Map(flat(LOCALES[locale]))]
      .filter(([, value]) => typeof value === 'string' && !value.trim())
      .map(([key]) => key)

    expect(empty).toEqual([])
  })
})

describe('resolveLocale', () => {
  const withLanguages = (languages, run) => {
    const original = navigator.languages
    Object.defineProperty(navigator, 'languages', { value: languages, configurable: true })
    try {
      return run()
    } finally {
      Object.defineProperty(navigator, 'languages', { value: original, configurable: true })
    }
  }

  it('honours an explicit preference over the system language', () => {
    withLanguages(['de-DE'], () => expect(resolveLocale('ja')).toBe('ja'))
  })

  /// A stored preference that is not a shipped locale used to be returned as-is
  /// and handed to vue-i18n, which cannot find a bundle for it and fails while
  /// mounting — a blank window with nothing on screen to say why. 2.9.32 shipped
  /// with `sing-along.locale` holding the single character "z" on a real machine.
  it.each([['z'], ['garbage'], ['zh'], ['zh-Hans-CN'], ['EN'], ['']])(
    'ignores the unusable preference %j and falls back to the system language',
    preference => {
      withLanguages(['ja-JP'], () => expect(resolveLocale(preference)).toBe('ja'))
    }
  )

  it('ignores an unusable preference when no system language matches either', () => {
    withLanguages(['ar-SA'], () => expect(resolveLocale('z')).toBe('en'))
  })

  it.each([
    [['fr-CA', 'en'], 'fr'],
    [['ja-JP'], 'ja'],
    [['ko-KR'], 'ko'],
    [['es-419'], 'es'],
    [['de-AT'], 'de'],
    [['vi-VN'], 'vi'],
    [['th-TH'], 'th'],
    [['ru-RU'], 'ru'],
    [['en-GB'], 'en'],
    [['pt-BR'], 'pt-BR'],
    [['pt-PT'], 'pt-BR'],
    [['it-IT'], 'it'],
    [['tr-TR'], 'tr'],
    [['id-ID'], 'id'],
    [['pl-PL'], 'pl'],
    [['uk-UA'], 'uk'],
  ])('maps %j to %s', (languages, expected) => {
    withLanguages(languages, () => expect(resolveLocale('auto')).toBe(expected))
  })

  it.each([
    [['zh-TW'], 'zh-Hant'],
    [['zh-HK'], 'zh-Hant'],
    [['zh-MO'], 'zh-Hant'],
    [['zh-Hant-CN'], 'zh-Hant'],
    [['zh-CN'], 'zh-Hans'],
    [['zh-SG'], 'zh-Hans'],
    [['zh'], 'zh-Hans'],
  ])('maps Chinese %j to %s', (languages, expected) => {
    withLanguages(languages, () => expect(resolveLocale('auto')).toBe(expected))
  })

  it('falls back to English for languages it does not ship', () => {
    withLanguages(['ar-SA', 'hi-IN'], () => expect(resolveLocale('auto')).toBe('en'))
  })

  it('skips unsupported tags but keeps looking further down the list', () => {
    withLanguages(['ar-SA', 'th-TH'], () => expect(resolveLocale('auto')).toBe('th'))
  })

describe('a stored locale that cannot be honoured', () => {
  // localStorage is not in the default test environment; a minimal one is enough
  // for these two functions, and keeps the assertions about the stored value
  // honest rather than mocking the functions away.
  let store = {}
  beforeEach(() => {
    store = {}
    globalThis.localStorage = {
      getItem: key => (key in store ? store[key] : null),
      setItem: (key, value) => {
        store[key] = String(value)
      },
      removeItem: key => {
        delete store[key]
      },
    }
    globalThis.localStorage.length = 0
  })

  it('keeps a preference that ships', () => {
    store['sing-along.locale'] = 'zh-Hans'
    expect(isUsablePreference('zh-Hans')).toBe(true)
    expect(discardUnusablePreference()).toBe(false)
    expect(store['sing-along.locale']).toBe('zh-Hans')
  })

  it('keeps auto', () => {
    expect(isUsablePreference('auto')).toBe(true)
    expect(discardUnusablePreference()).toBe(false)
  })

  it('removes a value this build does not ship', () => {
    store['sing-along.locale'] = 'z'
    expect(isUsablePreference('z')).toBe(false)
    expect(discardUnusablePreference()).toBe(true)
    expect(store['sing-along.locale']).toBeUndefined()
  })

  /// Removing rather than writing 'en' matters: the app then follows the system
  /// language, which is what a fresh install does. Writing 'en' would leave a
  /// Chinese user in English permanently, looking like a choice they made.
  it('does not replace the bad value with a different one', () => {
    store['sing-along.locale'] = 'garbage'
    discardUnusablePreference()
    expect(store['sing-along.locale']).toBeUndefined()
  })

  it('does nothing when nothing is stored', () => {
    expect(discardUnusablePreference()).toBe(false)
  })
})
})

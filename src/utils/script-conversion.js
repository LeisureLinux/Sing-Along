/**
 * Convert Chinese lyrics between Simplified and Traditional.
 *
 * Why this exists as a download-time option rather than an always-on transform:
 * converting is lossy in both directions. A one-to-many mapping needs the target
 * word to choose correctly — 发 becomes 發 (發財) or 髮 (頭髮) depending on
 * context — and OpenCC guesses from surrounding characters. That is right most
 * of the time and wrong often enough that silently rewriting someone's lyrics
 * would be worse than leaving them as published.
 *
 * So the user is asked once, on the download popup, and the answer is stored.
 * 'off' keeps the file exactly as LRCLIB serves it.
 */
import { Converter } from 'opencc-js'

// s2t: Simplified -> Traditional.  t2s: the reverse; exact because Traditional
// contains the Simplified forms as a subset in OpenCC's tables.
let toTraditional = null
let toSimplified = null

const getConverter = target => {
  const cache = target === 'traditional' ? toTraditional : toSimplified
  if (cache) {
    return cache
  }

  // 'twp' is Traditional with Taiwanese phrasing; it handles the common
  // one-to-many cases better than plain 't' for lyrics written in mainland
  // Simplified.
  const converter = Converter({
    from: target === 'traditional' ? 'cn' : 'tw',
    to: target === 'traditional' ? 'twp' : 'cn',
  })

  if (target === 'traditional') {
    toTraditional = converter
  } else {
    toSimplified = converter
  }
  return converter
}

/** True when the string contains any CJK ideograph. */
export const hasChinese = text => /[\u4e00-\u9fff\u3400-\u4dbf]/.test(text || '')

/**
 * Convert lyric text to the requested script.
 *
 * Returns the input unchanged when the target matches the current script, when
 * the text has no Chinese, or when conversion is off. Never throws: a failed
 * conversion must not lose the lyrics, so the original is returned on error.
 */
export const convertLyricScript = (text, target) => {
  if (!text || !target || target === 'off' || target === 'ask') {
    return text
  }
  if (!hasChinese(text)) {
    return text
  }

  try {
    return getConverter(target)(text)
  } catch (error) {
    console.error('Lyric script conversion failed, keeping the original text:', error)
    return text
  }
}

/**
 * Which conversion the UI language implies.
 *
 * A Traditional-Chinese reader downloading Simplified lyrics wants them in
 * Traditional, and vice versa. This is only the *suggested* default for the
 * prompt; the user's answer is what gets stored.
 */
export const scriptForLocale = locale => {
  if (locale === 'zh-Hant') {
    return 'traditional'
  }
  if (locale === 'zh-Hans') {
    return 'simplified'
  }
  return 'off'
}

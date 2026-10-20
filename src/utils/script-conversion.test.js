import { describe, expect, it } from 'vitest'
import { convertLyricScript, hasChinese, scriptForLocale } from './script-conversion.js'

// The search dialog's save button goes through the same conversion as a
// download. These tests pin the contract that button depends on.
describe('Lyric script conversion', () => {
  it('converts Traditional to Simplified', () => {
    // 送你一朵小紅花 is what LRCLIB serves; the Simplified form is what a
    // Simplified-Chinese reader expects from the save button.
    expect(convertLyricScript('送你一朵小紅花', 'simplified')).toBe('送你一朵小红花')
  })

  it('converts Simplified to Traditional', () => {
    expect(convertLyricScript('送你一朵小红花', 'traditional')).toBe('送你一朵小紅花')
  })

  it('keeps text untouched when conversion is off, ask, or absent', () => {
    for (const target of ['off', 'ask', undefined, null, '']) {
      expect(convertLyricScript('送你一朵小紅花', target)).toBe('送你一朵小紅花')
    }
  })

  it('leaves text with no Chinese alone', () => {
    expect(convertLyricScript('Hello world', 'simplified')).toBe('Hello world')
  })

  it('detects Chinese text', () => {
    expect(hasChinese('送你一朵小红花')).toBe(true)
    expect(hasChinese('Hello')).toBe(false)
    expect(hasChinese('')).toBe(false)
  })

  it('suggests a target from the UI locale', () => {
    expect(scriptForLocale('zh-Hant')).toBe('traditional')
    expect(scriptForLocale('zh-Hans')).toBe('simplified')
    expect(scriptForLocale('en')).toBe('off')
  })
})

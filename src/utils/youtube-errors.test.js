import { describe, expect, it } from 'vitest'
import { classifyDownloadError } from './youtube-errors.js'

describe('classifyDownloadError', () => {
  it('recognises the bot message as a cookie problem', () => {
    expect(
      classifyDownloadError(
        "ERROR: [youtube] to9jid5ZY6c: Sign in to confirm you're not a bot."
      )
    ).toBe('cookies')
  })

  it('recognises the rotated-cookie warning', () => {
    expect(
      classifyDownloadError(
        'WARNING: The provided YouTube account cookies are no longer valid.'
      )
    ).toBe('cookies')
  })

  it('recognises a login requirement', () => {
    expect(classifyDownloadError('login required')).toBe('cookies')
  })

  it('recognises a proxy failure', () => {
    expect(
      classifyDownloadError("WARNING: [youtube] ('Unable to connect to proxy')")
    ).toBe('proxy')
  })

  it('recognises a timeout as a connection failure', () => {
    expect(classifyDownloadError('Connection to www.youtube.com timed out')).toBe(
      'proxy'
    )
  })

  it('recognises rate limiting', () => {
    expect(classifyDownloadError('ERROR: HTTP Error 429: Too Many Requests')).toBe(
      'tooMany'
    )
  })

  it('recognises an unavailable video', () => {
    expect(classifyDownloadError('ERROR: [youtube] x: This video is unavailable')).toBe(
      'unavailable'
    )
  })

  it('recognises a missing yt-dlp', () => {
    expect(classifyDownloadError('yt-dlp was not found. Install it')).toBe('noBinary')
  })

  it('falls back to unknown for a message nobody recognises', () => {
    expect(classifyDownloadError('something entirely unexpected')).toBe('unknown')
  })

  it('handles an empty message', () => {
    expect(classifyDownloadError('')).toBe('unknown')
    expect(classifyDownloadError(undefined)).toBe('unknown')
  })

  it('prefers the proxy over the rate limit when both are mentioned', () => {
    // A connection failure is the more specific cause: a message about a
    // rate limit can also mention being unable to connect, and the fix is
    // the proxy rather than the waiting.
    expect(
      classifyDownloadError('Unable to connect to proxy (HTTP Error 429)')
    ).toBe('proxy')
  })
})

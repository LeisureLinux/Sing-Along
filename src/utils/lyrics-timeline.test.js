import { describe, expect, it } from 'vitest'
import {
  findLineIndexAt,
  findPrimaryLineIndex,
  isLineActiveAt,
  lineEndMs,
} from './lyrics-timeline.js'

const lines = [
  { start_ms: 0, text: 'a' },
  { start_ms: 1000, text: 'b' },
  { start_ms: 2000, text: 'c' },
  { start_ms: 3000, text: 'd' },
]

describe('findLineIndexAt', () => {
  it('returns -1 for empty or missing input', () => {
    expect(findLineIndexAt([], 500)).toBe(-1)
    expect(findLineIndexAt(null, 500)).toBe(-1)
  })

  it('returns -1 before the first line starts', () => {
    expect(findLineIndexAt([{ start_ms: 1000 }], 999)).toBe(-1)
  })

  it('matches exact, interior, and past-end times', () => {
    expect(findLineIndexAt(lines, 0)).toBe(0)
    expect(findLineIndexAt(lines, 999)).toBe(0)
    expect(findLineIndexAt(lines, 1000)).toBe(1)
    expect(findLineIndexAt(lines, 2999)).toBe(2)
    expect(findLineIndexAt(lines, 99999)).toBe(3)
  })

  it('agrees with a linear scan at every millisecond boundary', () => {
    const many = Array.from({ length: 200 }, (_, i) => ({ start_ms: i * 500 }))
    const linear = time => {
      let found = -1
      many.forEach((line, index) => {
        if (line.start_ms <= time) found = index
      })
      return found
    }
    for (let time = -10; time <= 100_500; time += 137) {
      expect(findLineIndexAt(many, time)).toBe(linear(time))
    }
  })

  it('tolerates a malformed line without throwing', () => {
    const mixed = [{ start_ms: 0 }, { start_ms: 'x' }, { start_ms: 2000 }]
    expect(() => findLineIndexAt(mixed, 2500)).not.toThrow()
  })
})

describe('lineEndMs', () => {
  it('prefers explicit end_ms', () => {
    expect(lineEndMs([{ start_ms: 0, end_ms: 500 }], 0)).toBe(500)
  })

  it('falls back to the next start, then to Infinity', () => {
    expect(lineEndMs(lines, 0)).toBe(1000)
    expect(lineEndMs(lines, 3)).toBe(Infinity)
  })

  it('returns Infinity for a missing line', () => {
    expect(lineEndMs(lines, 99)).toBe(Infinity)
  })
})

describe('isLineActiveAt', () => {
  it('is inclusive at the start and exclusive at the end', () => {
    expect(isLineActiveAt(lines, 1, 1000)).toBe(true)
    expect(isLineActiveAt(lines, 1, 1999)).toBe(true)
    expect(isLineActiveAt(lines, 1, 2000)).toBe(false)
  })

  it('rejects malformed lines', () => {
    expect(isLineActiveAt([{ text: 'no start' }], 0, 0)).toBe(false)
    expect(isLineActiveAt(lines, 99, 0)).toBe(false)
  })

  it('honours an explicit end inside a longer next-line gap', () => {
    const gapped = [{ start_ms: 0, end_ms: 400 }, { start_ms: 2000 }]
    expect(isLineActiveAt(gapped, 0, 399)).toBe(true)
    expect(isLineActiveAt(gapped, 0, 400)).toBe(false)
  })
})

describe('findPrimaryLineIndex', () => {
  it('returns -1 before playback starts', () => {
    expect(findPrimaryLineIndex(lines, -1)).toBe(-1)
    expect(findPrimaryLineIndex([], 0)).toBe(-1)
  })

  it('returns the active line', () => {
    expect(findPrimaryLineIndex(lines, 1500)).toBe(1)
  })

  it('keeps showing the most recent line during a gap', () => {
    const gapped = [{ start_ms: 0, end_ms: 1000 }, { start_ms: 5000 }]
    expect(findPrimaryLineIndex(gapped, 3000)).toBe(0)
  })

  it('resolves overlap in favour of the later line', () => {
    const overlapping = [
      { start_ms: 0, end_ms: 5000 },
      { start_ms: 1000, end_ms: 6000 },
      { start_ms: 9000 },
    ]
    expect(findPrimaryLineIndex(overlapping, 2000)).toBe(1)
    // 5500: the earlier line has ended but the later one is still playing.
    expect(findPrimaryLineIndex(overlapping, 5500)).toBe(1)
    // 8000: nothing is playing, so the most recent past line is shown.
    expect(findPrimaryLineIndex(overlapping, 8000)).toBe(1)
    expect(findPrimaryLineIndex(overlapping, 9000)).toBe(2)
  })
})

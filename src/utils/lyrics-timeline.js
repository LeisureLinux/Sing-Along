/**
 * Timeline lookups for synced lyrics.
 *
 * `LyricsViewer.vue` used to answer "which line is playing?" with a full linear
 * scan on every playback tick (twice: once for the playing line, once for the
 * expanded-view scroll offset). With 200-400 lines per song and a tick every
 * 100ms that is a lot of pointless work, so the lookup is a binary search here
 * and is covered by tests independently of the component.
 */

/** Last index whose `start_ms` is <= `time`, or -1 when `time` precedes all lines. */
export function findLineIndexAt(lines, time) {
  if (!lines || lines.length === 0) {
    return -1
  }

  let low = 0
  let high = lines.length - 1
  let found = -1

  while (low <= high) {
    const mid = (low + high) >> 1
    const start = lines[mid]?.start_ms

    if (!Number.isFinite(start)) {
      // Malformed entry: stop narrowing rather than guessing.
      high = mid - 1
      continue
    }

    if (start <= time) {
      found = mid
      low = mid + 1
    } else {
      high = mid - 1
    }
  }

  return found
}

/** End of a line: explicit `end_ms`, else the next line's start, else Infinity. */
export function lineEndMs(lines, index) {
  const line = lines?.[index]
  if (!line) {
    return Infinity
  }
  if (Number.isFinite(line.end_ms)) {
    return line.end_ms
  }
  const next = lines[index + 1]
  return Number.isFinite(next?.start_ms) ? next.start_ms : Infinity
}

/** Whether `time` falls inside the line at `index`. */
export function isLineActiveAt(lines, index, time) {
  const line = lines?.[index]
  if (!line || !Number.isFinite(line.start_ms)) {
    return false
  }
  return time >= line.start_ms && time < lineEndMs(lines, index)
}

/**
 * Primary line for the current time.
 *
 * Returns the most recently started line that is still playing, resolving
 * overlaps in favour of the later line. When nothing is playing (a gap between
 * lines) the most recent past line is returned so the UI keeps showing it.
 */
export function findPrimaryLineIndex(lines, time) {
  let candidate = findLineIndexAt(lines, time)
  if (candidate < 0) {
    return -1
  }

  // Overlapping lines can extend past the candidate; the loop is bounded by the
  // number of overlapping lines, not by the song length.
  for (let i = candidate + 1; i < lines.length; i++) {
    if (lines[i]?.start_ms > time) {
      break
    }
    if (isLineActiveAt(lines, i, time)) {
      candidate = i
    }
  }

  return candidate
}

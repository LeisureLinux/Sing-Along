const SHORTCUT_OVERRIDES_STORAGE_KEY = 'singalong.edit-lyrics-v2.shortcut-overrides'

const normalizeKey = key => (typeof key === 'string' && key.length === 1 ? key.toLowerCase() : key)

const isCtrlPressed = event => event.ctrlKey || event.metaKey

const hasExactModifiers = (event, { ctrl = false, alt = false, shift = false }) => {
  if (isCtrlPressed(event) !== ctrl) {
    return false
  }

  if (event.altKey !== alt) {
    return false
  }

  if (event.shiftKey !== shift) {
    return false
  }

  return true
}

const isShortcutKeys = keys => {
  return (
    Array.isArray(keys) &&
    keys.length > 0 &&
    keys.every(key => typeof key === 'string' && key.trim())
  )
}

const normalizeShortcutKeys = keys => {
  if (!isShortcutKeys(keys)) {
    return null
  }

  return keys.map(key => key.trim())
}

const loadShortcutOverrides = () => {
  if (typeof window === 'undefined') {
    return {}
  }

  try {
    const raw = window.localStorage.getItem(SHORTCUT_OVERRIDES_STORAGE_KEY)
    if (!raw) {
      return {}
    }

    const parsed = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return {}
    }

    const normalized = {}
    for (const [shortcutId, keys] of Object.entries(parsed)) {
      const normalizedKeys = normalizeShortcutKeys(keys)
      if (normalizedKeys) {
        normalized[shortcutId] = normalizedKeys
      }
    }

    return normalized
  } catch {
    return {}
  }
}

const persistShortcutOverrides = overrides => {
  if (typeof window === 'undefined') {
    return
  }

  try {
    window.localStorage.setItem(SHORTCUT_OVERRIDES_STORAGE_KEY, JSON.stringify(overrides))
  } catch {
    // Ignore storage failures and keep in-memory overrides.
  }
}

let shortcutOverrides = loadShortcutOverrides()

export const getShortcutOverrides = () => ({ ...shortcutOverrides })

export const resolveShortcutKeys = (shortcutId, fallbackKeys = []) => {
  const overrideKeys = shortcutOverrides[shortcutId]
  const normalizedOverrideKeys = normalizeShortcutKeys(overrideKeys)
  if (normalizedOverrideKeys) {
    return normalizedOverrideKeys
  }

  const normalizedFallbackKeys = normalizeShortcutKeys(fallbackKeys)
  return normalizedFallbackKeys || []
}

export const setShortcutOverride = (shortcutId, keys) => {
  if (typeof shortcutId !== 'string' || !shortcutId.trim()) {
    return false
  }

  const normalizedKeys = normalizeShortcutKeys(keys)
  if (!normalizedKeys) {
    return false
  }

  shortcutOverrides = {
    ...shortcutOverrides,
    [shortcutId]: normalizedKeys,
  }

  persistShortcutOverrides(shortcutOverrides)
  return true
}

export const resetShortcutOverride = shortcutId => {
  if (typeof shortcutId !== 'string' || !shortcutId.trim()) {
    return false
  }

  if (!(shortcutId in shortcutOverrides)) {
    return true
  }

  const { [shortcutId]: _removed, ...nextOverrides } = shortcutOverrides
  shortcutOverrides = nextOverrides
  persistShortcutOverrides(shortcutOverrides)
  return true
}

export const resetAllShortcutOverrides = () => {
  shortcutOverrides = {}
  persistShortcutOverrides(shortcutOverrides)
}

const shortcutTokenToEventKey = {
  '↑': 'ArrowUp',
  '↓': 'ArrowDown',
  '←': 'ArrowLeft',
  '→': 'ArrowRight',
  Space: ' ',
  Enter: 'Enter',
  Backspace: 'Backspace',
  Delete: 'Delete',
}

const matchesMainShortcutKey = (event, keyToken) => {
  if (keyToken === '/') {
    return (
      normalizeKey(event.key) === '/' || normalizeKey(event.key) === '?' || event.code === 'Slash'
    )
  }

  if (keyToken === '+') {
    return (
      normalizeKey(event.key) === '+' || normalizeKey(event.key) === '=' || event.code === 'Equal'
    )
  }

  if (keyToken === '-') {
    return (
      normalizeKey(event.key) === '-' || normalizeKey(event.key) === '_' || event.code === 'Minus'
    )
  }

  const eventKey = shortcutTokenToEventKey[keyToken] || keyToken
  return normalizeKey(event.key) === normalizeKey(eventKey)
}

const matchesShortcutKeys = (event, shortcutKeys) => {
  const normalizedKeys = normalizeShortcutKeys(shortcutKeys)
  if (!normalizedKeys) {
    return false
  }

  const ctrl = normalizedKeys.includes('Ctrl')
  const alt = normalizedKeys.includes('Alt')
  const shift = normalizedKeys.includes('Shift')
  const mainKey = normalizedKeys.find(key => key !== 'Ctrl' && key !== 'Alt' && key !== 'Shift')

  if (!mainKey) {
    return false
  }

  if (!hasExactModifiers(event, { ctrl, alt, shift })) {
    return false
  }

  return matchesMainShortcutKey(event, mainKey)
}

export const shortcutKeysFromKeyboardEvent = event => {
  const keys = []

  if (isCtrlPressed(event)) {
    keys.push('Ctrl')
  }

  if (event.altKey) {
    keys.push('Alt')
  }

  if (event.shiftKey) {
    keys.push('Shift')
  }

  const baseKey =
    event.key === ' '
      ? 'Space'
      : event.key === 'ArrowUp'
        ? '↑'
        : event.key === 'ArrowDown'
          ? '↓'
          : event.key === 'ArrowLeft'
            ? '←'
            : event.key === 'ArrowRight'
              ? '→'
              : event.key

  const ignoredBaseKeys = ['Control', 'Meta', 'Alt', 'Shift']
  if (!ignoredBaseKeys.includes(baseKey)) {
    if (baseKey.length === 1) {
      keys.push(baseKey.toUpperCase())
    } else {
      keys.push(baseKey)
    }
  }

  const hasNonModifierKey = keys.some(key => key !== 'Ctrl' && key !== 'Alt' && key !== 'Shift')
  return hasNonModifierKey ? keys : []
}

const createShortcutBinding = ({ id, defaultKeys, descriptionKey }) => ({
  id,
  descriptionKey,
  get defaultKeys() {
    return normalizeShortcutKeys(defaultKeys) || []
  },
  get keys() {
    return resolveShortcutKeys(id, defaultKeys)
  },
  matches: event => matchesShortcutKeys(event, resolveShortcutKeys(id, defaultKeys)),
})

export const globalShortcutBindings = [
  createShortcutBinding({
    id: 'saveLyrics',
    defaultKeys: ['Ctrl', 'S'],
    descriptionKey: 'shortcuts.saveLyrics',
  }),
  createShortcutBinding({
    id: 'openShortcutModal',
    defaultKeys: ['Ctrl', '/'],
    descriptionKey: 'shortcuts.openKeyboardShortcuts',
  }),
]

export const plainEditorShortcutBindings = [
  createShortcutBinding({
    id: 'zoomIn',
    defaultKeys: ['Ctrl', '+'],
    descriptionKey: 'shortcuts.zoomIn',
  }),
  createShortcutBinding({
    id: 'zoomOut',
    defaultKeys: ['Ctrl', '-'],
    descriptionKey: 'shortcuts.zoomOut',
  }),
  createShortcutBinding({
    id: 'zoomReset',
    defaultKeys: ['Ctrl', '0'],
    descriptionKey: 'shortcuts.resetZoom',
  }),
]

export const plainEditorDisplayOnlyShortcuts = [
  { keys: ['Ctrl', 'Scroll'], descriptionKey: 'shortcuts.zoomInOut' },
]

export const syncedEditorShortcutBindings = [
  createShortcutBinding({
    id: 'selectPreviousLine',
    defaultKeys: ['↑'],
    descriptionKey: 'shortcuts.selectPreviousLine',
  }),
  createShortcutBinding({
    id: 'selectNextLine',
    defaultKeys: ['↓'],
    descriptionKey: 'shortcuts.selectNextLine',
  }),
  createShortcutBinding({
    id: 'syncLineToPlayback',
    defaultKeys: ['Space'],
    descriptionKey: 'shortcuts.syncLineToCurrentPlayback',
  }),
  createShortcutBinding({
    id: 'syncLineEndToPlayback',
    defaultKeys: ['Shift', 'Space'],
    descriptionKey: 'shortcuts.syncSelectedLineEndToCurrentPlayback',
  }),
  createShortcutBinding({
    id: 'syncLineEndAndAdvance',
    defaultKeys: ['N'],
    descriptionKey: 'shortcuts.syncSelectedLineEndMoveToNext',
  }),
  createShortcutBinding({
    id: 'syncLineAndAdvance',
    defaultKeys: ['Enter'],
    descriptionKey: 'shortcuts.syncLineSyncPreviousLineEndMoveToNext',
  }),
  createShortcutBinding({
    id: 'syncLineAndAdvanceNoPreviousEndSync',
    defaultKeys: ['Shift', 'Enter'],
    descriptionKey: 'shortcuts.syncLineMoveToNextSkipLineEndSync',
  }),
  createShortcutBinding({
    id: 'rewindLine',
    defaultKeys: ['←'],
    descriptionKey: 'shortcuts.rewind100MsPlay',
  }),
  createShortcutBinding({
    id: 'forwardLine',
    defaultKeys: ['→'],
    descriptionKey: 'shortcuts.forward100MsPlay',
  }),
  createShortcutBinding({
    id: 'rewindLineEnd',
    defaultKeys: ['Shift', '←'],
    descriptionKey: 'shortcuts.rewindLineEndBy100Ms',
  }),
  createShortcutBinding({
    id: 'forwardLineEnd',
    defaultKeys: ['Shift', '→'],
    descriptionKey: 'shortcuts.forwardLineEndBy100Ms',
  }),
  createShortcutBinding({
    id: 'replaySelectedLine',
    defaultKeys: ['P'],
    descriptionKey: 'shortcuts.replaySelectedLine',
  }),
  createShortcutBinding({
    id: 'replayPreviousLine',
    defaultKeys: ['Shift', 'P'],
    descriptionKey: 'shortcuts.replayPreviousLine',
  }),
  createShortcutBinding({
    id: 'deleteSelectedLine',
    defaultKeys: ['Backspace'],
    descriptionKey: 'shortcuts.deleteSelectedLine',
  }),
]

export const wordTimingShortcutBindings = [
  createShortcutBinding({
    id: 'selectPreviousSeparator',
    defaultKeys: ['A'],
    descriptionKey: 'shortcuts.selectPreviousSeparator',
  }),
  createShortcutBinding({
    id: 'selectNextSeparator',
    defaultKeys: ['D'],
    descriptionKey: 'shortcuts.selectNextSeparator',
  }),
  createShortcutBinding({
    id: 'syncSelectedSeparatorNoAdvance',
    defaultKeys: ['S'],
    descriptionKey: 'shortcuts.syncSelectedSeparatorStaySelected',
  }),
  createShortcutBinding({
    id: 'syncSelectedSeparatorAndAdvance',
    defaultKeys: ['Z'],
    descriptionKey: 'shortcuts.syncSelectedSeparatorMoveToNext',
  }),
  createShortcutBinding({
    id: 'resetSeparatorCursor',
    defaultKeys: ['X'],
    descriptionKey: 'shortcuts.resetSeparatorCursorToSecondDivider',
  }),
  createShortcutBinding({
    id: 'deleteSelectedSeparators',
    defaultKeys: ['Delete'],
    descriptionKey: 'shortcuts.deleteSelectedSeparatorsToMergeAdjacentWords',
  }),
]

export const getShortcutCategories = () => [
  {
    id: 'global',
    label: 'Global',
    shortcuts: globalShortcutBindings.map(({ id, keys, defaultKeys, descriptionKey }) => ({
      id,
      keys,
      defaultKeys,
      descriptionKey,
    })),
  },
  {
    id: 'plain',
    label: 'Plain Editor',
    shortcuts: [
      ...plainEditorShortcutBindings.map(({ id, keys, defaultKeys, descriptionKey }) => ({
        id,
        keys,
        defaultKeys,
        descriptionKey,
      })),
      ...plainEditorDisplayOnlyShortcuts,
    ],
  },
  {
    id: 'synced',
    label: 'Synced Editor',
    shortcuts: syncedEditorShortcutBindings.map(({ id, keys, defaultKeys, descriptionKey }) => ({
      id,
      keys,
      defaultKeys,
      descriptionKey,
    })),
  },
  {
    id: 'wordTiming',
    label: 'Word Timing',
    shortcuts: wordTimingShortcutBindings.map(({ id, keys, defaultKeys, descriptionKey }) => ({
      id,
      keys,
      defaultKeys,
      descriptionKey,
    })),
  },
]

export const shortcutCategories = getShortcutCategories()

export const formatShortcutKeys = keys => {
  if (!Array.isArray(keys) || keys.length === 0) {
    return ''
  }

  return keys.join('+')
}

export const findShortcutById = (bindings, shortcutId) => {
  if (!Array.isArray(bindings) || typeof shortcutId !== 'string') {
    return null
  }

  return bindings.find(binding => binding.id === shortcutId) || null
}

export const withShortcutTitle = (title, bindings, shortcutId) => {
  const baseTitle = typeof title === 'string' ? title : ''
  const binding = findShortcutById(bindings, shortcutId)
  const shortcutLabel = formatShortcutKeys(binding?.keys)

  if (!shortcutLabel) {
    return baseTitle
  }

  return `${baseTitle} (${shortcutLabel})`
}

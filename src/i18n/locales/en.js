/**
 * English strings — the reference locale.
 *
 * Keys are grouped by the part of the UI they appear in, not by component, so a
 * translator can work through a screen at a time. Interpolations use vue-i18n's
 * named syntax (`{count}`) because word order differs between languages and
 * positional placeholders cannot be reordered safely.
 */
export default {
  app: {
    name: 'Sing-Along',
    loadingLibrary: 'Loading library...',
  },

  tabs: {
    tracks: 'Tracks',
    albums: 'Albums',
    artists: 'Artists',
    lrclib: 'LRCLIB',
  },

  actions: {
    download: 'Download',
    downloadAll: 'Download all lyrics',
    export: 'Export',
    cancel: 'Cancel',
    close: 'Close',
    save: 'Save',
    delete: 'Delete',
    confirm: 'Confirm',
    back: 'Back',
    search: 'Search',
    refresh: 'Refresh',
    open: 'Open',
    copy: 'Copy',
    copied: 'Copied',
    apply: 'Apply',
    retry: 'Retry',
  },

  search: {
    libraryPlaceholder: 'Search tracks in library...',
    lyricsPlaceholder: 'Type a song title, album, or artist to find lyrics...',
    noResults: 'No results',
  },

  track: {
    duration: 'Duration',
    lyrics: 'Lyrics',
    instrumental: 'Instrumental',
    synced: 'Synced',
    plain: 'Plain',
    track: 'Track',
    unknownAlbum: 'Unknown Album',
    unknownArtist: 'Unknown Artist',
  },

  player: {
    previousTrack: 'Previous track',
    nextTrack: 'Next track',
    playbackSpeed: 'Playback speed',
    shuffleOn: 'Shuffle on',
    shuffleOff: 'Shuffle off',
    repeatOne: 'Repeat one',
    repeatAll: 'Repeat all',
    repeatOff: 'Repeat off',
    volume: 'Volume',
  },

  filters: {
    allTracks: 'All tracks (overwrite existing lyrics)',
    syncedTracks: 'Only tracks without synced lyrics',
    plainTracks: 'Only tracks without plain lyrics',
    instrumentalTracks: 'Instrumental tracks',
    noLyricsTracks: 'Tracks without any lyrics',
  },

  config: {
    title: 'Settings',
    theme: 'Theme',
    themeAuto: 'Auto',
    themeLight: 'Light',
    themeDark: 'Dark',
    language: 'Language',
    languageAuto: 'Follow system',
    directories: 'Music directories',
    addDirectory: 'Add new directory',
    manageDirectories: 'Manage directories',
    directoriesHint: 'Add and remove scanning directories...',
    lrclibInstance: 'LRCLIB instance',
    embed: 'Embed lyrics into music files',
    embedExperimental: 'Experimental',
    downloadLyricsFor: 'Download lyrics for',
    showLineCount: 'Show line count',
    save: 'Save settings',
    saved: 'Settings saved',
  },

  exportOptions: {
    title: 'Export options',
    toTrackDirectory: 'Export all lyrics to the tracks’ directory',
    plainLyrics: 'Plain lyrics (.txt)',
    syncedLyrics: 'Synced lyrics (.lrc)',
    embedIntoTrack: 'Embed into track',
    wordLevelHint: 'Word-level lyrics — pick one format for the target player:',
    wordNone: 'Do not export word-level',
    a2: 'Enhanced LRC / A2 (.alrc) — Android',
    ttml: 'TTML (.ttml) — iOS',
  },

  downloadOptions: {
    title: 'Download options',
    autoExport: 'Automatically export lyrics',
    createLrc: 'Create separate LRC files',
    exportLrc: 'LRC (.lrc)',
    exportTxt: 'Plain text (.txt)',
    exportEmbedded: 'Embed into files',
    start: 'Start download',
  },

  about: {
    title: 'About',
    update: 'Update',
    currentVersion: 'Current version',
    newestVersion: 'Newest version',
    updateAvailable: 'A new version is available.',
    upToDate: 'You are using the newest version.',
    updateFailed: 'Could not check for updates.',
    homePage: 'Home page, contact and support',
    credits: 'Credits and license',
    creditsText:
      'Sing-Along is a fork of LRCGET, Copyright (c) 2023 tranxuanthang, used under the MIT License.',
    lyricsProvidedBy: 'Lyrics are provided by LRCLIB.',
  },

  status: {
    scanning: 'Scanning...',
    discovering: 'Discovering files...',
    preparing: 'Preparing',
    downloading: 'Downloading',
    exporting: 'Exporting',
    exported: 'Exported',
    processing: 'Processing',
    loadingEditor: 'Loading editor...',
    noLyricsFound: 'No lyrics found',
  },

  errors: {
    scanFailed: 'Scanning the library failed',
    loadTracksFailed: 'Failed to load tracks',
    searchFailed: 'Searching for lyrics failed. Please try again.',
    nothingToExport: 'No tracks with lyrics found to export',
    exportFormatRequired: 'Select at least one export format',
    unsavedChanges: 'You have unsaved changes. Are you sure you want to close?',
  },
}

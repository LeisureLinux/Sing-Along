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

  setup: {
    selectStrategy: 'Select Strategy',

    selectDirectories: 'Select directories',
  },

  tabs: {
    tracks: 'Tracks',
    albums: 'Albums',
    artists: 'Artists',
    lrclib: 'LRCLIB',
  },

  actions: {
    chooseFile: 'Choose file...',

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
    noTracksFound: 'No tracks found',
    typeToSearch: 'Type to search your library',

    libraryPlaceholder: 'Search tracks in library...',
    lyricsPlaceholder: 'Type a song title, album, or artist to find lyrics...',
    title: 'Title',
    album: 'Album',
    artist: 'Artist',
    tracksPlaceholder: 'Search for tracks...',
    albumsPlaceholder: 'Search for albums...',
    artistsPlaceholder: 'Search for artists...',
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
    scanIncremental: 'Scan for new and modified tracks...',
    scanFull: 'Reset library and perform full scan...',

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
    skipExisting: 'Skip tracks that already have lyrics',

    title: 'Download options',
    autoExport: 'Automatically export lyrics',
    createLrc: 'Create separate LRC files',
    exportLrc: 'LRC (.lrc)',
    exportTxt: 'Plain text (.txt)',
    exportEmbedded: 'Embed into files',
    start: 'Start download',
  },

  editor: {
    syncWord: 'Sync word',
    noPlainYet: 'No plain lyrics yet',
    noSyncedYet: 'No synced lyric lines yet',
    markedInstrumental: 'Track is marked as instrumental',

    publishHint: 'Publish lyrics into your LRCLIB instance:',
    exportHint: "Export to track's directory:",
    savingAndExporting: 'Saving and exporting...',
    saveAndExport: 'Save and export',
    saveAndPublish: 'Save and Publish',
    viewYamlDebug: 'View YAML debug',
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
    releasePage: 'Sing-Along release page',
    checking: 'Fetching newest version data from GitHub...',
    forkOf: 'Sing-Along is a fork of',
    copyrightSuffix: ', Copyright (c) 2023 tranxuanthang, used under the MIT License.',
    contributors: 'Previously contributed code by',
    contributorsSuffix: ', also under the MIT License.',
    lyricsFrom: 'Lyrics are provided by',
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

  lrclib: {
    previousDownload: 'Lyrics for this track were previously downloaded.',
  },

  errors: {
    scanFailed: 'Scanning the library failed',
    loadTracksFailed: 'Failed to load tracks',
    searchFailed: 'Searching for lyrics failed. Please try again.',
    nothingToExport: 'No tracks with lyrics found to export',
    exportFormatRequired: 'Select at least one export format',
    exportStartFailed: 'Failed to start export: {error}',
    scanFailedDetail: 'Scanning the library failed: {error}',
    unsavedChanges: 'You have unsaved changes. Are you sure you want to close?',
  },
}

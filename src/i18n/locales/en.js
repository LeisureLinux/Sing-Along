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
    windowTitle: 'Sing-Along Player: Offline music lyric download/edit/export',
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
    lrclib: 'Search',
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
    continueLabel: 'Continue',
    syncLyrics: 'Sync lyrics',
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

  album: {
    tracks: 'Tracks',
    trackCount: '{count} tracks',
    downloadAlbumLyrics: 'Download album lyrics',
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
    appearance: 'Appearance',
    fileWriteWarning:
      'This option could corrupt your track files. Make sure to back up your library before enabling it.',
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
    coverSync: 'Album Cover',
    syncCoverLabel: 'Download album cover art',
    syncCoverHint:
      'Fetch front cover art from iTunes or Deezer and embed it into the audio file when syncing lyrics. Both sources are public and require no key.',
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
    syncCover: 'Also download album cover art',
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
    saveHint: 'Save lyrics',
    saveAndPublish: 'Save and Publish',
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
    donate: 'Support',
    donateAlt: 'WeChat payment QR code',
    donateHint: 'Thank you for supporting Sing-Along.',
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

  lyricsSearch: {
    title: 'Search lyrics',
    placeholder: 'Type a song title, album, or artist to find lyrics...',
    searchingFor: 'Searching for',
    foundTracks: 'Found {count} tracks',
    lines: '{count} lines',
    preview: 'Preview this lyrics',
    edit: 'Edit this lyrics',
    errorSearch: 'An error occurred while searching for lyrics. Please try again.',
    errorOpen: 'An error occurred while opening the lyrics. Please try again.',
    chooseReferenceTrack: 'Choose a reference track',
    searchTracksPlaceholder: 'Search tracks in library...',
    editWithAudio: 'Edit with audio',
    existingLyricsFound: 'Existing Lyrics Found',
    redownload: 'Redownload',
    errorLoadTracks: 'Failed to load tracks',
    errorReadAudio: 'Failed to read the audio file',
    errorRedownload: 'Failed to redownload lyrics',
    noLyricsSubmitted: 'There is currently no lyrics submitted for this track',
    continueEditing: 'Continue Editing',
  },

  lrclib: {
    previousDownload: 'Lyrics for this track were previously downloaded.',
  },

  scriptConversion: {
    legend: 'Chinese script of downloaded lyrics',
    off: 'Leave as published',
    toSimplified: 'Convert to Simplified Chinese',
    toTraditional: 'Convert to Traditional Chinese',
    hint: 'Asked once; remembered for future downloads. Conversion is automatic and usually right, but it cannot resolve every one-to-many character.',
  },

  shortcuts: {
    changeFor: 'Change shortcut for {action}',
    resetFor: 'Reset shortcut for {action}',
    deleteSelectedLine: 'Delete selected line',
    deleteSelectedSeparatorsToMergeAdjacentWords:
      'Delete selected separators to merge adjacent words',
    forward100MsPlay: 'Forward 100 ms & play',
    forwardLineEndBy100Ms: 'Forward line end by 100 ms',
    openKeyboardShortcuts: 'Open keyboard shortcuts',
    replayPreviousLine: 'Replay previous line',
    replaySelectedLine: 'Replay selected line',
    resetSeparatorCursorToSecondDivider: 'Reset separator cursor to second divider',
    resetZoom: 'Reset zoom',
    rewind100MsPlay: 'Rewind 100 ms & play',
    rewindLineEndBy100Ms: 'Rewind line end by 100 ms',
    saveLyrics: 'Save lyrics',
    selectNextLine: 'Select next line',
    selectNextSeparator: 'Select next separator',
    selectPreviousLine: 'Select previous line',
    selectPreviousSeparator: 'Select previous separator',
    syncLineMoveToNextSkipLineEndSync: 'Sync line & move to next (skip line end sync)',
    syncLineSyncPreviousLineEndMoveToNext: 'Sync line, sync previous line end & move to next',
    syncLineToCurrentPlayback: 'Sync line to current playback',
    syncSelectedLineEndMoveToNext: 'Sync selected line end & move to next',
    syncSelectedLineEndToCurrentPlayback: 'Sync selected line end to current playback',
    syncSelectedSeparatorMoveToNext: 'Sync selected separator & move to next',
    syncSelectedSeparatorStaySelected: 'Sync selected separator (stay selected)',
    zoomIn: 'Zoom in',
    zoomInOut: 'Zoom in/out',
    zoomOut: 'Zoom out',
  },

  viewer: {
    downloadDone: 'Downloaded',
    exportDone: 'Export Complete',
    finish: 'Finish',
    stop: 'Stop',
    found: 'found',
    notFound: 'not found',
    total: 'total',
    exported: 'exported',
    skipped: 'skipped',
    errors: 'errors',
    exportOk: 'exported successfully',
    unknown: 'unknown',
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

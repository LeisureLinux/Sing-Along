/** 繁體中文 */
export default {
  app: {
    name: 'Sing-Along',
    loadingLibrary: '正在載入曲庫…',
  },

  tabs: {
    tracks: '歌曲',
    albums: '專輯',
    artists: '藝人',
    lrclib: 'LRCLIB',
  },

  actions: {
    download: '下載',
    downloadAll: '下載全部歌詞',
    export: '匯出',
    cancel: '取消',
    close: '關閉',
    save: '儲存',
    delete: '刪除',
    confirm: '確定',
    back: '返回',
    search: '搜尋',
    refresh: '重新整理',
    open: '開啟',
    copy: '複製',
    copied: '已複製',
    apply: '套用',
    retry: '重試',
  },

  search: {
    libraryPlaceholder: '在曲庫中搜尋歌曲…',
    lyricsPlaceholder: '輸入歌名、專輯或藝人以尋找歌詞…',
    noResults: '沒有結果',
  },

  track: {
    duration: '長度',
    lyrics: '歌詞',
    instrumental: '純音樂',
    synced: '同步',
    plain: '純文字',
    track: '歌曲',
    unknownAlbum: '未知專輯',
    unknownArtist: '未知藝人',
  },

  player: {
    previousTrack: '上一首',
    nextTrack: '下一首',
    playbackSpeed: '播放速度',
    shuffleOn: '隨機播放已開',
    shuffleOff: '隨機播放已關',
    repeatOne: '單曲循環',
    repeatAll: '清單循環',
    repeatOff: '不循環',
    volume: '音量',
  },

  filters: {
    allTracks: '全部歌曲（覆寫現有歌詞）',
    syncedTracks: '僅缺少同步歌詞的歌曲',
    plainTracks: '僅缺少純文字歌詞的歌曲',
    instrumentalTracks: '純音樂歌曲',
    noLyricsTracks: '完全沒有歌詞的歌曲',
  },

  config: {
    title: '設定',
    theme: '主題',
    themeAuto: '跟隨系統',
    themeLight: '淺色',
    themeDark: '深色',
    language: '語言',
    languageAuto: '跟隨系統',
    directories: '音樂目錄',
    addDirectory: '新增目錄',
    manageDirectories: '管理目錄',
    directoriesHint: '新增或移除掃描目錄…',
    lrclibInstance: 'LRCLIB 實例',
    embed: '將歌詞嵌入音樂檔案',
    embedExperimental: '實驗性',
    downloadLyricsFor: '為以下歌曲下載歌詞',
    showLineCount: '顯示歌詞行數',
    save: '儲存設定',
    saved: '設定已儲存',
  },

  exportOptions: {
    title: '匯出選項',
    toTrackDirectory: '將全部歌詞匯出到歌曲所在目錄',
    plainLyrics: '純文字歌詞（.txt）',
    syncedLyrics: '同步歌詞（.lrc）',
    embedIntoTrack: '嵌入到歌曲檔案',
    wordLevelHint: '逐詞歌詞 — 請為目標播放器選擇一種格式：',
    wordNone: '不匯出逐詞歌詞',
    a2: '增強型 LRC / A2（.alrc）— Android',
    ttml: 'TTML（.ttml）— iOS',
  },

  downloadOptions: {
    title: '下載選項',
    autoExport: '下載後自動匯出歌詞',
    createLrc: '產生獨立的 LRC 檔案',
    exportLrc: 'LRC（.lrc）',
    exportTxt: '純文字（.txt）',
    exportEmbedded: '嵌入到檔案',
    start: '開始下載',
  },

  about: {
    title: '關於',
    update: '更新',
    currentVersion: '目前版本',
    newestVersion: '最新版本',
    updateAvailable: '有新版本可用。',
    upToDate: '你正在使用最新版本。',
    updateFailed: '無法檢查更新。',
    homePage: '首頁、聯絡與支援',
    credits: '致謝與授權',
    creditsText:
      'Sing-Along 是 LRCGET 的分支專案，Copyright (c) 2023 tranxuanthang，依 MIT 授權使用。',
    lyricsProvidedBy: '歌詞由 LRCLIB 提供。',
  },

  status: {
    scanning: '正在掃描…',
    discovering: '正在尋找檔案…',
    preparing: '準備中',
    downloading: '正在下載',
    exporting: '正在匯出',
    exported: '已匯出',
    processing: '處理中',
    loadingEditor: '正在載入編輯器…',
    noLyricsFound: '未找到歌詞',
  },

  errors: {
    scanFailed: '掃描曲庫失敗',
    loadTracksFailed: '載入歌曲失敗',
    searchFailed: '搜尋歌詞時發生錯誤，請重試。',
    nothingToExport: '沒有找到帶歌詞的歌曲可匯出',
    exportFormatRequired: '請至少選擇一種匯出格式',
    unsavedChanges: '有未儲存的變更，確定要關閉嗎？',
  },
}

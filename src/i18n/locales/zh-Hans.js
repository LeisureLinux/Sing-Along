/** 简体中文 */
export default {
  app: {
    name: 'Sing-Along',
    loadingLibrary: '正在加载曲库…',
  },

  tabs: {
    tracks: '歌曲',
    albums: '专辑',
    artists: '艺人',
    lrclib: 'LRCLIB',
  },

  actions: {
    download: '下载',
    downloadAll: '下载全部歌词',
    export: '导出',
    cancel: '取消',
    close: '关闭',
    save: '保存',
    delete: '删除',
    confirm: '确定',
    back: '返回',
    search: '搜索',
    refresh: '刷新',
    open: '打开',
    copy: '复制',
    copied: '已复制',
    apply: '应用',
    retry: '重试',
  },

  search: {
    libraryPlaceholder: '在曲库中搜索歌曲…',
    lyricsPlaceholder: '输入歌名、专辑或艺人以查找歌词…',
    noResults: '没有结果',
  },

  track: {
    duration: '时长',
    lyrics: '歌词',
    instrumental: '纯音乐',
    synced: '同步',
    plain: '纯文本',
    track: '歌曲',
    unknownAlbum: '未知专辑',
    unknownArtist: '未知艺人',
  },

  player: {
    previousTrack: '上一首',
    nextTrack: '下一首',
    playbackSpeed: '播放速度',
    shuffleOn: '随机播放已开',
    shuffleOff: '随机播放已关',
    repeatOne: '单曲循环',
    repeatAll: '列表循环',
    repeatOff: '不循环',
    volume: '音量',
  },

  filters: {
    allTracks: '全部歌曲（覆盖已有歌词）',
    syncedTracks: '仅缺少同步歌词的歌曲',
    plainTracks: '仅缺少纯文本歌词的歌曲',
    instrumentalTracks: '纯音乐歌曲',
    noLyricsTracks: '完全没有歌词的歌曲',
  },

  config: {
    title: '设置',
    theme: '主题',
    themeAuto: '跟随系统',
    themeLight: '浅色',
    themeDark: '深色',
    language: '语言',
    languageAuto: '跟随系统',
    directories: '音乐目录',
    addDirectory: '添加新目录',
    manageDirectories: '管理目录',
    directoriesHint: '添加或移除扫描目录…',
    lrclibInstance: 'LRCLIB 实例',
    embed: '将歌词嵌入音乐文件',
    embedExperimental: '实验性',
    downloadLyricsFor: '为以下歌曲下载歌词',
    showLineCount: '显示歌词行数',
    save: '保存设置',
    saved: '设置已保存',
  },

  exportOptions: {
    title: '导出选项',
    toTrackDirectory: '将全部歌词导出到歌曲所在目录',
    plainLyrics: '纯文本歌词（.txt）',
    syncedLyrics: '同步歌词（.lrc）',
    embedIntoTrack: '嵌入到歌曲文件',
    wordLevelHint: '逐词歌词 — 请为目标播放器选择一种格式：',
    wordNone: '不导出逐词歌词',
    a2: '增强型 LRC / A2（.alrc）— Android',
    ttml: 'TTML（.ttml）— iOS',
  },

  downloadOptions: {
    title: '下载选项',
    autoExport: '下载后自动导出歌词',
    createLrc: '生成独立的 LRC 文件',
    exportLrc: 'LRC（.lrc）',
    exportTxt: '纯文本（.txt）',
    exportEmbedded: '嵌入到文件',
    start: '开始下载',
  },

  about: {
    title: '关于',
    update: '更新',
    currentVersion: '当前版本',
    newestVersion: '最新版本',
    updateAvailable: '有新版本可用。',
    upToDate: '你正在使用最新版本。',
    updateFailed: '无法检查更新。',
    homePage: '主页、联系与支持',
    credits: '致谢与许可',
    creditsText:
      'Sing-Along 是 LRCGET 的分支项目，Copyright (c) 2023 tranxuanthang，依 MIT 许可使用。',
    lyricsProvidedBy: '歌词由 LRCLIB 提供。',
  },

  status: {
    scanning: '正在扫描…',
    discovering: '正在发现文件…',
    preparing: '准备中',
    downloading: '正在下载',
    exporting: '正在导出',
    exported: '已导出',
    processing: '处理中',
    loadingEditor: '正在加载编辑器…',
    noLyricsFound: '未找到歌词',
  },

  errors: {
    scanFailed: '扫描曲库失败',
    loadTracksFailed: '加载歌曲失败',
    searchFailed: '搜索歌词时出错，请重试。',
    nothingToExport: '没有找到带歌词的歌曲可导出',
    exportFormatRequired: '请至少选择一种导出格式',
    unsavedChanges: '有未保存的修改，确定要关闭吗？',
  },
}

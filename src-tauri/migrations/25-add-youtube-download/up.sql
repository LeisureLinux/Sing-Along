-- Where the external downloader is and what it needs.
--
-- `ytdlp_binary` is the `yt-dlp` to run. Empty means the one on PATH.
--
-- `ytdlp_cookies_json` is the file a browser extension writes when the user
-- exports their YouTube cookies. It is the JSON, not the text format, because
-- the JSON is what the user has: the export is a button in the browser, and
-- asking them to convert it afterwards would be a step they would forget.
-- The text format is derived from it and written beside it.
--
-- `ytdlp_output_dir` is where the downloads land. Empty means the first
-- watched directory, which is where the rest of the library lives.
ALTER TABLE config_data ADD COLUMN ytdlp_binary TEXT NOT NULL DEFAULT '';
ALTER TABLE config_data ADD COLUMN ytdlp_cookies_json TEXT NOT NULL DEFAULT '';
ALTER TABLE config_data ADD COLUMN ytdlp_output_dir TEXT NOT NULL DEFAULT '';

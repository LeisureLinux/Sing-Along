-- NetEase fallback toggle.
--
-- LRCLIB has thin coverage of Chinese music; when it returns 404 for a track,
-- the downloader can try NetEase (music.163.com) as a second source. That is an
-- unofficial API which may change without notice, so it is behind a switch, and
-- the switch defaults to on because Chinese-music coverage is the point.
--
-- This is migration 20, not a line added to 19: 19 shipped with v2.3.2, so
-- existing databases are already at version 19 and will never re-run it.

ALTER TABLE config_data ADD COLUMN netease_fallback_enabled BOOLEAN NOT NULL DEFAULT TRUE;

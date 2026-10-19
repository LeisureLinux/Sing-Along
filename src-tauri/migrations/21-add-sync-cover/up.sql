-- Whether to download and embed album cover art during lyrics download.
-- Off by default: cover art adds significant download volume and writes
-- binary data into audio files, which some users may not want.
ALTER TABLE config_data ADD COLUMN sync_cover BOOLEAN NOT NULL DEFAULT FALSE;

-- Whether to contact GitHub for the latest release on startup.
--
-- Off means no automatic network call at all; the About dialog still offers a
-- manual check. Default on so existing installs keep the current behaviour of
-- being told when a newer version exists.
ALTER TABLE config_data ADD COLUMN update_check_enabled BOOLEAN NOT NULL DEFAULT TRUE;

-- Set once a version has been announced, so the same release is only mentioned
-- on startup a single time.
ALTER TABLE config_data ADD COLUMN last_announced_version TEXT NOT NULL DEFAULT '';

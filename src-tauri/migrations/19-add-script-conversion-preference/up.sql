-- Whether downloaded lyrics should be converted to the UI's Chinese script.
--
-- The setting is one tri-state value rather than two booleans, because the
-- meaningful choices are "leave lyrics as published", "convert to Simplified"
-- and "convert to Traditional" — a pair of flags would allow contradictory
-- states and say nothing about the default.
--
-- 'ask' keeps the current behaviour until the user answers the prompt shown on
-- the download popup; the answer is written back here so it is asked once.
ALTER TABLE config_data ADD COLUMN lyric_script TEXT NOT NULL DEFAULT 'ask';

-- Existing rows predate the prompt; 'ask' is the correct value for them too.
UPDATE config_data SET lyric_script = 'ask' WHERE lyric_script IS NULL;

-- Where the AMLL word-level lyrics database is served from.
--
-- Empty means the built-in default. It is a setting rather than a constant
-- because the database is one GitHub repository maintained by volunteers: if it
-- stops, any of its hundreds of forks serves the same paths, and a user who
-- cares can point this at one without waiting for a release.
--
-- The same shape as `lrclib_instance`, for the same reason.
ALTER TABLE config_data ADD COLUMN amll_db_url TEXT NOT NULL DEFAULT '';

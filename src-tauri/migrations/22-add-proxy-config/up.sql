-- Optional HTTP proxy for the lyric APIs.
--
-- Empty means "connect directly". This exists because reqwest otherwise picks up
-- HTTP_PROXY/HTTPS_PROXY from whatever environment launched the app, which may
-- be a proxy set up for entirely different tooling — and one that cannot reach
-- the lyric services. Making it explicit keeps the choice visible instead of
-- depending on the shell the desktop session inherited.
ALTER TABLE config_data ADD COLUMN proxy_url TEXT NOT NULL DEFAULT '';

-- Rebuild the full-text indexes with the `trigram` tokenizer.
--
-- The default `unicode61` tokenizer treats a run of CJK characters as a single
-- token, so "送你一朵小红花" is indexed as one word and a search for "小红花"
-- matches nothing: prefix queries only help when the user types the beginning of
-- a token. `trigram` indexes overlapping three-character sequences instead,
-- which is what makes substring search work for Chinese and for the middle of
-- Latin words ("remember" inside "Don't You Remember").
--
-- Trigram has one constraint: query terms shorter than three characters cannot
-- be matched. The application falls back to a LIKE scan for those, so the effect
-- is a slower query on very short input rather than no results.

DROP TABLE IF EXISTS tracks_fts;
DROP TABLE IF EXISTS albums_fts;
DROP TABLE IF EXISTS artists_fts;

CREATE VIRTUAL TABLE tracks_fts USING fts5(
    track_id UNINDEXED,
    title,
    artist_name,
    album_name,
    tokenize = 'trigram'
);

CREATE VIRTUAL TABLE albums_fts USING fts5(
    album_id UNINDEXED,
    album_name,
    album_artist_name,
    tokenize = 'trigram'
);

CREATE VIRTUAL TABLE artists_fts USING fts5(
    artist_id UNINDEXED,
    artist_name,
    tokenize = 'trigram'
);

INSERT INTO tracks_fts (track_id, title, artist_name, album_name)
SELECT
    t.id,
    t.title_lower,
    a.name_lower,
    al.name_lower
FROM tracks t
JOIN artists a ON t.artist_id = a.id
JOIN albums al ON t.album_id = al.id;

INSERT INTO albums_fts (album_id, album_name, album_artist_name)
SELECT
    id,
    name_lower,
    album_artist_name_lower
FROM albums;

INSERT INTO artists_fts (artist_id, artist_name)
SELECT
    id,
    name_lower
FROM artists;

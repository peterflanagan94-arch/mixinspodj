-- Where a track's BPM and key came from: 'traktor' when imported from the
-- DJ's Traktor collection (authoritative, never overwritten by analysis),
-- NULL when estimated by MixInspo DJ's own analysis.
ALTER TABLE tracks ADD COLUMN tempo_source TEXT;

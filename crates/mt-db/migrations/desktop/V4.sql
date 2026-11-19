ALTER TABLE tracks ADD COLUMN artwork_source_id TEXT REFERENCES artwork(id) ON DELETE SET NULL;
ALTER TABLE tracks ADD COLUMN artwork_source_kind INTEGER CHECK (artwork_source_kind BETWEEN 0 AND 2);

-- Remember any artwork already assigned to older rows. Re-read each audio file
-- once because the previous scanner did not inspect embedded pictures.
UPDATE tracks
SET artwork_source_id = artwork_id,
    artwork_source_kind = CASE WHEN artwork_id IS NOT NULL THEN 2 END,
    file_mtime_ns = NULL;

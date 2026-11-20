CREATE INDEX tracks_file_hash ON tracks(file_hash) WHERE file_hash IS NOT NULL;

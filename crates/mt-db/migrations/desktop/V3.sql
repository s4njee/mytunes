-- V1 and V2 stored no exact filesystem timestamp. Their rows are read once
-- after migration to establish a baseline for the incremental scanner.
ALTER TABLE tracks ADD COLUMN file_mtime_ns INTEGER;

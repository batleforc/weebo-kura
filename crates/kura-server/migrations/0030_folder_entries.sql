-- RFC 0007: a folder is an entry with no content. A live revision without a
-- blob is a folder (size 0); a deleted revision still never has a blob.
ALTER TABLE revision DROP CONSTRAINT revision_check;
ALTER TABLE revision ADD CONSTRAINT revision_check
    CHECK (NOT (deleted AND blob_hash IS NOT NULL) AND (blob_hash IS NOT NULL OR size = 0));

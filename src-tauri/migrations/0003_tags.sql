-- Tags (V2): free labels put on operations, besides the note (`operation.note`, created in 0001).
-- A tag is created the first time it is put on an operation, and stays until it is deleted.

CREATE TABLE tag (
    id   INTEGER PRIMARY KEY,
    name TEXT    NOT NULL CHECK (name <> '')
);

CREATE UNIQUE INDEX tag_unique_name ON tag (name COLLATE NOCASE);

CREATE TABLE operation_tag (
    operation_id INTEGER NOT NULL REFERENCES operation (id) ON DELETE CASCADE,
    tag_id       INTEGER NOT NULL REFERENCES tag (id) ON DELETE CASCADE,
    PRIMARY KEY (operation_id, tag_id)
);

CREATE INDEX operation_tag_tag ON operation_tag (tag_id);

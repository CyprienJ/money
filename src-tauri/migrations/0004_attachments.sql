-- Attachments (V2): files (bills, receipts) put on an operation, copied into the
-- `attachments/` directory next to the data file under `stored_name`.
-- Files whose row is gone (operation deleted, import cancelled) are removed by the application.

CREATE TABLE attachment (
    id           INTEGER PRIMARY KEY,
    operation_id INTEGER NOT NULL REFERENCES operation (id) ON DELETE CASCADE,
    file_name    TEXT    NOT NULL CHECK (file_name <> ''),   -- original name, shown to the user
    stored_name  TEXT    NOT NULL UNIQUE,                    -- name in attachments/
    size_bytes   INTEGER NOT NULL CHECK (size_bytes >= 0),
    added_at     TEXT    NOT NULL
);

CREATE INDEX attachment_operation ON attachment (operation_id);

-- no-transaction
-- OFX and QIF imports (V2).
--
-- The CHECK constraint of import_batch.source_type gains 'OFX' and 'QIF': SQLite rebuilds the
-- table. Foreign keys are disabled during the rebuild, outside a transaction (a no-op inside
-- one): dropping the old table must not cascade to the imported operations.

PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE import_batch_new (
    id                            INTEGER PRIMARY KEY,
    account_id                    INTEGER NOT NULL REFERENCES account (id) ON DELETE RESTRICT,
    source_type                   TEXT    NOT NULL CHECK (source_type IN ('CSV', 'PDF', 'OFX', 'QIF')),
    source_file_name              TEXT    NOT NULL,
    source_file_hash              TEXT    NOT NULL,
    imported_at                   TEXT    NOT NULL,
    statement_start_balance_cents INTEGER,
    statement_end_balance_cents   INTEGER,
    balance_check_status          TEXT    NOT NULL
        CHECK (balance_check_status IN ('OK', 'MISMATCH', 'NOT_AVAILABLE'))
);

INSERT INTO import_batch_new SELECT * FROM import_batch;
DROP TABLE import_batch;
ALTER TABLE import_batch_new RENAME TO import_batch;

CREATE INDEX import_batch_account ON import_batch (account_id);
CREATE INDEX import_batch_file_hash ON import_batch (source_file_hash);

-- Category of the application chosen for a category of a QIF file ("Alimentation:Courses"),
-- remembered for the next imports.
CREATE TABLE source_category_mapping (
    source_category TEXT    PRIMARY KEY,
    category_id     INTEGER NOT NULL REFERENCES category (id) ON DELETE CASCADE
);

COMMIT;

PRAGMA foreign_keys = ON;

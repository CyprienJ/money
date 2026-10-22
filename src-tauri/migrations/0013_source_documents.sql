-- Source documents (V2): every imported file is kept in the statements/ directory, next to
-- the data file, under the SHA-256 of its content; an operation or a portfolio transaction
-- opens the statement it comes from, at its page (PDF) or line (CSV, OFX, QIF).

-- Name of the copy in statements/; NULL for the imports made before.
ALTER TABLE import_batch ADD COLUMN stored_file TEXT;

-- Position of the line of the operation in its file, from 1.
ALTER TABLE operation ADD COLUMN source_page INTEGER;
ALTER TABLE operation ADD COLUMN source_line INTEGER;

-- Trades read from a statement (Trade Republic) keep their import, without operations.
ALTER TABLE investment_transaction
    ADD COLUMN import_batch_id INTEGER REFERENCES import_batch (id) ON DELETE SET NULL;
ALTER TABLE investment_transaction ADD COLUMN source_page INTEGER;

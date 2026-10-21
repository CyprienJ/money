-- Optional fee and state columns of a CSV format (exports such as Revolut's):
-- the fee is taken off the amount, and only lines whose state equals `state_keep` are read.
-- `column_state` and `state_keep` are set together, which the application checks.
ALTER TABLE csv_mapping ADD COLUMN column_fee INTEGER CHECK (column_fee >= 0);
ALTER TABLE csv_mapping ADD COLUMN column_state INTEGER CHECK (column_state >= 0);
ALTER TABLE csv_mapping ADD COLUMN state_keep TEXT;

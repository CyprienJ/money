-- The opening date of an account becomes optional: without it, the account counts from its
-- first operation. SQLite cannot drop a NOT NULL constraint, and rebuilding the table would
-- cascade the deletion to its operations: the column is replaced instead.
ALTER TABLE account ADD COLUMN opened_on TEXT
    CHECK (opened_on GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
UPDATE account SET opened_on = opening_date;
ALTER TABLE account DROP COLUMN opening_date;
ALTER TABLE account RENAME COLUMN opened_on TO opening_date;

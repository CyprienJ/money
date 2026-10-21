-- Payees (V2): each normalized label is an alias of exactly one payee; several labels can be
-- merged into one payee, which the user can rename.
-- `operation.payee_id` mirrors the payee of the operation's normalized label. It was created
-- without a foreign key in 0001; the application keeps it consistent, and fills it for existing
-- operations at startup.

CREATE TABLE payee (
    id   INTEGER PRIMARY KEY,
    name TEXT    NOT NULL CHECK (name <> '')
);

CREATE UNIQUE INDEX payee_unique_name ON payee (name COLLATE NOCASE);

CREATE TABLE payee_alias (
    normalized_label TEXT    PRIMARY KEY,
    payee_id         INTEGER NOT NULL REFERENCES payee (id) ON DELETE CASCADE
);

CREATE INDEX payee_alias_payee ON payee_alias (payee_id);
CREATE INDEX operation_payee ON operation (payee_id);

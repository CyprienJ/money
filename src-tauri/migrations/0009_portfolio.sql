-- Detailed portfolio (V2): securities and the transactions of investment accounts. Quantities
-- are stored in millionths of a unit (fractional shares); amounts in cents. The value of an
-- account and the net worth still come from its valuations.

CREATE TABLE security (
    id       INTEGER PRIMARY KEY,
    name     TEXT    NOT NULL,
    -- 2 letters, 9 letters or digits, 1 check digit
    isin     TEXT    UNIQUE
        CHECK (isin IS NULL OR (length(isin) = 12 AND isin GLOB '[A-Z][A-Z]*[0-9]' AND isin NOT GLOB '*[^A-Z0-9]*')),
    currency TEXT    NOT NULL DEFAULT 'EUR' -- planned (multi-currency)
);

CREATE TABLE investment_transaction (
    id             INTEGER PRIMARY KEY,
    account_id     INTEGER NOT NULL REFERENCES account (id) ON DELETE RESTRICT,
    security_id    INTEGER NOT NULL REFERENCES security (id) ON DELETE RESTRICT,
    date           TEXT    NOT NULL
        CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    kind           TEXT    NOT NULL CHECK (kind IN ('BUY', 'SELL', 'DIVIDEND')),
    -- Millionths of a unit; 0 for a dividend.
    quantity_micro INTEGER NOT NULL CHECK (quantity_micro >= 0),
    -- Quantity × unit price for a buy or a sell, amount received for a dividend.
    amount_cents   INTEGER NOT NULL CHECK (amount_cents > 0),
    fees_cents     INTEGER NOT NULL DEFAULT 0 CHECK (fees_cents >= 0),
    note           TEXT
);

CREATE INDEX investment_transaction_account ON investment_transaction (account_id, date);
CREATE INDEX investment_transaction_security ON investment_transaction (security_id);

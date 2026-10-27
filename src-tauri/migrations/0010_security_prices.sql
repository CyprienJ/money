-- Security prices (V2): daily prices entered by hand or fetched from Yahoo Finance, the only
-- network access of the application, opt-in. A price entered by hand is never overwritten.

ALTER TABLE security ADD COLUMN symbol TEXT; -- Yahoo Finance symbol, e.g. CW8.PA

CREATE TABLE security_price (
    security_id INTEGER NOT NULL REFERENCES security (id) ON DELETE CASCADE,
    date        TEXT    NOT NULL CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    price_cents INTEGER NOT NULL CHECK (price_cents > 0),
    source      TEXT    NOT NULL CHECK (source IN ('MANUAL', 'YAHOO')),
    PRIMARY KEY (security_id, date)
);

-- Application settings kept with the data, by key.
CREATE TABLE app_setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

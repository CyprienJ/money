-- Multi-currency (V2): the currency of an account is chosen in its form, its operations take
-- it, and the statistics convert every amount to euros, the reference currency, with the
-- exchange rates below (entered by hand, or fetched with the security prices).

-- Euros for one unit of the currency on a day, in billionths (no floating point).
CREATE TABLE exchange_rate (
    currency  TEXT    NOT NULL CHECK (currency GLOB '[A-Z][A-Z][A-Z]' AND currency <> 'EUR'),
    date      TEXT    NOT NULL CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    rate_nano INTEGER NOT NULL CHECK (rate_nano > 0),
    source    TEXT    NOT NULL CHECK (source IN ('MANUAL', 'YAHOO')),
    PRIMARY KEY (currency, date)
);

-- The currency of an operation is the one of its account.
CREATE TRIGGER operation_currency_on_insert AFTER INSERT ON operation
BEGIN
    UPDATE operation
    SET currency = (SELECT currency FROM account WHERE id = NEW.account_id)
    WHERE id = NEW.id;
END;

CREATE TRIGGER operation_currency_on_account_change AFTER UPDATE OF currency ON account
BEGIN
    UPDATE operation SET currency = NEW.currency WHERE account_id = NEW.id;
END;

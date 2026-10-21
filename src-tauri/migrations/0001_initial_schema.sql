-- Initial V1 schema (see README, "Modèle de données").
-- Conventions: amounts in integer cents (negative = debit), dates as TEXT 'YYYY-MM-DD',
-- timestamps as TEXT 'YYYY-MM-DDTHH:MM:SSZ' (UTC), enums as TEXT with CHECK, booleans as 0/1.
-- Columns marked "planned" are created now but only used in later versions.

CREATE TABLE account (
    id                    INTEGER PRIMARY KEY,
    name                  TEXT    NOT NULL,
    type                  TEXT    NOT NULL
        CHECK (type IN ('CHECKING', 'SAVINGS', 'CREDIT_CARD', 'INVESTMENT', 'ASSET', 'LIABILITY')),
    currency              TEXT    NOT NULL DEFAULT 'EUR', -- planned (multi-currency)
    institution           TEXT,
    opening_balance_cents INTEGER NOT NULL DEFAULT 0,
    opening_date          TEXT    NOT NULL
        CHECK (opening_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    archived              INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1))
);

CREATE TABLE category (
    id                   INTEGER PRIMARY KEY,
    name                 TEXT    NOT NULL,
    -- Two levels at most, enforced by the application.
    parent_id            INTEGER REFERENCES category (id) ON DELETE RESTRICT,
    -- Color family of the design tokens (--cat-<color>).
    color                TEXT    NOT NULL DEFAULT 'neutral'
        CHECK (color IN ('housing', 'groceries', 'restaurants', 'transport', 'leisure',
                         'subscriptions', 'health', 'neutral')),
    is_internal_transfer INTEGER NOT NULL DEFAULT 0 CHECK (is_internal_transfer IN (0, 1)),
    is_income            INTEGER NOT NULL DEFAULT 0 CHECK (is_income IN (0, 1))
);

-- Sibling names are unique, including at the top level (NULL parent).
CREATE UNIQUE INDEX category_unique_name ON category (IFNULL(parent_id, 0), name);
-- A single system "internal transfer" category.
CREATE UNIQUE INDEX category_single_internal_transfer ON category (is_internal_transfer)
    WHERE is_internal_transfer = 1;

CREATE TABLE import_batch (
    id                            INTEGER PRIMARY KEY,
    account_id                    INTEGER NOT NULL REFERENCES account (id) ON DELETE RESTRICT,
    source_type                   TEXT    NOT NULL CHECK (source_type IN ('CSV', 'PDF')),
    source_file_name              TEXT    NOT NULL,
    source_file_hash              TEXT    NOT NULL,
    imported_at                   TEXT    NOT NULL,
    statement_start_balance_cents INTEGER,
    statement_end_balance_cents   INTEGER,
    balance_check_status          TEXT    NOT NULL
        CHECK (balance_check_status IN ('OK', 'MISMATCH', 'NOT_AVAILABLE'))
);

CREATE INDEX import_batch_account ON import_batch (account_id);
CREATE INDEX import_batch_file_hash ON import_batch (source_file_hash);

CREATE TABLE operation (
    id                   INTEGER PRIMARY KEY,
    account_id           INTEGER NOT NULL REFERENCES account (id) ON DELETE RESTRICT,
    date                 TEXT    NOT NULL
        CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    value_date           TEXT
        CHECK (value_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    amount_cents         INTEGER NOT NULL,
    currency             TEXT    NOT NULL DEFAULT 'EUR', -- planned (multi-currency)
    raw_label            TEXT    NOT NULL,               -- original bank label, never modified
    normalized_label     TEXT    NOT NULL,               -- used for categorization and payees
    payee_id             INTEGER,                        -- planned (V2 payees), no table yet
    import_batch_id      INTEGER REFERENCES import_batch (id) ON DELETE CASCADE, -- NULL for manual entries
    is_internal_transfer INTEGER NOT NULL DEFAULT 0 CHECK (is_internal_transfer IN (0, 1)),
    transfer_pair_id     INTEGER REFERENCES operation (id) ON DELETE SET NULL, -- planned (V2 pairing)
    note                 TEXT,                           -- planned (V2 notes)
    created_at           TEXT    NOT NULL,
    updated_at           TEXT    NOT NULL
);

CREATE INDEX operation_account_date ON operation (account_id, date);
CREATE INDEX operation_normalized_label ON operation (normalized_label);
CREATE INDEX operation_import_batch ON operation (import_batch_id);
-- Duplicate detection: same account, date, amount and raw label.
CREATE INDEX operation_duplicate_key ON operation (account_id, date, amount_cents, raw_label);

-- One row per operation in V1; several rows once splitting is supported.
CREATE TABLE operation_split (
    id           INTEGER PRIMARY KEY,
    operation_id INTEGER NOT NULL REFERENCES operation (id) ON DELETE CASCADE,
    category_id  INTEGER REFERENCES category (id) ON DELETE RESTRICT, -- NULL = to categorize
    amount_cents INTEGER NOT NULL
);

CREATE INDEX operation_split_operation ON operation_split (operation_id);
CREATE INDEX operation_split_category ON operation_split (category_id);

CREATE TABLE csv_mapping (
    id                INTEGER PRIMARY KEY,
    name              TEXT    NOT NULL UNIQUE,
    delimiter         TEXT    NOT NULL,
    encoding          TEXT    NOT NULL,
    date_format       TEXT    NOT NULL,
    decimal_separator TEXT    NOT NULL,
    -- Zero-based column indexes.
    column_date       INTEGER NOT NULL,
    column_label      INTEGER NOT NULL,
    column_amount     INTEGER,
    column_debit      INTEGER,
    column_credit     INTEGER,
    skip_lines        INTEGER NOT NULL DEFAULT 0 CHECK (skip_lines >= 0),
    -- Optional fee column, taken off the amount (exports such as Revolut's).
    column_fee        INTEGER CHECK (column_fee >= 0),
    -- Optional state column: only lines whose state equals `state_keep` are read.
    column_state      INTEGER CHECK (column_state >= 0),
    state_keep        TEXT,
    -- Either a single signed amount column, or separate debit and credit columns.
    CHECK (column_amount IS NOT NULL OR (column_debit IS NOT NULL AND column_credit IS NOT NULL)),
    CHECK ((column_state IS NULL) = (state_keep IS NULL))
);

-- Automatic category reuse: last category given to a normalized label.
CREATE TABLE label_category_memory (
    normalized_label TEXT    PRIMARY KEY,
    category_id      INTEGER NOT NULL REFERENCES category (id) ON DELETE CASCADE,
    last_used_at     TEXT    NOT NULL
);

-- Value history of INVESTMENT / ASSET accounts, and outstanding capital of LIABILITY accounts
-- (stored as a positive amount, subtracted in net worth).
CREATE TABLE valuation (
    id          INTEGER PRIMARY KEY,
    account_id  INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    date        TEXT    NOT NULL
        CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    value_cents INTEGER NOT NULL,
    UNIQUE (account_id, date)
);

-- Recurring operations, display only in V1.
CREATE TABLE scheduled_operation (
    id            INTEGER PRIMARY KEY,
    account_id    INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    label         TEXT    NOT NULL,
    amount_cents  INTEGER NOT NULL,
    category_id   INTEGER REFERENCES category (id) ON DELETE SET NULL,
    frequency     TEXT    NOT NULL
        CHECK (frequency IN ('WEEKLY', 'MONTHLY', 'QUARTERLY', 'SEMIANNUAL', 'YEARLY')),
    day_of_period INTEGER NOT NULL CHECK (day_of_period >= 1),
    start_date    TEXT    NOT NULL
        CHECK (start_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    end_date      TEXT
        CHECK (end_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    CHECK (end_date IS NULL OR end_date >= start_date)
);

CREATE INDEX scheduled_operation_account ON scheduled_operation (account_id);

-- System category, excluded from income and expense statistics.
INSERT INTO category (name, color, is_internal_transfer, is_income)
VALUES ('Virement interne', 'neutral', 1, 0);

-- Default category tree, created on the first launch. Subcategories inherit the color
-- and the income flag of their top-level category.

INSERT INTO category (name, color, is_income) VALUES
    ('Logement', 'housing', 0),
    ('Courses', 'groceries', 0),
    ('Restaurants', 'restaurants', 0),
    ('Transport', 'transport', 0),
    ('Loisirs', 'leisure', 0),
    ('Abonnements', 'subscriptions', 0),
    ('Santé', 'health', 0),
    ('Revenus', 'transport', 1),
    ('Autres', 'neutral', 0);

INSERT INTO category (name, parent_id, color, is_income)
SELECT child.name, parent.id, parent.color, parent.is_income
FROM (
              SELECT 'Logement' AS parent, 'Loyer ou crédit' AS name, 1 AS position
    UNION ALL SELECT 'Logement', 'Charges et énergie', 2
    UNION ALL SELECT 'Logement', 'Assurance habitation', 3
    UNION ALL SELECT 'Logement', 'Travaux et équipement', 4
    UNION ALL SELECT 'Courses', 'Alimentation', 1
    UNION ALL SELECT 'Courses', 'Maison et hygiène', 2
    UNION ALL SELECT 'Restaurants', 'Restaurants', 1
    UNION ALL SELECT 'Restaurants', 'Cafés et snacks', 2
    UNION ALL SELECT 'Restaurants', 'Livraison', 3
    UNION ALL SELECT 'Transport', 'Carburant', 1
    UNION ALL SELECT 'Transport', 'Transports en commun', 2
    UNION ALL SELECT 'Transport', 'Train et avion', 3
    UNION ALL SELECT 'Transport', 'Entretien du véhicule', 4
    UNION ALL SELECT 'Transport', 'Péages et parking', 5
    UNION ALL SELECT 'Loisirs', 'Sorties et culture', 1
    UNION ALL SELECT 'Loisirs', 'Sport', 2
    UNION ALL SELECT 'Loisirs', 'Voyages', 3
    UNION ALL SELECT 'Loisirs', 'Shopping', 4
    UNION ALL SELECT 'Loisirs', 'Cadeaux', 5
    UNION ALL SELECT 'Abonnements', 'Téléphone et internet', 1
    UNION ALL SELECT 'Abonnements', 'Streaming', 2
    UNION ALL SELECT 'Abonnements', 'Logiciels et presse', 3
    UNION ALL SELECT 'Santé', 'Médecin', 1
    UNION ALL SELECT 'Santé', 'Pharmacie', 2
    UNION ALL SELECT 'Santé', 'Mutuelle', 3
    UNION ALL SELECT 'Revenus', 'Salaire', 1
    UNION ALL SELECT 'Revenus', 'Remboursements', 2
    UNION ALL SELECT 'Revenus', 'Intérêts', 3
    UNION ALL SELECT 'Revenus', 'Autres revenus', 4
    UNION ALL SELECT 'Autres', 'Impôts et taxes', 1
    UNION ALL SELECT 'Autres', 'Frais bancaires', 2
    UNION ALL SELECT 'Autres', 'Retraits d''espèces', 3
    UNION ALL SELECT 'Autres', 'Divers', 4
) AS child
JOIN category parent ON parent.name = child.parent AND parent.parent_id IS NULL
ORDER BY parent.id, child.position;

-- Alerts (V2) are computed on demand. Dismissed alerts are kept here so that they are not shown
-- again: an unusual expense by top-level category and month (first day of the month), a missing
-- occurrence by scheduled operation and occurrence date. A negative forecast cannot be dismissed.

CREATE TABLE unusual_expense_dismissal (
    category_id INTEGER NOT NULL REFERENCES category (id) ON DELETE CASCADE,
    month       TEXT    NOT NULL CHECK (month GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-01'),
    PRIMARY KEY (category_id, month)
);

CREATE TABLE missing_occurrence_dismissal (
    scheduled_operation_id INTEGER NOT NULL REFERENCES scheduled_operation (id) ON DELETE CASCADE,
    occurrence_date        TEXT    NOT NULL
        CHECK (occurrence_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    PRIMARY KEY (scheduled_operation_id, occurrence_date)
);

-- Recurring operation detection (V2): series of regular operations of a payee that the schedule
-- does not cover are suggested as scheduled operations. Dismissed suggestions are kept here, by
-- account, payee and direction, so that they are not suggested again.

CREATE TABLE recurring_dismissal (
    account_id INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    payee_id   INTEGER NOT NULL REFERENCES payee (id) ON DELETE CASCADE,
    is_credit  INTEGER NOT NULL CHECK (is_credit IN (0, 1)),
    PRIMARY KEY (account_id, payee_id, is_credit)
);

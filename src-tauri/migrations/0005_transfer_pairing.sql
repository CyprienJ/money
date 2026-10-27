-- Internal transfer pairing (V2): `operation.transfer_pair_id` (created in 0001) links a debit
-- on one account to the matching credit on another, both ways. Linked operations are in the
-- internal transfer category.

-- Proposed pairs the user dismissed, so that they are not proposed again.
CREATE TABLE transfer_dismissal (
    debit_id  INTEGER NOT NULL REFERENCES operation (id) ON DELETE CASCADE,
    credit_id INTEGER NOT NULL REFERENCES operation (id) ON DELETE CASCADE,
    PRIMARY KEY (debit_id, credit_id)
);

CREATE INDEX transfer_dismissal_credit ON transfer_dismissal (credit_id);
-- Pairing looks for opposite amounts.
CREATE INDEX operation_amount ON operation (amount_cents);
CREATE INDEX operation_transfer_pair ON operation (transfer_pair_id);

-- Schedule matching (V2): an imported or manual operation realizes one occurrence of a
-- scheduled operation. Occurrences are computed, so they are identified by their date.

CREATE TABLE schedule_match (
    scheduled_operation_id INTEGER NOT NULL REFERENCES scheduled_operation (id) ON DELETE CASCADE,
    occurrence_date        TEXT    NOT NULL
        CHECK (occurrence_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    operation_id           INTEGER NOT NULL UNIQUE REFERENCES operation (id) ON DELETE CASCADE,
    PRIMARY KEY (scheduled_operation_id, occurrence_date)
);

-- Proposed matches the user dismissed, so that they are not proposed again.
CREATE TABLE schedule_match_dismissal (
    scheduled_operation_id INTEGER NOT NULL REFERENCES scheduled_operation (id) ON DELETE CASCADE,
    occurrence_date        TEXT    NOT NULL,
    operation_id           INTEGER NOT NULL REFERENCES operation (id) ON DELETE CASCADE,
    PRIMARY KEY (scheduled_operation_id, occurrence_date, operation_id)
);

CREATE INDEX schedule_match_dismissal_operation ON schedule_match_dismissal (operation_id);

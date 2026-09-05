-- no-transaction
--
-- Enforce internally consistent allocation dates at the database boundary.
-- SQLite cannot add a CHECK to an existing table, so allocations is rebuilt
-- with the same columns and constraints plus the date checks.

PRAGMA foreign_keys = OFF;

CREATE TABLE allocations_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    resident_id INTEGER NOT NULL,
    room_id INTEGER NOT NULL,
    check_in_date DATE NOT NULL,
    check_out_date DATE,
    expected_checkout DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    rate_agreed TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    FOREIGN KEY (resident_id) REFERENCES residents(id) ON DELETE RESTRICT,
    FOREIGN KEY (room_id) REFERENCES rooms(id) ON DELETE RESTRICT,
    CHECK (status IN ('active', 'checked_out')),
    CHECK (expected_checkout >= check_in_date),
    CHECK (check_out_date IS NULL OR check_out_date >= check_in_date),
    CHECK (
        rate_agreed GLOB '*[0-9]*'
        AND rate_agreed NOT GLOB '*[^0-9.]*'
        AND length(rate_agreed) - length(replace(rate_agreed, '.', '')) <= 1
        AND substr(rate_agreed, 1, 1) GLOB '[0-9.]'
        AND rate_agreed NOT GLOB '*-*'
        AND rate_agreed NOT GLOB '*+*'
        AND replace(replace(rate_agreed, '.', ''), '0', '') <> ''
    )
);

INSERT INTO allocations_new (
    id, hostel_id, resident_id, room_id, check_in_date, check_out_date,
    expected_checkout, status, rate_agreed, created_at
)
SELECT
    id, hostel_id, resident_id, room_id, check_in_date, check_out_date,
    expected_checkout, status, rate_agreed, created_at
FROM allocations;

DROP TABLE allocations;
ALTER TABLE allocations_new RENAME TO allocations;

CREATE INDEX idx_allocations_hostel_status
    ON allocations(hostel_id, status);
CREATE INDEX idx_allocations_resident
    ON allocations(resident_id);
CREATE INDEX idx_allocations_room_active
    ON allocations(room_id, hostel_id, status);

PRAGMA foreign_keys = ON;

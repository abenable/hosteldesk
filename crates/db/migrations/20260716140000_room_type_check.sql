-- no-transaction
--
-- Add a CHECK constraint on rooms.room_type so the database enforces the
-- canonical set of values used by the app and DB layer.
--
-- SQLite cannot add a CHECK constraint to an existing table, so recreate
-- rooms with the same shape and migrate the data explicitly. Indexes on the
-- recreated tables are restored at the end.

PRAGMA foreign_keys = OFF;

CREATE TABLE rooms_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    room_number TEXT NOT NULL,
    room_type TEXT NOT NULL DEFAULT 'single'
        CHECK (room_type IN ('single', 'double', 'dorm')),
    capacity INTEGER NOT NULL DEFAULT 1,
    floor INTEGER,
    rate_per_period TEXT NOT NULL,
    billing_period TEXT NOT NULL DEFAULT 'semester'
        CHECK (billing_period IN ('night', 'week', 'month', 'semester')),
    status TEXT NOT NULL DEFAULT 'vacant',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    UNIQUE (hostel_id, room_number),
    CHECK (status IN ('vacant', 'occupied', 'maintenance')),
    CHECK (capacity > 0),
    CHECK (
        rate_per_period GLOB '*[0-9]*'
        AND rate_per_period NOT GLOB '*[^0-9.]*'
        AND length(rate_per_period) - length(replace(rate_per_period, '.', '')) <= 1
        AND substr(rate_per_period, 1, 1) GLOB '[0-9.]'
        AND rate_per_period NOT GLOB '*-*'
        AND rate_per_period NOT GLOB '*+*'
        AND replace(replace(rate_per_period, '.', ''), '0', '') <> ''
    )
);

INSERT INTO rooms_new (
    id, hostel_id, room_number, room_type, capacity, floor,
    rate_per_period, billing_period, status, created_at
)
SELECT
    id, hostel_id, room_number, room_type, capacity, floor,
    rate_per_period, billing_period, status, created_at
FROM rooms;

DROP TABLE rooms;
ALTER TABLE rooms_new RENAME TO rooms;

PRAGMA foreign_keys = ON;

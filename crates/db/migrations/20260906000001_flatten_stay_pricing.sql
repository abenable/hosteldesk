-- A stay is a flat one-time sale (rate_agreed), not a per-day accrual, and
-- billing never actually used the stay dates for real date math (it only
-- gated on whether the stay had started, which is now always true since
-- check-in always happens today). Drop the dates and the checks tied to
-- them; `updated_at` replaces check_out_date for knowing when a stay closed.
CREATE TABLE allocations_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    resident_id INTEGER NOT NULL,
    room_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    rate_agreed TEXT NOT NULL,
    term TEXT NOT NULL CHECK (term IN ('semester_1', 'semester_2', 'recess')),
    academic_year TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    FOREIGN KEY (resident_id) REFERENCES residents(id) ON DELETE RESTRICT,
    FOREIGN KEY (room_id) REFERENCES rooms(id) ON DELETE RESTRICT,
    CHECK (status IN ('active', 'checked_out')),
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

INSERT INTO allocations_new (id, hostel_id, resident_id, room_id, status, rate_agreed, term, academic_year, created_at, updated_at)
SELECT id, hostel_id, resident_id, room_id, status, rate_agreed, term, academic_year, created_at, created_at
FROM allocations;

DROP TABLE allocations;
ALTER TABLE allocations_new RENAME TO allocations;

CREATE INDEX IF NOT EXISTS idx_allocations_hostel_status
    ON allocations(hostel_id, status);

CREATE INDEX IF NOT EXISTS idx_allocations_resident
    ON allocations(resident_id);

CREATE INDEX IF NOT EXISTS idx_allocations_room_active
    ON allocations(room_id, hostel_id, status);

-- no-transaction
--
-- Fix incorrect GLOB negation in the decimal CHECK constraints added by
-- migration 20260706000001. SQLite GLOB uses `[^...]` for negation, not
-- `[!...]`; the original `NOT GLOB '*[!0-9.+-]*'` treated `!` as a literal
-- class member, so the clause matched (and the CHECK rejected) every valid
-- decimal string. Tables are re-created in place using the same rename/drop
-- pattern as 20260706000001; foreign keys are disabled for the rewrite.

PRAGMA foreign_keys = OFF;

ALTER TABLE rooms RENAME TO old_rooms;
ALTER TABLE allocations RENAME TO old_allocations;
ALTER TABLE payments RENAME TO old_payments;
ALTER TABLE expenses RENAME TO old_expenses;

CREATE TABLE rooms (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    room_number TEXT NOT NULL,
    room_type TEXT NOT NULL DEFAULT 'single',
    capacity INTEGER NOT NULL DEFAULT 1,
    floor INTEGER,
    rate_per_period TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'vacant',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    UNIQUE (hostel_id, room_number),
    CHECK (status IN ('vacant', 'occupied', 'maintenance')),
    CHECK (capacity > 0),
    CHECK (
        rate_per_period GLOB '*[0-9]*'
        AND rate_per_period NOT GLOB '*[^0-9.+-]*'
        AND length(rate_per_period) - length(replace(rate_per_period, '.', '')) <= 1
        AND substr(rate_per_period, 1, 1) GLOB '[0-9.+-]'
    )
);

INSERT INTO rooms SELECT * FROM old_rooms;

CREATE TABLE allocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    resident_id INTEGER NOT NULL,
    room_id INTEGER NOT NULL,
    bed_id INTEGER,
    check_in_date DATE NOT NULL,
    check_out_date DATE,
    expected_checkout DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    rate_agreed TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    FOREIGN KEY (resident_id) REFERENCES residents(id) ON DELETE RESTRICT,
    FOREIGN KEY (room_id) REFERENCES rooms(id) ON DELETE RESTRICT,
    FOREIGN KEY (bed_id) REFERENCES beds(id) ON DELETE SET NULL,
    CHECK (status IN ('active', 'checked_out')),
    CHECK (
        rate_agreed GLOB '*[0-9]*'
        AND rate_agreed NOT GLOB '*[^0-9.+-]*'
        AND length(rate_agreed) - length(replace(rate_agreed, '.', '')) <= 1
        AND substr(rate_agreed, 1, 1) GLOB '[0-9.+-]'
    )
);

INSERT INTO allocations SELECT * FROM old_allocations;

CREATE TABLE payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    allocation_id INTEGER NOT NULL,
    resident_id INTEGER NOT NULL,
    amount TEXT NOT NULL,
    payment_date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    payment_method TEXT NOT NULL,
    period_start DATE,
    period_end DATE,
    notes TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    FOREIGN KEY (allocation_id) REFERENCES allocations(id) ON DELETE RESTRICT,
    FOREIGN KEY (resident_id) REFERENCES residents(id) ON DELETE RESTRICT,
    CHECK (payment_method IN ('cash', 'mobile_money', 'bank_transfer', 'card')),
    CHECK (
        amount GLOB '*[0-9]*'
        AND amount NOT GLOB '*[^0-9.+-]*'
        AND length(amount) - length(replace(amount, '.', '')) <= 1
        AND substr(amount, 1, 1) GLOB '[0-9.+-]'
    )
);

INSERT INTO payments SELECT * FROM old_payments;

CREATE TABLE expenses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    category TEXT NOT NULL,
    amount TEXT NOT NULL,
    date DATE NOT NULL,
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    CHECK (
        amount GLOB '*[0-9]*'
        AND amount NOT GLOB '*[^0-9.+-]*'
        AND length(amount) - length(replace(amount, '.', '')) <= 1
        AND substr(amount, 1, 1) GLOB '[0-9.+-]'
    )
);

INSERT INTO expenses SELECT * FROM old_expenses;

DROP TABLE old_rooms;
DROP TABLE old_allocations;
DROP TABLE old_payments;
DROP TABLE old_expenses;

PRAGMA foreign_keys = ON;

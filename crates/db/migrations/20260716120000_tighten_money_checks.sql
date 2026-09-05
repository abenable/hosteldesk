-- no-transaction
--
-- Tighten CHECK constraints on money-holding columns so that negative, signed,
-- or zero decimal strings cannot be stored in the database.
--
-- Tables affected: rooms(rate_per_period), allocations(rate_agreed),
-- payments(amount), expenses(amount).
--
-- SQLite cannot drop an unnamed CHECK constraint, so each table is recreated
-- with the corrected constraint and its data is migrated explicitly.

PRAGMA foreign_keys = OFF;

-- rooms: rate_per_period must be a positive decimal without a sign.
CREATE TABLE rooms_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    room_number TEXT NOT NULL,
    room_type TEXT NOT NULL DEFAULT 'single',
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

-- allocations: rate_agreed must be a positive decimal without a sign.
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

-- payments: amount must be a positive decimal without a sign.
CREATE TABLE payments_new (
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
        AND amount NOT GLOB '*[^0-9.]*'
        AND length(amount) - length(replace(amount, '.', '')) <= 1
        AND substr(amount, 1, 1) GLOB '[0-9.]'
        AND amount NOT GLOB '*-*'
        AND amount NOT GLOB '*+*'
        AND replace(replace(amount, '.', ''), '0', '') <> ''
    )
);

INSERT INTO payments_new (
    id, hostel_id, allocation_id, resident_id, amount,
    payment_date, payment_method, period_start, period_end, notes, created_at
)
SELECT
    id, hostel_id, allocation_id, resident_id, amount,
    payment_date, payment_method, period_start, period_end, notes, created_at
FROM payments;

DROP TABLE payments;
ALTER TABLE payments_new RENAME TO payments;

-- expenses: amount must be a positive decimal without a sign.
CREATE TABLE expenses_new (
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
        AND amount NOT GLOB '*[^0-9.]*'
        AND length(amount) - length(replace(amount, '.', '')) <= 1
        AND substr(amount, 1, 1) GLOB '[0-9.]'
        AND amount NOT GLOB '*-*'
        AND amount NOT GLOB '*+*'
        AND replace(replace(amount, '.', ''), '0', '') <> ''
    )
);

INSERT INTO expenses_new (
    id, hostel_id, category, amount, date, description, created_at
)
SELECT
    id, hostel_id, category, amount, date, description, created_at
FROM expenses;

DROP TABLE expenses;
ALTER TABLE expenses_new RENAME TO expenses;

-- Re-create indexes that were dropped with the table recreations.
CREATE INDEX IF NOT EXISTS idx_allocations_hostel_status ON allocations(hostel_id, status);
CREATE INDEX IF NOT EXISTS idx_allocations_resident ON allocations(resident_id);
CREATE INDEX IF NOT EXISTS idx_payments_hostel_date ON payments(hostel_id, payment_date);
CREATE INDEX IF NOT EXISTS idx_payments_allocation ON payments(allocation_id);

PRAGMA foreign_keys = ON;

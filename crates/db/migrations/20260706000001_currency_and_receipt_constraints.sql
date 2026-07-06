-- no-transaction
--
-- Enforce valid decimal strings on monetary columns and positive integer receipt
-- numbers. Tables are re-created because SQLite cannot add CHECK constraints to
-- existing columns with ALTER TABLE. Foreign keys are disabled for the rewrite
-- so that tables can be recreated in place without cascading reference updates.

PRAGMA foreign_keys = OFF;

-- Rename the existing tables so we can recreate them with the same names.
ALTER TABLE rooms RENAME TO old_rooms;
ALTER TABLE allocations RENAME TO old_allocations;
ALTER TABLE payments RENAME TO old_payments;
ALTER TABLE receipts RENAME TO old_receipts;
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
        AND rate_per_period NOT GLOB '*[!0-9.+-]*'
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
        AND rate_agreed NOT GLOB '*[!0-9.+-]*'
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
        AND amount NOT GLOB '*[!0-9.+-]*'
        AND length(amount) - length(replace(amount, '.', '')) <= 1
        AND substr(amount, 1, 1) GLOB '[0-9.+-]'
    )
);

INSERT INTO payments SELECT * FROM old_payments;

CREATE TABLE receipts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    payment_id INTEGER NOT NULL,
    receipt_number TEXT NOT NULL,
    generated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    pdf_path TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    FOREIGN KEY (payment_id) REFERENCES payments(id) ON DELETE RESTRICT,
    UNIQUE (hostel_id, receipt_number),
    CHECK (
        receipt_number GLOB '[0-9]*'
        AND receipt_number <> ''
        AND CAST(receipt_number AS INTEGER) > 0
    )
);

INSERT INTO receipts SELECT * FROM old_receipts;

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
        AND amount NOT GLOB '*[!0-9.+-]*'
        AND length(amount) - length(replace(amount, '.', '')) <= 1
        AND substr(amount, 1, 1) GLOB '[0-9.+-]'
    )
);

INSERT INTO expenses SELECT * FROM old_expenses;

-- Drop the old tables after their replacements are populated.
DROP TABLE old_rooms;
DROP TABLE old_allocations;
DROP TABLE old_payments;
DROP TABLE old_receipts;
DROP TABLE old_expenses;

PRAGMA foreign_keys = ON;

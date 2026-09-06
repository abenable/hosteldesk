-- no-transaction
--
-- Initial Hostel Desk schema.
--
-- This is a squash of the pre-1.0 migration chain into one clean baseline.
-- The previous chain is safe to replace because no production/installed
-- databases exist in the wild.
--
-- All core tables carry a hostel_id to make future multi-property support a
-- non-breaking migration instead of a painful retrofit.
--
-- Monetary columns are stored as TEXT decimal strings and validated with GLOB
-- constraints (unsigned, at most one '.', not all-zero). Never use
-- REAL/floating-point for money.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS hostels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    address TEXT,
    phone TEXT,
    currency TEXT NOT NULL DEFAULT 'UGX',
    branding_logo_path TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS rooms (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    room_number TEXT NOT NULL,
    room_type TEXT NOT NULL DEFAULT 'single'
        CHECK (room_type IN ('single', 'double', 'dorm')),
    capacity INTEGER NOT NULL DEFAULT 1,
    floor INTEGER,
    semester_rate TEXT NOT NULL,
    recess_rate TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'vacant',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    UNIQUE (hostel_id, room_number),
    CHECK (status IN ('vacant', 'occupied', 'maintenance')),
    CHECK (capacity > 0),
    CHECK (
        semester_rate GLOB '*[0-9]*'
        AND semester_rate NOT GLOB '*[^0-9.]*'
        AND length(semester_rate) - length(replace(semester_rate, '.', '')) <= 1
        AND substr(semester_rate, 1, 1) GLOB '[0-9.]'
        AND semester_rate NOT GLOB '*-*'
        AND semester_rate NOT GLOB '*+*'
        AND replace(replace(semester_rate, '.', ''), '0', '') <> ''
    ),
    CHECK (
        recess_rate GLOB '*[0-9]*'
        AND recess_rate NOT GLOB '*[^0-9.]*'
        AND length(recess_rate) - length(replace(recess_rate, '.', '')) <= 1
        AND substr(recess_rate, 1, 1) GLOB '[0-9.]'
        AND recess_rate NOT GLOB '*-*'
        AND recess_rate NOT GLOB '*+*'
        AND replace(replace(recess_rate, '.', ''), '0', '') <> ''
    )
);

CREATE TABLE IF NOT EXISTS residents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    full_name TEXT NOT NULL,
    phone TEXT,
    email TEXT,
    id_number TEXT,
    next_of_kin TEXT,
    photo_path TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS allocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    resident_id INTEGER NOT NULL,
    room_id INTEGER NOT NULL,
    check_in_date DATE NOT NULL,
    check_out_date DATE,
    expected_checkout DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    rate_agreed TEXT NOT NULL,
    term TEXT NOT NULL CHECK (term IN ('semester_1', 'semester_2', 'recess')),
    academic_year TEXT NOT NULL,
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

CREATE TABLE IF NOT EXISTS payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    allocation_id INTEGER NOT NULL,
    resident_id INTEGER NOT NULL,
    amount TEXT NOT NULL,
    payment_date TIMESTAMP NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
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

CREATE TABLE IF NOT EXISTS receipts (
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

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    full_name TEXT NOT NULL,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'manager',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    UNIQUE (hostel_id, email),
    CHECK (role IN ('admin', 'manager'))
);

CREATE TABLE IF NOT EXISTS app_preferences (
    hostel_id INTEGER PRIMARY KEY,
    date_format TEXT NOT NULL DEFAULT 'dd/MM/yyyy',
    language TEXT NOT NULL DEFAULT 'en',
    receipt_footer TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    CHECK (date_format IN ('dd/MM/yyyy', 'MM/dd/yyyy', 'yyyy-MM-dd')),
    CHECK (language IN ('en'))
);

CREATE INDEX IF NOT EXISTS idx_allocations_hostel_status
    ON allocations(hostel_id, status);

CREATE INDEX IF NOT EXISTS idx_allocations_resident
    ON allocations(resident_id);

CREATE INDEX IF NOT EXISTS idx_allocations_room_active
    ON allocations(room_id, hostel_id, status);

CREATE INDEX IF NOT EXISTS idx_payments_hostel_date
    ON payments(hostel_id, payment_date);

CREATE INDEX IF NOT EXISTS idx_payments_allocation
    ON payments(allocation_id);

-- Covers get_resident_detail's per-resident payment lookup. Without it,
-- SQLite falls back to idx_payments_hostel_date and scans every payment row
-- in the hostel (measured 65ms -> 0.16ms at 100k payments with this index).
CREATE INDEX IF NOT EXISTS idx_payments_hostel_resident
    ON payments(hostel_id, resident_id, payment_date);

CREATE INDEX IF NOT EXISTS idx_receipts_payment
    ON receipts(payment_id);

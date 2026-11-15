-- Public ids, resident tenancy, and the local sync queue.
-- Integer primary keys stay as local foreign keys. The wire format uses public_id.

ALTER TABLE hostels ADD COLUMN public_id TEXT NOT NULL DEFAULT '';
ALTER TABLE hostels ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE rooms ADD COLUMN public_id TEXT NOT NULL DEFAULT '';
ALTER TABLE rooms ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE allocations ADD COLUMN public_id TEXT NOT NULL DEFAULT '';
ALTER TABLE allocations ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE payments ADD COLUMN public_id TEXT NOT NULL DEFAULT '';
ALTER TABLE payments ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE app_preferences ADD COLUMN public_id TEXT NOT NULL DEFAULT '';
ALTER TABLE app_preferences ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;

ALTER TABLE residents ADD COLUMN hostel_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE residents ADD COLUMN public_id TEXT NOT NULL DEFAULT '';
ALTER TABLE residents ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;

UPDATE residents
SET hostel_id = (
    SELECT a.hostel_id
    FROM allocations a
    WHERE a.resident_id = residents.id
    ORDER BY a.id
    LIMIT 1
)
WHERE hostel_id IS NULL;

UPDATE residents
SET hostel_id = (SELECT id FROM hostels ORDER BY id LIMIT 1)
WHERE hostel_id IS NULL;

UPDATE hostels
SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
WHERE public_id = '';
UPDATE rooms
SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
WHERE public_id = '';
UPDATE allocations
SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
WHERE public_id = '';
UPDATE payments
SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
WHERE public_id = '';
UPDATE app_preferences
SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
WHERE public_id = '';
UPDATE residents
SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
WHERE public_id = '';

CREATE UNIQUE INDEX idx_residents_public_id ON residents(public_id);
CREATE INDEX idx_residents_hostel ON residents(hostel_id);

CREATE UNIQUE INDEX idx_hostels_public_id ON hostels(public_id);
CREATE UNIQUE INDEX idx_rooms_public_id ON rooms(public_id);
CREATE UNIQUE INDEX idx_allocations_public_id ON allocations(public_id);
CREATE UNIQUE INDEX idx_payments_public_id ON payments(public_id);
CREATE UNIQUE INDEX idx_app_preferences_public_id ON app_preferences(public_id);

CREATE TABLE receipts_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    payment_id INTEGER NOT NULL,
    receipt_number TEXT NOT NULL,
    generated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    pdf_path TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    public_id TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    FOREIGN KEY (payment_id) REFERENCES payments(id) ON DELETE RESTRICT,
    UNIQUE (hostel_id, receipt_number),
    CHECK (
        length(receipt_number) > 0
        AND receipt_number NOT GLOB '*[^0-9A-Za-z-]*'
        AND receipt_number GLOB '*[0-9]*'
    )
);

INSERT INTO receipts_new (
    id, hostel_id, payment_id, receipt_number, generated_at, pdf_path, created_at, public_id, revision
)
SELECT
    id, hostel_id, payment_id, receipt_number, generated_at, pdf_path, created_at,
    lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6))),
    1
FROM receipts;

DROP TABLE receipts;
ALTER TABLE receipts_new RENAME TO receipts;

CREATE INDEX idx_receipts_payment ON receipts(payment_id);
CREATE UNIQUE INDEX idx_receipts_public_id ON receipts(public_id);

CREATE TRIGGER hostels_public_id
AFTER INSERT ON hostels
WHEN NEW.public_id = ''
BEGIN
    UPDATE hostels
    SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
    WHERE id = NEW.id AND public_id = '';
END;

CREATE TRIGGER rooms_public_id
AFTER INSERT ON rooms
WHEN NEW.public_id = ''
BEGIN
    UPDATE rooms
    SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
    WHERE id = NEW.id AND public_id = '';
END;

CREATE TRIGGER residents_public_id
AFTER INSERT ON residents
WHEN NEW.public_id = ''
BEGIN
    UPDATE residents
    SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
    WHERE id = NEW.id AND public_id = '';
END;

CREATE TRIGGER allocations_public_id
AFTER INSERT ON allocations
WHEN NEW.public_id = ''
BEGIN
    UPDATE allocations
    SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
    WHERE id = NEW.id AND public_id = '';
END;

CREATE TRIGGER payments_public_id
AFTER INSERT ON payments
WHEN NEW.public_id = ''
BEGIN
    UPDATE payments
    SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
    WHERE id = NEW.id AND public_id = '';
END;

CREATE TRIGGER receipts_public_id
AFTER INSERT ON receipts
WHEN NEW.public_id = ''
BEGIN
    UPDATE receipts
    SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
    WHERE id = NEW.id AND public_id = '';
END;

CREATE TRIGGER app_preferences_public_id
AFTER INSERT ON app_preferences
WHEN NEW.public_id = ''
BEGIN
    UPDATE app_preferences
    SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
    WHERE hostel_id = NEW.hostel_id AND public_id = '';
END;

CREATE TABLE sync_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    command_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'acked', 'conflict')),
    error TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE
);

CREATE INDEX idx_sync_outbox_pending ON sync_outbox(hostel_id, status, id);

CREATE TABLE sync_state (
    hostel_id INTEGER PRIMARY KEY,
    cursor TEXT NOT NULL DEFAULT '0',
    device_id TEXT,
    device_prefix TEXT,
    server_url TEXT,
    lease_text TEXT,
    next_receipt INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE
);

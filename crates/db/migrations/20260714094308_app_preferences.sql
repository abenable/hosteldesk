-- no-transaction
--
-- App-level preferences keyed by hostel.
--

PRAGMA foreign_keys = ON;

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

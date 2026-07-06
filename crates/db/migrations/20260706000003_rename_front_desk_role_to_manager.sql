-- no-transaction
--
-- Rename the staff_users role 'front_desk' to 'manager' to match the
-- onboarding UI's Admin/Manager vocabulary. SQLite cannot ALTER a CHECK
-- constraint, so staff_users is recreated in place. Foreign keys are off for
-- the rewrite. Existing 'front_desk' rows are migrated to 'manager' first.

PRAGMA foreign_keys = OFF;

UPDATE staff_users SET role = 'manager' WHERE role = 'front_desk';

ALTER TABLE staff_users RENAME TO old_staff_users;

CREATE TABLE staff_users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'manager',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    UNIQUE (hostel_id, username),
    CHECK (role IN ('admin', 'manager'))
);

INSERT INTO staff_users (id, hostel_id, name, username, password_hash, role, created_at)
SELECT id, hostel_id, name, username, password_hash, role, created_at FROM old_staff_users;

DROP TABLE old_staff_users;

PRAGMA foreign_keys = ON;

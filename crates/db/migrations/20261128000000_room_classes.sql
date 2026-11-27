-- no-transaction
--
-- Room classes. A room's type, floor, and prices move to a class its rooms
-- share, and beds (rooms.capacity) are dropped: a room holds any number of
-- residents. `rooms` is rebuilt and `allocations` references it, so foreign
-- keys are switched off around the rebuild (hence no-transaction).
--
-- Each (floor, type, semester rate, recess rate) of a hostel's rooms becomes
-- one class whose public id is spelled from those values, exactly as the sync
-- Worker's migration 0004 spells it for the same rooms, so both sides agree.
-- A linked desk also queues every class (a create, which the Worker treats as
-- a no-op when it already holds the same class) ahead of whatever was still
-- waiting to push, and rewrites its waiting room upserts to name the class.

PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE room_classes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    floor INTEGER,
    room_type TEXT NOT NULL CHECK (room_type IN ('single', 'double', 'dorm')),
    semester_rate TEXT NOT NULL,
    recess_rate TEXT NOT NULL,
    public_id TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
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

INSERT INTO room_classes (hostel_id, floor, room_type, semester_rate, recess_rate, public_id)
SELECT DISTINCT
    r.hostel_id, r.floor, r.room_type, r.semester_rate, r.recess_rate,
    'legacy:' || h.public_id || ':' || coalesce(CAST(r.floor AS TEXT), '') || ':'
        || r.room_type || ':' || r.semester_rate || ':' || r.recess_rate
FROM rooms r
JOIN hostels h ON h.id = r.hostel_id
ORDER BY r.hostel_id, r.floor, r.room_type, r.semester_rate, r.recess_rate;

CREATE TABLE rooms_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hostel_id INTEGER NOT NULL,
    room_number TEXT NOT NULL,
    class_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'vacant',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    public_id TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (hostel_id) REFERENCES hostels(id) ON DELETE CASCADE,
    FOREIGN KEY (class_id) REFERENCES room_classes(id) ON DELETE RESTRICT,
    UNIQUE (hostel_id, room_number),
    CHECK (status IN ('vacant', 'occupied', 'maintenance'))
);

INSERT INTO rooms_new (id, hostel_id, room_number, class_id, status, created_at, public_id, revision)
SELECT r.id, r.hostel_id, r.room_number, c.id, r.status, r.created_at, r.public_id, r.revision
FROM rooms r
JOIN room_classes c
    ON c.hostel_id = r.hostel_id
    AND c.floor IS r.floor
    AND c.room_type = r.room_type
    AND c.semester_rate = r.semester_rate
    AND c.recess_rate = r.recess_rate;

-- Waiting room upserts take the shape the Worker now accepts. Rooms are never
-- deleted, so every one of them finds its room (and so its class).
UPDATE sync_outbox
SET payload = json_set(
    json_remove(payload, '$.room_type', '$.capacity', '$.floor', '$.semester_rate', '$.recess_rate'),
    '$.class_public_id',
    (
        SELECT c.public_id
        FROM rooms_new r
        JOIN room_classes c ON c.id = r.class_id
        WHERE r.public_id = json_extract(sync_outbox.payload, '$.public_id')
    )
)
WHERE status = 'pending' AND command_type = 'upsert_room';

-- The classes go ahead of everything still waiting, which keeps its order.
CREATE TEMP TABLE waiting AS
SELECT hostel_id, idempotency_key, command_type, payload, status, error, created_at
FROM sync_outbox
WHERE status = 'pending'
ORDER BY id;

DELETE FROM sync_outbox WHERE status = 'pending';

INSERT INTO sync_outbox (hostel_id, idempotency_key, command_type, payload)
SELECT
    c.hostel_id,
    lower(hex(randomblob(16))),
    'upsert_room_class',
    json_object(
        'type', 'upsert_room_class',
        'public_id', c.public_id,
        'hostel_public_id', h.public_id,
        'base_revision', 0,
        'floor', c.floor,
        'room_type', c.room_type,
        'semester_rate', c.semester_rate,
        'recess_rate', c.recess_rate
    )
FROM room_classes c
JOIN hostels h ON h.id = c.hostel_id
JOIN sync_state s ON s.hostel_id = c.hostel_id AND s.device_id IS NOT NULL
ORDER BY c.id;

INSERT INTO sync_outbox (hostel_id, idempotency_key, command_type, payload, status, error, created_at)
SELECT hostel_id, idempotency_key, command_type, payload, status, error, created_at
FROM waiting
ORDER BY rowid;

DROP TABLE waiting;

DROP TABLE rooms;
ALTER TABLE rooms_new RENAME TO rooms;

CREATE UNIQUE INDEX idx_rooms_public_id ON rooms(public_id);
CREATE INDEX idx_rooms_class ON rooms(class_id);
CREATE UNIQUE INDEX idx_room_classes_public_id ON room_classes(public_id);
CREATE INDEX idx_room_classes_hostel ON room_classes(hostel_id);

CREATE TRIGGER rooms_public_id
AFTER INSERT ON rooms
WHEN NEW.public_id = ''
BEGIN
    UPDATE rooms
    SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
    WHERE id = NEW.id AND public_id = '';
END;

CREATE TRIGGER room_classes_public_id
AFTER INSERT ON room_classes
WHEN NEW.public_id = ''
BEGIN
    UPDATE room_classes
    SET public_id = lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6)))
    WHERE id = NEW.id AND public_id = '';
END;

-- Abort (the CHECK fails) rather than commit a broken reference.
CREATE TEMP TABLE fk_violations (n INTEGER CHECK (n = 0));
INSERT INTO fk_violations SELECT COUNT(*) FROM pragma_foreign_key_check;
DROP TABLE fk_violations;

COMMIT;

PRAGMA foreign_keys = ON;

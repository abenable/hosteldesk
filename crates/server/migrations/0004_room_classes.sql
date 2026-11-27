-- Rooms used to carry their own type, floor, prices, and bed count. The type,
-- floor, and prices move to room classes and beds are dropped: a room now
-- names its class (`class_public_id`).
--
-- Each (floor, type, semester rate, recess rate) of a hostel's current rooms
-- becomes one class whose id is spelled from those values, so a desk upgrading
-- its own copy of the same rooms derives the same id
-- (crates/db/migrations/20261128000000_room_classes.sql) and re-sending it is
-- a no-op (protocol: re-creating an identical room class).
--
-- Every room change, old versions included, is rewritten to name the room's
-- current class. The classes enter the change log at negative cursors: a
-- fresh desk pulls from cursor 0, which the Worker serves from the very start,
-- so it sees each class before the first room naming it. A desk that already
-- pulled past 0 made the same classes itself when it upgraded.

CREATE TEMP TABLE legacy_rooms ON COMMIT DROP AS
SELECT
    hostel_public_id,
    public_id,
    'legacy:' || hostel_public_id || ':' || coalesce(body->>'floor', '') || ':'
        || (body->>'room_type') || ':' || (body->>'semester_rate') || ':'
        || (body->>'recess_rate') AS class_id,
    body
FROM entities
WHERE kind = 'room' AND NOT body ? 'class_public_id';

CREATE TEMP TABLE legacy_classes ON COMMIT DROP AS
SELECT DISTINCT ON (class_id)
    hostel_public_id,
    class_id,
    jsonb_build_object(
        'entity', 'room_class',
        'public_id', class_id,
        'hostel_public_id', hostel_public_id,
        'revision', 1,
        'floor', body->'floor',
        'room_type', body->'room_type',
        'semester_rate', body->'semester_rate',
        'recess_rate', body->'recess_rate'
    ) AS body
FROM legacy_rooms
ORDER BY class_id;

INSERT INTO entities (hostel_public_id, kind, public_id, lookup_key, active, body)
SELECT hostel_public_id, 'room_class', class_id, NULL, false, body
FROM legacy_classes;

INSERT INTO changes (hostel_public_id, cursor, body)
SELECT
    hostel_public_id,
    -row_number() OVER (PARTITION BY hostel_public_id ORDER BY class_id),
    body
FROM legacy_classes;

UPDATE entities e
SET body = (e.body - 'room_type' - 'capacity' - 'floor' - 'semester_rate' - 'recess_rate')
    || jsonb_build_object('class_public_id', l.class_id)
FROM legacy_rooms l
WHERE e.kind = 'room'
  AND e.hostel_public_id = l.hostel_public_id
  AND e.public_id = l.public_id;

UPDATE changes c
SET body = (c.body - 'room_type' - 'capacity' - 'floor' - 'semester_rate' - 'recess_rate')
    || jsonb_build_object('class_public_id', l.class_id)
FROM legacy_rooms l
WHERE c.body->>'entity' = 'room'
  AND c.hostel_public_id = l.hostel_public_id
  AND c.body->>'public_id' = l.public_id
  AND NOT c.body ? 'class_public_id';

-- A room change left without a class would fail every desk's pull: stop here.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM changes
        WHERE body->>'entity' = 'room' AND NOT body ? 'class_public_id'
    ) THEN
        RAISE EXCEPTION 'a room change has no class_public_id';
    END IF;
END $$;

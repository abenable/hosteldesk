-- Floors are gone: room numbers carry the prefix instead (A01, B12), and a
-- room class is its type and prices. Nothing indexes, checks, or triggers on
-- the column, so SQLite drops it in place. Class changes still waiting to
-- push may name a floor; the Worker ignores it.
ALTER TABLE room_classes DROP COLUMN floor;

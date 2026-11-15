-- 20261115000000 added residents.hostel_id as NOT NULL DEFAULT 0, then
-- backfilled only rows WHERE hostel_id IS NULL, which never matches. Every
-- resident from v1.x kept hostel_id = 0 and disappeared from the directory.
-- Give each one the hostel of its first stay, or the desk's first hostel.
UPDATE residents
SET hostel_id = COALESCE(
    (SELECT a.hostel_id FROM allocations a WHERE a.resident_id = residents.id ORDER BY a.id LIMIT 1),
    (SELECT id FROM hostels ORDER BY id LIMIT 1),
    0
)
WHERE hostel_id = 0;

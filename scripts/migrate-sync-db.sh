#!/usr/bin/env bash
# Apply pending crates/server/migrations/*.sql to the sync database, in name
# order, each in its own transaction together with its schema_migrations row.
#
#   scripts/migrate-sync-db.sh                          # production: URL from ~/.hosteldesk/neon_url
#   SYNC_DATABASE_URL=postgres://... scripts/migrate-sync-db.sh   # any other database
set -euo pipefail

url="${SYNC_DATABASE_URL:-$(cat "$HOME/.hosteldesk/neon_url")}"
dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/server/migrations" && pwd)"

psql "$url" -q -v ON_ERROR_STOP=1 -c \
  "CREATE TABLE IF NOT EXISTS schema_migrations (version TEXT PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT now())"

for file in "$dir"/*.sql; do
  version="$(basename "$file" .sql)"
  [[ "$version" =~ ^[0-9]{4}_[a-z0-9_]+$ ]] || { echo "bad migration name: $version" >&2; exit 1; }
  applied="$(psql "$url" -tAq -v ON_ERROR_STOP=1 -v version="$version" \
    <<<"SELECT 1 FROM schema_migrations WHERE version = :'version'")"
  if [[ "$applied" == "1" ]]; then
    continue
  fi
  echo "applying $version"
  psql "$url" -q -v ON_ERROR_STOP=1 -v version="$version" --single-transaction \
    -f "$file" -c "INSERT INTO schema_migrations (version) VALUES ('$version')"
done
echo "sync database is up to date"

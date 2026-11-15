#!/usr/bin/env bash
# End-to-end check of the sync Worker against a real Postgres.
#
# Locally (see docs/DEVELOPMENT.md for the Postgres container):
#   SYNC_DATABASE_URL=postgres://postgres:devpass@127.0.0.1:55432/hosteldesk scripts/migrate-sync-db.sh
#   cd crates/server && CLOUDFLARE_HYPERDRIVE_LOCAL_CONNECTION_STRING_HYPERDRIVE=\
#     "postgres://postgres:devpass@127.0.0.1:55432/hosteldesk?sslmode=disable" npx -y wrangler@4 dev --port 8787 &
#   scripts/smoke-worker.sh http://127.0.0.1:8787
#
# Point it at the deployed URL to smoke-test production (it creates a throwaway org).
set -euo pipefail

base="${1:-http://127.0.0.1:8787}"
run="$(date +%s)-$RANDOM"
email="smoke-${run}@example.com"
hostel="h-${run}"
fail() { echo "FAIL: $*" >&2; exit 1; }

# post PATH TOKEN BODY -> prints "STATUS BODY"
post() {
  local auth=()
  [[ -n "$2" ]] && auth=(-H "authorization: Bearer $2")
  curl -sS -o /tmp/smoke-body.$$ -w '%{http_code}' -X POST "$base$1" \
    -H 'content-type: application/json' "${auth[@]}" --data "$3"
  echo " $(cat /tmp/smoke-body.$$)"
}
trap 'rm -f /tmp/smoke-body.$$' EXIT
expect() { [[ "${2%% *}" == "$1" ]] || fail "$3: expected HTTP $1, got ${2}"; }

[[ "$(curl -sS "$base/health")" == "ok" ]] || fail "health"

out="$(post /v1/sign-up "" "{\"organization_name\":\"Smoke\",\"email\":\"$email\",\"password\":\"smoke-pass-1\"}")"
expect 200 "$out" "sign-up"
access="$(jq -r .access_token <<<"${out#* }")"

out="$(post /v1/devices "$access" '{"device_name":"smoke desk"}')"
expect 200 "$out" "enroll"
refresh="$(jq -r .refresh_token <<<"${out#* }")"
[[ "$(jq -r '.device_prefix | length' <<<"${out#* }")" == 6 ]] || fail "device prefix length"

room() { # id number
  echo "{\"idempotency_key\":\"room-$1\",\"command\":{\"type\":\"upsert_room\",\"public_id\":\"$1\",\"hostel_public_id\":\"$hostel\",\"base_revision\":0,\"room_number\":\"$2\",\"room_type\":\"single\",\"capacity\":1,\"floor\":null,\"semester_rate\":\"100\",\"recess_rate\":\"50\",\"status\":\"vacant\"}}"
}
check_in() { # allocation resident
  echo "{\"idempotency_key\":\"in-$1\",\"command\":{\"type\":\"check_in\",\"allocation_public_id\":\"$1\",\"hostel_public_id\":\"$hostel\",\"resident_public_id\":\"$2\",\"room_public_id\":\"r1\",\"full_name\":\"Ada\",\"phone\":null,\"email\":null,\"id_number\":null,\"next_of_kin\":null,\"next_of_kin_phone\":null,\"rate_agreed\":\"100\",\"term\":\"semester_1\",\"academic_year\":\"2026/2027\",\"checked_in_at\":\"2026-11-15 09:00:00\"}}"
}
pay() { # payment amount receipt
  echo "{\"idempotency_key\":\"pay-$1\",\"command\":{\"type\":\"record_payment\",\"payment_public_id\":\"$1\",\"receipt_public_id\":\"rc-$1\",\"hostel_public_id\":\"$hostel\",\"allocation_public_id\":\"a1\",\"resident_public_id\":\"p1\",\"amount\":\"$2\",\"payment_method\":\"cash\",\"receipt_number\":\"$3\",\"payment_date\":\"2026-11-15T09:30:00Z\",\"notes\":null}}"
}

batch="[$(room r1 101),$(check_in a1 p1),$(check_in a2 p2),$(pay pay1 40 SMK-0001),$(pay pay2 40 SMK-0001),$(pay pay3 -5 SMK-0002),$(room r2 101)]"
out="$(post /v1/push "$refresh" "{\"hostel_public_id\":\"$hostel\",\"commands\":$batch}")"
expect 200 "$out" "push"
statuses="$(jq -r '[.results[].status] | join(",")' <<<"${out#* }")"
[[ "$statuses" == "acked,acked,acked,acked,conflict,conflict,conflict" ]] || fail "push statuses: $statuses"
jq -e '.results[2].message | test("over capacity")' <<<"${out#* }" >/dev/null || fail "over-capacity warning"
jq -e '.results[5].message | test("amount")' <<<"${out#* }" >/dev/null || fail "bad amount message"

# Replaying the same batch changes nothing and acks the already-applied keys.
out="$(post /v1/push "$refresh" "{\"hostel_public_id\":\"$hostel\",\"commands\":[$(room r1 101)]}")"
expect 200 "$out" "replay"
[[ "$(jq -r '.results[0].status' <<<"${out#* }")" == acked ]] || fail "replay ack"

# A push in another organization's hostel is refused.
other="$(post /v1/sign-up "" "{\"organization_name\":\"Other\",\"email\":\"other-$email\",\"password\":\"smoke-pass-1\"}")"
other_access="$(jq -r .access_token <<<"${other#* }")"
other_refresh="$(jq -r .refresh_token <<<"$(post /v1/devices "$other_access" '{"device_name":"x"}' | cut -d' ' -f2-)")"
out="$(post /v1/pull "$other_refresh" "{\"hostel_public_id\":\"$hostel\",\"cursor\":0}")"
expect 401 "$out" "cross-org pull"

# Concurrent pushes from two desks of the same org both land (optimistic retry).
out="$(post /v1/devices "$access" '{"device_name":"second desk"}')"
refresh2="$(jq -r .refresh_token <<<"${out#* }")"
post /v1/push "$refresh" "{\"hostel_public_id\":\"$hostel\",\"commands\":[$(room r3 103)]}" >/tmp/smoke-a.$$ &
post /v1/push "$refresh2" "{\"hostel_public_id\":\"$hostel\",\"commands\":[$(room r4 104)]}" >/tmp/smoke-b.$$ &
wait
for f in /tmp/smoke-a.$$ /tmp/smoke-b.$$; do
  expect 200 "$(cat "$f")" "concurrent push"
  [[ "$(cut -d' ' -f2- "$f" | jq -r '.results[0].status')" == acked ]] || fail "concurrent ack"
  rm -f "$f"
done

out="$(post /v1/pull "$refresh2" "{\"hostel_public_id\":\"$hostel\",\"cursor\":0}")"
expect 200 "$out" "pull"
body="${out#* }"
jq -e '.has_more == false and .lease == null' <<<"$body" >/dev/null || fail "pull flags: $body"
rooms="$(jq -r '[.changes[] | select(.entity == "room") | .room_number] | unique | join(",")' <<<"$body")"
[[ "$rooms" == "101,103,104" ]] || fail "pulled rooms: $rooms"
jq -e '[.changes[] | select(.entity == "payment")] | length == 1 and .[0].payment_date == "2026-11-15T09:30:00Z"' <<<"$body" >/dev/null || fail "pulled payment"
cursor="$(jq -r .cursor <<<"$body")"
out="$(post /v1/pull "$refresh2" "{\"hostel_public_id\":\"$hostel\",\"cursor\":$cursor}")"
[[ "$(jq -r '.changes | length' <<<"${out#* }")" == 0 ]] || fail "pull after cursor not empty"

# Five wrong passwords lock the email; the sixth attempt is 429.
for _ in 1 2 3 4 5; do
  expect 401 "$(post /v1/sign-in "" "{\"email\":\"$email\",\"password\":\"wrong-pass\"}")" "wrong password"
done
expect 429 "$(post /v1/sign-in "" "{\"email\":\"$email\",\"password\":\"smoke-pass-1\"}")" "lockout"

echo "OK: worker smoke test passed against $base"

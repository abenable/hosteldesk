#!/usr/bin/env bash
# Approve and license HostelDesk accounts.
#
#   scripts/license-account.sh pending                     # accounts waiting for approval
#   scripts/license-account.sh owner@hostel.com approve    # let the account in
#   scripts/license-account.sh owner@hostel.com show       # approval, plan, end date
#   scripts/license-account.sh owner@hostel.com 1-year     # sell a plan
#   scripts/license-account.sh owner@hostel.com end        # end the license today
#
# New accounts start pending, on a 30-day trial that counts from sign-up.
# Plans: semester (6 months), 1-year, 3-year, 5-year. A sold plan starts from
# today, or from the current end date if that is still ahead (so renewing early
# never loses days). Desks pick changes up on their next sync (every 30
# seconds while online); an offline desk at most 14 days later.
#
# Uses ~/.hosteldesk/neon_url, or SYNC_DATABASE_URL when set.
set -euo pipefail

usage() { sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
url="${SYNC_DATABASE_URL:-$(cat "$HOME/.hosteldesk/neon_url")}"
columns="o.name, a.email, CASE WHEN o.approved_at IS NULL THEN 'pending' ELSE 'approved' END,
         o.license_plan, o.license_expires_on"

if [[ $# -eq 1 && "$1" == "pending" ]]; then
  psql "$url" -qtA -F ' | ' -v ON_ERROR_STOP=1 <<<"
    SELECT o.name, a.email, to_char(o.created_at, 'YYYY-MM-DD HH24:MI'), o.license_expires_on
    FROM organizations o JOIN accounts a ON a.org_id = o.id
    WHERE o.approved_at IS NULL ORDER BY o.created_at"
  exit 0
fi
[[ $# -eq 2 ]] || usage
email="$1"
action="$2"

case "$action" in
  show)
    sql="SELECT $columns FROM organizations o JOIN accounts a ON a.org_id = o.id
         WHERE a.email = lower(trim(:'email'))" ;;
  approve)
    sql="UPDATE organizations o SET approved_at = coalesce(o.approved_at, now())
         FROM accounts a WHERE a.org_id = o.id AND a.email = lower(trim(:'email'))
         RETURNING $columns" ;;
  end)
    sql="UPDATE organizations o SET license_expires_on = current_date - 1
         FROM accounts a WHERE a.org_id = o.id AND a.email = lower(trim(:'email'))
         RETURNING $columns" ;;
  semester|1-year|3-year|5-year)
    case "$action" in
      semester) span="6 months" ;;
      1-year) span="1 year" ;;
      3-year) span="3 years" ;;
      5-year) span="5 years" ;;
    esac
    sql="UPDATE organizations o SET license_plan = :'plan',
             license_expires_on = (greatest(o.license_expires_on, current_date) + :'span'::interval)::date
         FROM accounts a WHERE a.org_id = o.id AND a.email = lower(trim(:'email'))
         RETURNING $columns" ;;
  *)
    echo "unknown action: $action (use approve, show, end, semester, 1-year, 3-year, 5-year)" >&2
    exit 2 ;;
esac

# Variables are bound by psql (:'name' quoting), never pasted into the SQL.
out="$(psql "$url" -qtA -F ' | ' -v ON_ERROR_STOP=1 \
  -v email="$email" -v plan="$action" -v span="${span:-}" <<<"$sql")"
if [[ -z "$out" ]]; then
  echo "no account with email $email" >&2
  exit 1
fi
echo "$out"

#!/usr/bin/env bash
# Manage the license of the HostelDesk account that owns <email>.
#
#   scripts/license-account.sh owner@hostel.com show      # plan and end date
#   scripts/license-account.sh owner@hostel.com 1-year    # sell a plan
#   scripts/license-account.sh owner@hostel.com end       # end the license today
#
# Plans: semester (6 months), 1-year, 3-year, 5-year. A sold plan starts from
# today, or from the current end date if that is still ahead (so renewing early
# never loses days). Desks pick the change up on their next sync; an ended
# license locks them once their current lease runs out (at most 14 days).
#
# Uses ~/.hosteldesk/neon_url, or SYNC_DATABASE_URL when set.
set -euo pipefail

if [[ $# -ne 2 ]]; then
  sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
fi
email="$1"
action="$2"
url="${SYNC_DATABASE_URL:-$(cat "$HOME/.hosteldesk/neon_url")}"

case "$action" in
  show)
    sql="SELECT o.name, o.license_plan, o.license_expires_on
         FROM organizations o JOIN accounts a ON a.org_id = o.id
         WHERE a.email = lower(trim(:'email'))" ;;
  end)
    sql="UPDATE organizations o SET license_expires_on = current_date - 1
         FROM accounts a WHERE a.org_id = o.id AND a.email = lower(trim(:'email'))
         RETURNING o.name, o.license_plan, o.license_expires_on" ;;
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
         RETURNING o.name, o.license_plan, o.license_expires_on" ;;
  *)
    echo "unknown plan or action: $action (use show, end, semester, 1-year, 3-year, 5-year)" >&2
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

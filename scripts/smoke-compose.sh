#!/bin/sh
# Compose-integrated smoke: findings path (investigate→resolve, and accept).
set -eu
API="${API:-http://127.0.0.1:18080}"
DB="${DATABASE_URL:-postgres://postgres:nylonite@127.0.0.1:55432/nylonite}"
TENANT=11111111-1111-1111-1111-111111111111
PERSON=77770000-0000-0000-0000-000000000001
SITE=a5170000-0000-0000-0000-000000000001

echo "=== health ==="
curl -sf "$API/health" | grep -q '"status":"ok"'
echo ok

echo "=== stock ==="
curl -sf -H "x-tenant-id: $TENANT" "$API/stock" | grep -q GLOVE
echo ok

echo "=== open findings ==="
curl -sf -H "x-tenant-id: $TENANT" "$API/discrepancies" >/dev/null
curl -sf -H "x-tenant-id: $TENANT" \
  "$API/discrepancies?state=open,investigating&kind=count_variance" >/dev/null
# Filters: item + location codes; age bound (may be empty).
curl -sf -H "x-tenant-id: $TENANT" \
  "$API/discrepancies?item=GLOVE-M&location=A-01-1&state=open,investigating,resolved,accepted" \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert isinstance(d,list)"
curl -sf -H "x-tenant-id: $TENANT" \
  "$API/discrepancies?older_than_hours=0&state=open,investigating,resolved,accepted&limit=5" \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert isinstance(d,list)"
echo ok

ROW=$(psql "$DB" -tAc "
SELECT set_config('nylonite.tenant_id', '$TENANT', false);
SELECT id::text || '|' || quantity::text FROM stock
 WHERE quantity >= 10 AND holder_location_id IS NOT NULL
 ORDER BY quantity DESC LIMIT 1;
" | tail -1)
STOCK_ID=$(echo "$ROW" | cut -d'|' -f1)
SYSTEM=$(echo "$ROW" | cut -d'|' -f2)
COUNTED=$((SYSTEM - 3))
REASON=$(psql "$DB" -tAc "SELECT id FROM adjustment_reason WHERE code='damaged' AND tenant_id IS NULL")
NOW=$(python3 -c 'from datetime import datetime,timezone; print(datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))')
CE1=$(uuidgen | tr '[:upper:]' '[:lower:]')
CE2=$(uuidgen | tr '[:upper:]' '[:lower:]')
CE3=$(uuidgen | tr '[:upper:]' '[:lower:]')

echo "=== count (system=$SYSTEM counted=$COUNTED) ==="
COUNT=$(curl -sf -X POST "$API/counts" \
  -H "x-tenant-id: $TENANT" -H "content-type: application/json" \
  -d "{\"stock_id\":\"$STOCK_ID\",\"counted_quantity\":$COUNTED,\"client_event_id\":\"$CE1\",\"counted_at\":\"$NOW\",\"recorded_by_id\":\"$PERSON\",\"site_id\":\"$SITE\",\"challenged\":true,\"challenge_context\":\"smoke\",\"confirmed\":true}")
echo "$COUNT"
DISC=$(echo "$COUNT" | python3 -c "import sys,json; print(json.load(sys.stdin)['discrepancy_id'])")
SC1=$(echo "$COUNT" | python3 -c "import sys,json; print(json.load(sys.stdin)['stock_count_id'])")

echo "=== count replay same client_event (WP1) ==="
COUNT_REPLAY=$(curl -sf -X POST "$API/counts" \
  -H "x-tenant-id: $TENANT" -H "content-type: application/json" \
  -d "{\"stock_id\":\"$STOCK_ID\",\"counted_quantity\":$COUNTED,\"client_event_id\":\"$CE1\",\"counted_at\":\"$NOW\",\"recorded_by_id\":\"$PERSON\",\"site_id\":\"$SITE\",\"challenged\":true,\"challenge_context\":\"smoke\",\"confirmed\":true}")
echo "$COUNT_REPLAY"
echo "$COUNT_REPLAY" | SC1="$SC1" DISC="$DISC" python3 -c "
import sys, json, os
d = json.load(sys.stdin)
assert d['stock_count_id'] == os.environ['SC1']
assert str(d.get('discrepancy_id') or '') == str(os.environ.get('DISC') or '')
"
N_COUNTS=$(psql "$DB" -tAc "
SELECT set_config('nylonite.tenant_id', '$TENANT', false);
SELECT count(*) FROM stock_count WHERE client_event_id='$CE1';" | tail -1)
test "$N_COUNTS" = "1"
echo "stock_count rows for act=$N_COUNTS"

echo "=== investigate (open → investigating) ==="
INV=$(curl -sf -X POST "$API/discrepancies/$DISC/investigate" \
  -H "x-tenant-id: $TENANT" -H "content-type: application/json" \
  -d '{"note":"smoke checking bin"}')
echo "$INV"
echo "$INV" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['state']=='investigating'; assert d['id']=='$DISC'; assert 'item_code' in d; assert d.get('variance') is not None"
# Idempotent: second investigate is soft-ok with a warning; still full row.
INV2=$(curl -sf -X POST "$API/discrepancies/$DISC/investigate" \
  -H "x-tenant-id: $TENANT" -H "content-type: application/json" \
  -d '{}')
echo "$INV2" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['state']=='investigating'; assert d.get('warnings'); assert d['id']=='$DISC'"
STATE=$(psql "$DB" -tAc "
SELECT set_config('nylonite.tenant_id', '$TENANT', false);
SELECT state::text FROM discrepancy WHERE id='$DISC';" | tail -1)
test "$STATE" = "investigating"
echo "finding state=$STATE"

echo "=== adjust resolve ==="
ADJ=$(curl -sf -X POST "$API/adjustments" \
  -H "x-tenant-id: $TENANT" -H "content-type: application/json" \
  -d "{\"stock_id\":\"$STOCK_ID\",\"counted_quantity\":$COUNTED,\"adjustment_reason_id\":\"$REASON\",\"discrepancy_id\":\"$DISC\",\"client_event_id\":\"$CE2\",\"occurred_at\":\"$NOW\",\"recorded_by_id\":\"$PERSON\",\"site_id\":\"$SITE\"}")
echo "$ADJ"
echo "$ADJ" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d.get('resolved_discrepancy_id'); assert d.get('movement_id')"

STATE=$(psql "$DB" -tAc "
SELECT set_config('nylonite.tenant_id', '$TENANT', false);
SELECT state::text FROM discrepancy WHERE id='$DISC';" | tail -1)
test "$STATE" = "resolved"
echo "finding state=$STATE"

# Second cell: accept without ledger write (no adjust).
ROW2=$(psql "$DB" -tAc "
SELECT set_config('nylonite.tenant_id', '$TENANT', false);
SELECT id::text || '|' || quantity::text FROM stock
 WHERE quantity >= 5 AND holder_location_id IS NOT NULL AND id <> '$STOCK_ID'
 ORDER BY quantity DESC LIMIT 1;
" | tail -1)
STOCK2=$(echo "$ROW2" | cut -d'|' -f1)
SYS2=$(echo "$ROW2" | cut -d'|' -f2)
CNT2=$((SYS2 - 1))
NOW2=$(python3 -c 'from datetime import datetime,timezone; print(datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))')

echo "=== count for accept (system=$SYS2 counted=$CNT2) ==="
COUNT2=$(curl -sf -X POST "$API/counts" \
  -H "x-tenant-id: $TENANT" -H "content-type: application/json" \
  -d "{\"stock_id\":\"$STOCK2\",\"counted_quantity\":$CNT2,\"client_event_id\":\"$CE3\",\"counted_at\":\"$NOW2\",\"recorded_by_id\":\"$PERSON\",\"site_id\":\"$SITE\",\"challenged\":true,\"challenge_context\":\"smoke-accept\",\"confirmed\":true}")
echo "$COUNT2"
DISC2=$(echo "$COUNT2" | python3 -c "import sys,json; print(json.load(sys.stdin)['discrepancy_id'])")

echo "=== accept (open → accepted, no movement) ==="
ACC=$(curl -sf -X POST "$API/discrepancies/$DISC2/accept" \
  -H "x-tenant-id: $TENANT" -H "content-type: application/json" \
  -d "{\"recorded_by_id\":\"$PERSON\",\"reason\":\"within tolerance — smoke\",\"note\":\"no stock change\"}")
echo "$ACC"
echo "$ACC" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['state']=='accepted'; assert d.get('resolution_reason'); assert d['id']=='$DISC2'; assert 'item_code' in d; assert d.get('resolving_movement_id') is None"
# Soft re-accept still returns full row
ACC2=$(curl -sf -X POST "$API/discrepancies/$DISC2/accept" \
  -H "x-tenant-id: $TENANT" -H "content-type: application/json" \
  -d "{\"recorded_by_id\":\"$PERSON\",\"reason\":\"again\"}")
echo "$ACC2" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['state']=='accepted'; assert d.get('warnings'); assert d.get('resolution_reason')"
# Resolved cannot accept
code=$(curl -s -o /tmp/acc_rej.json -w "%{http_code}" -X POST "$API/discrepancies/$DISC/accept" \
  -H "x-tenant-id: $TENANT" -H "content-type: application/json" \
  -d "{\"recorded_by_id\":\"$PERSON\",\"reason\":\"nope\"}")
test "$code" = "400"
STATE2=$(psql "$DB" -tAc "
SELECT set_config('nylonite.tenant_id', '$TENANT', false);
SELECT state::text || '|' || COALESCE(resolving_movement_id::text,'') FROM discrepancy WHERE id='$DISC2';" | tail -1)
test "$(echo "$STATE2" | cut -d'|' -f1)" = "accepted"
test -z "$(echo "$STATE2" | cut -d'|' -f2)"
echo "accept finding state=$(echo "$STATE2" | cut -d'|' -f1) (no resolving movement)"
echo "SMOKE_OK"

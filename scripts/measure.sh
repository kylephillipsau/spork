#!/bin/sh
# Load a year of history and measure what question 122 asks about.
#
#   "The receiving queries are written and reasoned about, not measured."
#
# This is the measuring. It rebuilds from empty, loads both fixtures, checks that
# the generated year is reproducible, and runs the queries D24 names, printing
# what they actually cost rather than what they were expected to cost.
#
#   scripts/measure.sh
#   DATABASE_URL=... scripts/measure.sh
set -eu

DB="${DATABASE_URL:-postgres://postgres:spork@localhost:55432/spork}"
ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT"
GAMMA='33333333-3333-3333-3333-333333333333'

q()  { psql "$DB" -tAc "$1"; }
qq() { psql "$DB" -q -v ON_ERROR_STOP=1 "$@" >/dev/null; }

rebuild() {
    psql "$DB" -q -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;' >/dev/null 2>&1 || true
    psql "$DB" -q -c 'GRANT USAGE ON SCHEMA public TO spork_app, spork_platform,
                        spork_scheduler, spork_projection_owner;' >/dev/null 2>&1 || true
    for m in migrations/*/up.sql; do qq -f "$m"; done
    qq -f fixtures/seed.sql
    qq -f fixtures/history.sql
}

# The claim the fixture makes about itself, checked the only way that actually
# checks it: build it twice from nothing and compare.
#
# The cheap version -- delete the tenant and reload -- does not work and fails
# quietly if you let it. `person_tenant` references `tenant`, so the delete
# errors, the reload never happens, and the comparison is the data against
# itself. That is a check that passes by not running, which is the thing this
# repository keeps finding.
CK="SELECT md5(string_agg(x, '|' ORDER BY x)) FROM (
      SELECT id::text||quantity::text||occurred_at::text AS x
        FROM stock_movement WHERE tenant_id='$GAMMA') s"

echo "  building a year of history"
start=$(date +%s)
rebuild
echo "  built and folded in $(( $(date +%s) - start ))s"
a=$(q "$CK")

echo "  building it again, from nothing, to see whether it is the same year"
rebuild
b=$(q "$CK")
[ "$a" = "$b" ] && echo "  reproducible: two builds from empty agree" \
                || { echo "  FAIL not reproducible: $a vs $b"; exit 1; }

psql "$DB" -q -c "ANALYZE;" >/dev/null

echo
echo "  volume"
q "SELECT '    ' || rpad(t, 22) || to_char(n, 'FM999,999,999')
     FROM (VALUES
       ('stock_movement',      (SELECT count(*) FROM stock_movement)),
       ('expected_supply',     (SELECT count(*) FROM expected_supply)),
       ('goods_receipt_line',  (SELECT count(*) FROM goods_receipt_line)),
       ('purchase_order_line', (SELECT count(*) FROM purchase_order_line)),
       ('stock cells',         (SELECT count(*) FROM stock)),
       ('client_event',        (SELECT count(*) FROM client_event))
     ) v(t, n)"

echo
echo "  the live set, which is what D24's partial index rests on"
q "SELECT '    ' || count(*) FILTER (WHERE closed_at IS NULL) || ' open of ' || count(*)
       || ' (' || round(100.0 * count(*) FILTER (WHERE closed_at IS NULL) / count(*), 1) || '%)'
     FROM expected_supply WHERE tenant_id = '$GAMMA'"
q "SELECT '    ' || rpad(coalesce(closed_reason::text, 'still open'), 20) || c
     FROM (SELECT closed_reason, count(*) AS c FROM expected_supply
            WHERE tenant_id = '$GAMMA' GROUP BY closed_reason) s ORDER BY 1"

echo
echo "  D24 query B: the line list for one delivery"
psql "$DB" -tAc "SET spork.tenant_id = '$GAMMA';
EXPLAIN (ANALYZE, COSTS OFF, TIMING OFF, SUMMARY ON)
SELECT es.id, es.quantity_outstanding, i.code
  FROM expected_supply es
  JOIN purchase_order_line pol ON pol.id = es.purchase_order_line_id
  JOIN item i ON i.id = es.item_id
 WHERE es.tenant_id = '$GAMMA'
   AND pol.purchase_order_id = (SELECT id FROM purchase_order
                                 WHERE tenant_id = '$GAMMA'
                                 ORDER BY created_at DESC LIMIT 1)
   AND es.closed_at IS NULL;" | sed 's/^/    /' | grep -E "Execution|Planning|Scan|Loop|Index"

echo
echo "  what is still outstanding, by site: the open-work read"
psql "$DB" -tAc "SET spork.tenant_id = '$GAMMA';
EXPLAIN (ANALYZE, COSTS OFF, TIMING OFF, SUMMARY ON)
SELECT site_id, count(*), sum(quantity_outstanding)
  FROM expected_supply
 WHERE tenant_id = '$GAMMA' AND closed_at IS NULL
 GROUP BY site_id;" | sed 's/^/    /' | grep -E "Execution|Planning|Scan|Index"

echo
echo "  the fold itself, over a year (D95, remeasured under D106)"
# Three runs: the first may pay for buffer cache; the median is what cadence math uses.
fold_ms=""
i=1
while [ "$i" -le 3 ]; do
    f0=$(date +%s%N)
    q "SELECT projection_run_all('$GAMMA')" >/dev/null
    f1=$(date +%s%N)
    ms=$(( (f1 - f0) / 1000000 ))
    fold_ms="$fold_ms $ms"
    echo "    run $i: ${ms} ms"
    i=$((i + 1))
done
# shell median of three
set -- $fold_ms
# sort numerically
med=$(printf '%s\n' "$@" | sort -n | sed -n '2p')
echo "    projection_run_all median: ${med} ms over $(q "SELECT count(*) FROM projection_step") steps"

echo
echo "  per step (last run's last_run_ms against freshness_bound)"
q "SELECT '    ' || lpad(s.ordinal::text, 3) || '  '
         || rpad(s.function_name, 42)
         || lpad(coalesce(f.last_run_ms::text, '-'), 6) || ' ms'
         || '  bound ' || s.freshness_bound
         || '  touched ' || coalesce(f.rows_touched::text, '0')
     FROM projection_step s
     LEFT JOIN projection_freshness f
       ON f.function_name = s.function_name AND f.tenant_id = '$GAMMA'
    ORDER BY s.ordinal"

echo
echo "  cadence ceiling (sequential tenants inside the 5-minute stock bound)"
if [ -n "$med" ] && [ "$med" -gt 0 ] 2>/dev/null; then
    echo "    $(( 300000 / med )) tenants per 5-minute cycle at this median fold time"
else
    echo "    (median fold time unavailable)"
fi

echo
echo "  the write budget: what one promise row costs under contention"
( cd "$ROOT" && DATABASE_URL="$DB" cargo test -q --test contention -- --nocapture --test-threads=1 2>&1 ) \
  | grep -E "update per|gate held|^ +[0-9]+ +[0-9.]+ ms|allocators contending" | sed 's/^/  /'

echo
echo "  and the suite, against a year rather than against two movements"
s0=$(date +%s%N)
( cd "$ROOT" && DATABASE_URL="$DB" cargo test -q 2>&1 | grep -cE "test result: ok" ) >/dev/null
s1=$(date +%s%N)
echo "    $(( (s1 - s0) / 1000000 )) ms"

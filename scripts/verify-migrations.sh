#!/bin/sh
# Verify the migration set five ways, and print the line a commit message quotes.
#
# Every commit here has claimed "up and down from empty leaving no tables". That
# was true and it is the weaker of the two properties. A down migration is only
# ever run against a database with rows in it, and the seeded reverse had never
# been tested once — which is how migration 14 shipped a down that could not run,
# and stayed that way for three commits.
#
#   1. up from empty          the schema builds
#   2. down from empty        it reverses, leaving no tables and no types
#   3. up, seed               the small fixture applies
#   4. the generated year     the large fixture applies too, one day of it
#   5. down with data         it reverses with both fixtures' rows in it
#
# Phase 4 caught nothing on the day it was added and that is the point of it: the
# large fixture had already been unloadable for three migrations, because the only
# script that touches it is measure.sh and nothing runs measure.sh. Phase 5 was the
# new one before it, and the only one that caught the down migration that could not
# run at all.
#
#   scripts/verify-migrations.sh
#   DATABASE_URL=... scripts/verify-migrations.sh
set -eu

DB="${DATABASE_URL:-postgres://postgres:spork@localhost:55432/spork}"
ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT"

psql_q() { psql "$DB" -q -v ON_ERROR_STOP=1 "$@"; }

reset() {
    psql_q -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;' >/dev/null 2>&1 || true
}

# The reset used to GRANT USAGE ON SCHEMA public to the four spork roles
# here, and that grant is why this script passed for months against a schema
# in which every projection maintainer was broken. Roles are cluster-wide, so
# one run left the repair behind for every database on the machine, forever.
# The migrations grant what they need (1, 48 and now 77). A harness that
# repairs the thing it is checking is worse than no harness.

ups() {
    for m in migrations/*/up.sql; do
        psql_q -f "$m" >/dev/null || { echo "FAIL  up $m"; exit 1; }
    done
}

# Reverse order. `ls -r` over the date-ordered directory names is the reverse
# sequence, which is what a down chain has to be.
downs() {
    for d in $(ls -rd migrations/*/); do
        psql_q -f "$d/down.sql" >/dev/null || { echo "FAIL  down $d"; exit 1; }
    done
}

# Nothing left behind. Types are checked as well as tables because a dropped
# table takes its columns and not the enum they were declared over.
assert_empty() {
    t=$(psql "$DB" -tAc "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'")
    y=$(psql "$DB" -tAc "SELECT count(*) FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace
                          WHERE n.nspname='public' AND t.typtype IN ('e','c','d')")
    p=$(psql "$DB" -tAc "SELECT count(*) FROM pg_policy")
    f=$(psql "$DB" -tAc "SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
                          WHERE n.nspname='public' AND p.proname LIKE 'projection%'")
    [ "$t" = 0 ] && [ "$y" = 0 ] && [ "$p" = 0 ] && [ "$f" = 0 ] && return 0
    echo "FAIL  $1 left $t table(s), $y type(s), $p policy(ies), $f projection function(s)"
    exit 1
}

n=$(ls -d migrations/*/ | wc -l | tr -d ' ')

reset
ups
echo "ok    1. $n migrations up from empty"

downs
assert_empty "down from empty"
echo "ok    2. down from empty, nothing left behind"

ups
psql_q -f fixtures/seed.sql >/dev/null || { echo "FAIL  fixture"; exit 1; }
echo "ok    3. fixture applies"

# Phase 5 is the newest and it exists because this file could not load for three
# migrations without anything noticing. D92 added a CHECK the generated year did
# not satisfy, and the only script that loads it is measure.sh, which no test
# runs. One day rather than the year: the statements are what is being checked,
# not the volume, and a day costs 0.15s against two minutes.
psql_q -v days=0 -f fixtures/history.sql >/dev/null || {
    echo "FAIL  the generated year no longer loads against this schema"; exit 1; }
echo "ok    4. the generated year applies, one day of it"

downs
assert_empty "down with data"
echo "ok    5. down again with both fixtures' rows in place, nothing left behind"

# Leave the database in the state the suite expects to run against.
ups
psql_q -f fixtures/seed.sql >/dev/null

echo
echo "$n migrations up and down from empty, and again with both fixtures' rows, leaving no tables, types or policies."

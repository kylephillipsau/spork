#!/usr/bin/env sh
#
# A verified dump of the deployed database.
#
#   scripts/backup.sh                    # dump to ./backups, keep 14
#   scripts/backup.sh --out /srv/backups --keep 30
#
# Run it on the host the stack is running on — it reaches Postgres through
# `docker exec`, so it needs no published port and no password on the command
# line.
#
# # A dump nobody checked is not a backup
#
# The way this fails is not a crash. `pg_dump` writes to a pipe; if the
# container dies, the disk fills, or the pipe breaks halfway, the file is a
# perfectly readable prefix of a database and `$?` can still be 0. It restores,
# it reports no errors, and it is missing the second half of the ledger.
#
# So every dump is checked before it is kept: the archive must decompress, and
# it must end with the line `pg_dump` writes only after it has written
# everything. A dump without that line is deleted rather than filed, because a
# bad backup in the folder is worse than no backup — it is the one nobody
# doubts at four in the morning.
#
# # The flags are the ones deployment.md arrived at by being wrong first
#
# No `--no-owner`, no `--no-acl`. The GRANTs and the `SECURITY DEFINER`
# ownership *are* the privilege model here: strip them and the restore succeeds
# silently, then the first request fails with "permission denied for table
# client_event". Restoring is in `docs/deployment.md`, and it wants
# `ON_ERROR_STOP=1` for the same class of reason this script wants that marker.

set -eu

OUT="./backups"
KEEP=14

while [ $# -gt 0 ]; do
  case "$1" in
    --out)  OUT="$2"; shift 2 ;;
    --keep) KEEP="$2"; shift 2 ;;
    -h|--help) sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

CONTAINER="${POSTGRES_CONTAINER:-$(docker ps --filter "ancestor=postgres:18" --format '{{.Names}}' | head -1)}"
if [ -z "$CONTAINER" ]; then
  echo "no running postgres:18 container found." >&2
  echo "Set POSTGRES_CONTAINER to name one explicitly." >&2
  exit 1
fi

mkdir -p "$OUT"
STAMP=$(date -u +%Y%m%d-%H%M%SZ)
FILE="$OUT/nylonite-$STAMP.sql.gz"

echo "  dumping from $CONTAINER"
# No `-t`: a cron job has no terminal, and asking for one makes docker refuse.
docker exec "$CONTAINER" pg_dump -U postgres -d nylonite \
  | gzip -9 > "$FILE"

# ── the checks, before this counts as a backup ──────────────────────────────

if ! gzip -t "$FILE" 2>/dev/null; then
  echo "  ✗ the archive does not decompress — deleting it" >&2
  rm -f "$FILE"
  exit 1
fi

# `pg_dump` writes this line last and only on success. Its absence is a
# truncated dump wearing a successful exit code.
if ! gzip -dc "$FILE" | tail -5 | grep -q "PostgreSQL database dump complete"; then
  echo "  ✗ the dump is truncated — deleting it" >&2
  rm -f "$FILE"
  exit 1
fi

TABLES=$(gzip -dc "$FILE" | grep -c "^CREATE TABLE" || true)
MIGRATIONS=$(docker exec "$CONTAINER" psql -U postgres -d nylonite -tAc \
  "SELECT count(*) FROM schema_migration" 2>/dev/null || echo "?")
SIZE=$(du -h "$FILE" | cut -f1)

echo "  ✓ $FILE"
echo "    $SIZE · $TABLES tables · schema at $MIGRATIONS migrations"

# ── retention ──────────────────────────────────────────────────────────────
#
# Counted rather than aged: "keep the last fourteen" survives a fortnight of
# the job not running, where "delete older than fourteen days" quietly empties
# the folder on the day somebody notices it stopped.

KEPT=$(ls -1t "$OUT"/nylonite-*.sql.gz 2>/dev/null | wc -l | tr -d ' ')
if [ "$KEPT" -gt "$KEEP" ]; then
  ls -1t "$OUT"/nylonite-*.sql.gz | tail -n "+$((KEEP + 1))" | while read -r old; do
    echo "    pruning $(basename "$old")"
    rm -f "$old"
  done
fi

echo "    $(ls -1 "$OUT"/nylonite-*.sql.gz 2>/dev/null | wc -l | tr -d ' ') kept"

#!/bin/sh
# Apply the migrations this database does not have, and nothing else.
#
# This is the deploy's first step and the server's precondition. It runs to
# completion before the server starts, so **a binary ahead of its schema is not
# a state the system can be in** — which is the failure that shipped twice: a
# deploy against a database missing migration 78, a clean start in the log, and
# `/resolve` answering 500 behind a green health check.
#
#   scripts/migrate.sh              apply what is pending
#   scripts/migrate.sh --baseline   record everything as applied, apply nothing
#   scripts/migrate.sh --status     say what is pending and exit
#
# `verify-migrations.sh` is the other way to build a database and stays exactly
# as it is: it applies every file to a throwaway and proves the set reverses.
# This one is for a database somebody is keeping.
set -eu

DB="${DATABASE_URL:-postgres://postgres:spork@localhost:55432/spork}"
ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT"

MODE=apply
case "${1:-}" in
    --baseline) MODE=baseline ;;
    --status)   MODE=status ;;
    "")         ;;
    *)          echo "usage: $0 [--baseline|--status]" >&2; exit 2 ;;
esac

psql_q() { psql "$DB" -q -v ON_ERROR_STOP=1 "$@"; }

# The ledger arrives in migration 80, so a database older than that has nowhere
# to be asked. Treat "no table" as "nothing recorded" and let the run put it
# there — 80 is applied like any other migration and its own row goes in with it.
have_ledger() {
    psql "$DB" -tAc "SELECT to_regclass('public.schema_migration') IS NOT NULL" 2>/dev/null \
        | grep -q '^t$'
}

applied() {
    if have_ledger; then
        psql "$DB" -tAc 'SELECT name FROM schema_migration'
    fi
}

# Every migration directory, in the order the filesystem gives them — which is
# the order the numbered prefixes define and the order every other script here
# already relies on.
all() { ls -1 migrations | sort; }

pending() {
    have=$(applied | sort)
    for m in $(all); do
        printf '%s\n' "$have" | grep -qxF "$m" || printf '%s\n' "$m"
    done
}

# ---------------------------------------------------------------------------
# Two of these at once
# ---------------------------------------------------------------------------
#
# **There is no lock, and that is a correction rather than an omission.**
#
# The first version took a `pg_advisory_lock` in a backgrounded psql and carried
# a comment saying the second deploy would wait and then find nothing pending.
# Both halves were false. Nothing waited for the lock to be *acquired* — the
# background session blocked while the foreground loop applied migrations
# anyway — and `pending` was computed before the lock was even attempted, so a
# second run would have applied a stale list regardless. It was a lock that did
# not lock, described by a comment that said it did, in the one script somebody
# reads during a bad deploy.
#
# What actually protects the database is below it: each migration is applied
# atomically where Postgres allows, and its ledger row goes in immediately
# after. Two concurrent runs therefore race on the same migration, one wins, and
# **the loser fails on something that already exists and exits 1** — which is a
# red deploy rather than a corrupted schema. That is the honest behaviour, and
# it is good enough because the thing it guards against is rare and its symptom
# is loud.
#
# If concurrent deploys ever stop being rare, the fix is a lock acquired
# synchronously — psql holding it and signalling that it has, before `pending`
# is computed — not the version that was here.

# ---------------------------------------------------------------------------
# Modes
# ---------------------------------------------------------------------------

if [ "$MODE" = status ]; then
    left=$(pending)
    if [ -z "$left" ]; then
        echo "up to date — $(all | wc -l | tr -d ' ') migrations applied"
    else
        echo "pending:"
        printf '  %s\n' $left
    fi
    exit 0
fi

if [ "$MODE" = baseline ]; then
    # **Explicit, never inferred.** The alternative is a heuristic — if some
    # table exists, assume everything before it ran — and a heuristic that is
    # wrong marks a pending migration as applied, which is the one lie this
    # whole mechanism exists to prevent. So an operator says it, once, about a
    # database they know is current.
    #
    # It needs the ledger to exist, so 80 itself is applied first if it is not
    # there yet.
    if ! have_ledger; then
        echo "baseline: applying migration 80 first, because the ledger is what records a baseline"
        for m in $(all); do
            case "$m" in *_a_database_says_which_migrations_it_has)
                psql_q -1 -f "migrations/$m/up.sql" >/dev/null \
                    || { echo "FAIL  $m" >&2; exit 1; } ;;
            esac
        done
    fi
    n=0
    for m in $(all); do
        psql_q -c "INSERT INTO schema_migration (name) VALUES ('$m')
                   ON CONFLICT (name) DO NOTHING" >/dev/null
        n=$((n + 1))
    done
    echo "baseline — $n migration(s) recorded as applied, none run"
    exit 0
fi

# ---------------------------------------------------------------------------
# Apply
# ---------------------------------------------------------------------------

left=$(pending)
if [ -z "$left" ]; then
    echo "up to date — nothing to apply"
    exit 0
fi

count=$(printf '%s\n' $left | wc -l | tr -d ' ')
echo "applying $count migration(s)"

# **The ledger cannot record the migrations that precede it**, and that is not a
# flaw to design around — it is what "the schema is the migrations" costs. The
# table arrives in migration 80 like every other table, so while 1 to 79 are
# being applied there is nowhere to write a row. The first draft tried to write
# one anyway and failed on migration 1, which is the whole bootstrap paradox
# arriving on the first run.
#
# So: apply, and record when there is somewhere to record. The moment the ledger
# appears, everything applied so far in this run — and everything that was
# already there — is by definition applied, because migrations run in order.
# That backfill is one line here rather than a list of eighty names inside 80's
# own up.sql, which would be a second copy of the directory listing.
for m in $left; do
    printf '  %s ... ' "$m"

    # **`--single-transaction` where it can be, and named where it cannot.**
    #
    # The obvious design wraps the migration and its ledger row in one
    # transaction, so a crash cannot leave a database that has the change and
    # does not know it. It works for most of these and not for all, and the
    # reason is not one anybody checks for.
    #
    # Six migrations run `ALTER TYPE … ADD VALUE`, and some of them *use* the
    # value they just added. Postgres refuses that inside a transaction —
    # *"New enum values must be committed before they can be used"* — so
    # migration 30 fails under `-1` and always would have.
    # `verify-migrations.sh` has applied every file without `-1` since it was
    # written, which is why nothing ever noticed.
    #
    # I grepped for `CONCURRENTLY` before writing the transactional version,
    # found none, and concluded the set was transaction-safe. It was the wrong
    # check. Only running it found the right one.
    #
    # So the choice is per file. A migration that adds an enum value runs
    # without a transaction and can leave part of itself behind if it fails;
    # every other one is atomic, and its failure leaves nothing.
    if grep -qiE 'ALTER +TYPE.*ADD +VALUE' "migrations/$m/up.sql"; then
        atomic=no
    else
        atomic=yes
    fi

    ok=0
    if [ "$atomic" = yes ]; then
        why=$(psql "$DB" -q -v ON_ERROR_STOP=1 -1 \
                  -f "migrations/$m/up.sql" 2>&1 >/dev/null) || ok=1
    else
        why=$(psql "$DB" -q -v ON_ERROR_STOP=1 \
                  -f "migrations/$m/up.sql" 2>&1 >/dev/null) || ok=1
    fi

    if [ "$ok" = 0 ]; then
        # Record it. Before migration 80 there is nowhere to write, and the
        # moment the ledger appears everything applied so far is by definition
        # applied — migrations run in order — so the backfill happens here
        # rather than as eighty hardcoded names inside 80's own up.sql.
        if have_ledger; then
            for done_m in $(all); do
                psql_q -c "INSERT INTO schema_migration (name) VALUES ('$done_m')
                           ON CONFLICT (name) DO NOTHING" >/dev/null
                [ "$done_m" = "$m" ] && break
            done
        fi
        echo "ok"
    else
        echo "FAILED"
        # **The error from the attempt that failed**, not from a second run.
        # The first draft re-ran the file to show what went wrong, and the
        # re-run hit "relation already exists" left behind by the first
        # attempt — so it reported a different, misleading error than the one
        # that actually stopped the deploy.
        echo "" >&2
        printf '%s\n' "$why" >&2
        echo "" >&2
        if [ "$atomic" = yes ]; then
            echo "migration $m failed inside a transaction: nothing it did survives, and nothing was recorded." >&2
        else
            # Said plainly, because it is the one case where a re-run needs a
            # person. This migration adds an enum value and therefore could not
            # be wrapped.
            echo "migration $m failed and could not run in a transaction, so part of it may have been applied." >&2
            echo "nothing was recorded. Inspect the database before running this again." >&2
        fi
        exit 1
    fi
done

echo "schema is at $(all | tail -1)"

import { useMemo, useState } from "react";
import { CircleAlert, Package } from "lucide-react";

import { Button, Card, DataTable, EmptyState, Link, PageHeader, SearchField, Tabs, type Column } from "@ui/index";
import { href } from "@app/routing/location";
import { useNavigate } from "@app/routing/Router";
import { DueBadge, Faint, Progress, StageBadge } from "@app/common/cells";
import type { PackJob, Stage } from "@domain/types";

import { STAGES, STAGE_LABELS, type QueueBench } from "./useQueue";
import s from "./pack-queue.module.css";

type View = "all" | Stage;

/**
 * The packing queue (D151, D171): every fulfilment at this site with work
 * for the bench, filtered by stage and searched by reference, order or
 * customer. A row opens the pack bench.
 */
export function PackQueuePage({ bench, onOpen }: { bench: QueueBench; onOpen?: ((job: PackJob) => void) | undefined }) {
  const [view, setView] = useState<View>("all");
  const jobs = bench.state.kind === "ready" ? bench.state.jobs : [];

  const counts = useMemo(() => {
    const c: Record<View, number> = { all: jobs.length, ready: 0, on_the_bench: 0, packed: 0, nothing_committed: 0 };
    for (const j of jobs) c[j.stage] += 1;
    return c;
  }, [jobs]);

  const rows = view === "all" ? jobs : jobs.filter((j) => j.stage === view);

  return (
    <div className={s.page}>
      <PageHeader title="Packing" description="Fulfilments at this site with work for the pack bench." />

      <Card padded={false}>
        <div className={s.toolbar}>
          <Tabs
            aria-label="Stage"
            value={view}
            onValueChange={(v) => setView(v as View)}
            items={[
              { value: "all", label: "All", count: counts.all },
              ...STAGES.map((st) => ({ value: st, label: STAGE_LABELS[st], count: counts[st] })),
            ]}
          />
          <form
            className={s.search}
            onSubmit={(e) => {
              e.preventDefault();
              void bench.search();
            }}
          >
            <SearchField
              aria-label="Search the queue"
              placeholder="Reference, order or customer"
              value={bench.term}
              onChange={(e) => bench.type(e.target.value)}
            />
            <Button type="submit" size="md">
              Search
            </Button>
          </form>
        </div>

        {bench.state.kind === "failed" ? (
          <p className={s.failed} role="alert">
            <CircleAlert aria-hidden /> {bench.state.message}
          </p>
        ) : (
          <DataTable
            aria-label="Packing queue"
            columns={COLUMNS}
            rows={rows}
            rowKey={(j) => j.fulfilment_id}
            loading={bench.state.kind === "loading"}
            onRowClick={onOpen}
            initialSort={{ key: "due", direction: "asc" }}
            empty={
              <EmptyState
                icon={<Package />}
                title={bench.term.trim() ? "Nothing matches that search" : "Nothing to pack"}
                description={bench.term.trim() ? "Try a different reference, order or customer." : undefined}
              />
            }
          />
        )}
      </Card>
    </div>
  );
}

/** The live screen: a row opens the pack bench. */
export function LivePackQueue({ bench }: { bench: QueueBench }) {
  const navigate = useNavigate();
  return <PackQueuePage bench={bench} onOpen={(j) => navigate(`/pack/${j.fulfilment_id}`)} />;
}

const COLUMNS: Column<PackJob>[] = [
  {
    key: "ref",
    header: "Fulfilment",
    cell: (j) => (
      <Link href={href(`/pack/${j.fulfilment_id}`)} onClick={(e) => e.stopPropagation()}>
        {j.reference ?? "—"}
      </Link>
    ),
    sort: (j) => j.reference,
    mono: true,
    width: "120px",
  },
  {
    key: "order",
    header: "Order",
    cell: (j) => j.order_reference ?? <Faint>—</Faint>,
    sort: (j) => j.order_reference,
    mono: true,
    width: "110px",
  },
  { key: "customer", header: "Customer", cell: (j) => j.customer, sort: (j) => j.customer, grow: true },
  {
    key: "picked",
    header: "Picked",
    cell: (j) => <Progress done={j.picked} of={j.committed} />,
    sort: (j) => (j.committed ? j.picked / j.committed : 0),
    width: "150px",
  },
  { key: "lines", header: "Lines", cell: (j) => j.lines, sort: (j) => j.lines, align: "right", width: "70px" },
  {
    key: "cartons",
    header: "Cartons",
    cell: (j) => j.cartons || <Faint>—</Faint>,
    sort: (j) => j.cartons,
    align: "right",
    width: "80px",
  },
  {
    key: "due",
    header: "Due",
    cell: (j) => <DueBadge due={j.due} />,
    sort: (j) => (j.due === "overdue" ? 0 : j.due ? 1 : 2),
    width: "130px",
  },
  {
    key: "stage",
    header: "Stage",
    cell: (j) => <StageBadge stage={j.stage} />,
    sort: (j) => STAGES.indexOf(j.stage),
    width: "150px",
  },
];

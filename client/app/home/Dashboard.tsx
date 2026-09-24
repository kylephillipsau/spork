import type { ReactNode } from "react";
import {
  ArrowRight,
  CircleAlert,
  ClipboardList,
  Inbox,
  Package,
  RefreshCw,
  ScanLine,
  TriangleAlert,
  Truck,
  type LucideIcon,
} from "lucide-react";

import { Badge, Button, DataTable, EmptyState, Link, PageHeader, Skeleton, cx, type Column, type Tone } from "@ui/index";
import { href } from "@app/routing/location";
import type { DiscrepancyRow, OrderMatch, PackJob } from "@domain/types";

import type { DashboardBench, Read } from "./useDashboard";
import s from "./dashboard.module.css";

/** How many rows each panel shows before "View all". */
const TOP = 8;

export function Dashboard({ dash, site }: { dash: DashboardBench; site: string | null }) {
  const w = dash.work.kind === "ready" ? dash.work.data : null;

  return (
    <div className={s.page}>
      <PageHeader
        title="Dashboard"
        description={site ? `Work waiting at ${site}` : "Work waiting"}
        actions={
          <Button size="sm" icon={<RefreshCw />} loading={dash.refreshing} onClick={() => void dash.refresh()}>
            Refresh
          </Button>
        }
      />

      <div className={s.tiles}>
        <Tile icon={ScanLine} label="Lines to pick" value={w?.pick} path="/picking" read={dash.work} />
        <Tile icon={Package} label="Orders to pick and pack" value={w?.pack} path="/pack" read={dash.work} />
        <Tile icon={Truck} label="Cartons to despatch" value={w?.despatch} path="/despatch" read={dash.work} />
        <Tile
          icon={TriangleAlert}
          label="Open findings"
          value={w?.findings}
          path="/findings"
          read={dash.work}
          tone={w?.findings ? "warning" : undefined}
        />
      </div>

      <div className={s.split}>
        <Panel
          title="Packing queue"
          count={dash.queue.kind === "ready" ? dash.queue.data.length : undefined}
          viewAll="/pack"
        >
          <ReadBody read={dash.queue} rows={5}>
            {(jobs) => (
              <DataTable
                aria-label="Packing queue"
                columns={QUEUE_COLUMNS}
                rows={jobs.filter((j) => j.stage !== "packed").slice(0, TOP)}
                rowKey={(j) => j.fulfilment_id}
                empty={<EmptyState icon={<Inbox />} title="Nothing waiting to pack" />}
              />
            )}
          </ReadBody>
        </Panel>

        <Panel
          title="Open findings"
          count={dash.findings.kind === "ready" ? dash.findings.data.length : undefined}
          viewAll="/findings"
        >
          <ReadBody read={dash.findings} rows={4}>
            {(rows) =>
              rows.length === 0 ? (
                <EmptyState icon={<TriangleAlert />} title="No open findings" />
              ) : (
                <ul className={s.findings}>
                  {rows.slice(0, 6).map((f) => (
                    <li key={f.id}>
                      <FindingRow f={f} />
                    </li>
                  ))}
                </ul>
              )
            }
          </ReadBody>
        </Panel>
      </div>

      <Panel
        title="Latest orders"
        count={dash.orders.kind === "ready" ? dash.orders.data.length : undefined}
        viewAll="/orders"
      >
        <ReadBody read={dash.orders} rows={4}>
          {(orders) => (
            <DataTable
              aria-label="Latest orders"
              columns={ORDER_COLUMNS}
              rows={orders.slice(0, TOP)}
              rowKey={(o) => o.order_id}
              empty={<EmptyState icon={<ClipboardList />} title="No orders yet" description="Orders sent from NetSuite appear here." />}
            />
          )}
        </ReadBody>
      </Panel>
    </div>
  );
}

/* ---- tiles ---- */

function Tile({
  icon: Icon,
  label,
  value,
  path,
  read,
  tone,
}: {
  icon: LucideIcon;
  label: string;
  value: number | undefined;
  path: string;
  read: Read<unknown>;
  tone?: "warning" | undefined;
}) {
  return (
    <Link variant="plain" href={href(path)} className={cx(s.tile, tone === "warning" && s.tileWarning)}>
      <span className={s.tileTop}>
        <span className={s.tileLabel}>{label}</span>
        <span className={s.tileIcon} aria-hidden>
          <Icon />
        </span>
      </span>
      <span className={s.tileValue}>
        {read.kind === "loading" ? <Skeleton width={40} height={26} /> : read.kind === "failed" ? "–" : value ?? 0}
      </span>
    </Link>
  );
}

/* ---- panels ---- */

function Panel({
  title,
  count,
  viewAll,
  className,
  children,
}: {
  title: string;
  count?: number | undefined;
  viewAll: string;
  className?: string | undefined;
  children: ReactNode;
}) {
  return (
    <section className={cx(s.panel, className)}>
      <header className={s.panelHead}>
        <h2 className={s.panelTitle}>
          {title}
          {count !== undefined && <span className={s.panelCount}>{count}</span>}
        </h2>
        <Link href={href(viewAll)} className={s.viewAll}>
          View all <ArrowRight aria-hidden />
        </Link>
      </header>
      {children}
    </section>
  );
}

function ReadBody<T>({ read, rows, children }: { read: Read<T>; rows: number; children: (data: T) => ReactNode }) {
  if (read.kind === "loading") {
    return (
      <div className={s.loading} aria-busy>
        {Array.from({ length: rows }, (_, i) => (
          <Skeleton key={i} width={`${90 - i * 9}%`} />
        ))}
      </div>
    );
  }
  if (read.kind === "failed") {
    return (
      <p className={s.failed} role="alert">
        <CircleAlert aria-hidden /> {read.message}
      </p>
    );
  }
  return <>{children(read.data)}</>;
}

/* ---- packing queue ---- */

const STAGE: Record<PackJob["stage"], { label: string; tone: Tone }> = {
  ready: { label: "Ready to pack", tone: "accent" },
  on_the_bench: { label: "On the bench", tone: "info" },
  packed: { label: "Packed", tone: "success" },
  nothing_committed: { label: "Nothing committed", tone: "neutral" },
};

const QUEUE_COLUMNS: Column<PackJob>[] = [
  {
    key: "ref",
    header: "Fulfilment",
    cell: (j) => (
      <Link href={href(`/pack/${j.fulfilment_id}`)}>{j.reference ?? j.order_reference ?? "—"}</Link>
    ),
    sort: (j) => j.reference ?? j.order_reference,
    mono: true,
    width: "110px",
  },
  { key: "customer", header: "Customer", cell: (j) => j.customer, sort: (j) => j.customer, grow: true },
  {
    key: "picked",
    header: "Picked",
    cell: (j) => <Progress done={j.picked} of={j.committed} />,
    sort: (j) => (j.committed ? j.picked / j.committed : 0),
    width: "130px",
  },
  {
    key: "due",
    header: "Due",
    cell: (j) =>
      j.due ? <Badge tone={j.due === "overdue" ? "danger" : "neutral"}>{sentence(j.due)}</Badge> : <span className={s.faint}>—</span>,
    sort: (j) => (j.due === "overdue" ? 0 : j.due ? 1 : 2),
    width: "130px",
  },
  {
    key: "stage",
    header: "Stage",
    cell: (j) => (
      <Badge tone={STAGE[j.stage].tone} dot>
        {STAGE[j.stage].label}
      </Badge>
    ),
    sort: (j) => ["ready", "on_the_bench", "packed", "nothing_committed"].indexOf(j.stage),
    width: "140px",
  },
];

function Progress({ done, of }: { done: number; of: number }) {
  const pct = of > 0 ? Math.min(100, Math.round((done / of) * 100)) : 0;
  return (
    <span className={s.progress} title={`${done} of ${of} picked`}>
      <span className={s.bar} aria-hidden>
        <span className={cx(s.fill, pct === 100 && s.full)} style={{ width: `${pct}%` }} />
      </span>
      <span className={s.progressText}>
        {done}/{of}
      </span>
    </span>
  );
}

/* ---- findings ---- */

function FindingRow({ f }: { f: DiscrepancyRow }) {
  const subject = [f.item_code, f.location_code ?? f.package_barcode].filter(Boolean).join(" · ");
  return (
    <Link variant="plain" href={href(`/findings/${f.id}`)} className={s.finding}>
      <span className={cx(s.findingDot, f.state === "investigating" && s.findingDotInfo)} aria-hidden />
      <span className={s.findingMain}>
        <span className={s.findingKind}>{sentence(f.kind)}</span>
        <span className={s.findingSubject}>{subject || "—"}</span>
      </span>
      <span className={s.findingMeta}>
        {f.variance && <span className={s.variance}>{signed(f.variance)}</span>}
        <span className={s.faint}>{ago(f.detected_at)}</span>
      </span>
    </Link>
  );
}

/* ---- orders ---- */

const ORDER_STATE_TONE: Record<string, Tone> = {
  placed: "info",
  open: "info",
  fulfilled: "success",
  despatched: "success",
  cancelled: "neutral",
  superseded: "neutral",
};

function orderTotals(o: OrderMatch) {
  return o.fulfilments.reduce(
    (t, f) => ({
      lines: t.lines + f.line_count,
      committed: t.committed + f.committed_quantity,
      picked: t.picked + f.picked_quantity,
      packed: t.packed + f.packed_quantity,
    }),
    { lines: 0, committed: 0, picked: 0, packed: 0 },
  );
}

const ORDER_COLUMNS: Column<OrderMatch>[] = [
  {
    key: "order",
    header: "Order",
    cell: (o) => {
      const ref = o.confirmation_number ?? o.external_ref;
      return ref ? <Link href={href(`/orders?reference=${encodeURIComponent(ref)}`)}>{ref}</Link> : "—";
    },
    sort: (o) => o.confirmation_number,
    mono: true,
    width: "130px",
  },
  { key: "customer", header: "Customer", cell: (o) => o.customer_name ?? "—", sort: (o) => o.customer_name, grow: true },
  {
    key: "sites",
    header: "Site",
    cell: (o) => [...new Set(o.fulfilments.map((f) => f.site_code).filter(Boolean))].join(", ") || "—",
    width: "90px",
  },
  { key: "lines", header: "Lines", cell: (o) => orderTotals(o).lines, sort: (o) => orderTotals(o).lines, align: "right", width: "70px" },
  {
    key: "picked",
    header: "Picked",
    cell: (o) => {
      const t = orderTotals(o);
      return <Progress done={t.picked} of={t.committed} />;
    },
    width: "150px",
  },
  {
    key: "state",
    header: "Status",
    cell: (o) => (
      <Badge tone={ORDER_STATE_TONE[o.state] ?? "neutral"} dot>
        {sentence(o.state)}
      </Badge>
    ),
    sort: (o) => o.state,
    width: "130px",
  },
  {
    key: "placed",
    header: "Placed",
    cell: (o) => (o.placed_at ? shortDate(o.placed_at) : "—"),
    sort: (o) => o.placed_at,
    align: "right",
    width: "100px",
  },
];

/* ---- formatting (D114: the client formats) ---- */

function sentence(s: string): string {
  const t = s.replace(/_/g, " ").trim();
  return t.charAt(0).toUpperCase() + t.slice(1);
}

function signed(v: string): string {
  const n = Number(v);
  return Number.isFinite(n) && n > 0 ? `+${v}` : v;
}

function shortDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

function ago(iso: string): string {
  const mins = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / 60000));
  if (mins < 60) return `${mins} min`;
  const h = Math.round(mins / 60);
  if (h < 48) return `${h} h`;
  return `${Math.round(h / 24)} d`;
}

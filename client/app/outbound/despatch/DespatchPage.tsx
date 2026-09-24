import { useState } from "react";
import { PackageCheck, Send, Truck } from "lucide-react";

import { Badge, Button, Card, DataTable, EmptyState, PageHeader, Select, Skeleton, type Column } from "@ui/index";
import { grams, millimetres } from "@design/format";
import type { BookedCarton, BookedConsignment, CarrierLine, DespatchScreen, GoneConsignment, WaitingJob } from "@domain/types";
import { Faint, dateTime, sentence } from "@app/common/cells";
import { Alert } from "@app/admin/Alert";

import type { DespatchBench } from "./useDespatch";
import s from "./despatch.module.css";

const kg = (g: number | null) => (g === null ? "—" : `${grams(g)} kg`);

/**
 * Despatch (D171): consign sealed cartons to a carrier, then record them
 * leaving. Consigning is preparation and reversible until the goods move;
 * Despatch is the act that says they have gone, so it is the primary button.
 */
export function DespatchPage({ bench }: { bench: DespatchBench }) {
  const st = bench.status;

  return (
    <div className={s.page}>
      <PageHeader
        title="Despatch"
        description="Consign sealed cartons to a carrier, then record them leaving."
      />

      {(bench.problem || bench.notice) && (
        <Alert tone={bench.problem ? "danger" : "success"} onDismiss={bench.dismiss}>
          {bench.problem ?? `Booked ${bench.booked?.packages ?? 0} carton(s) with ${bench.booked?.carrier ?? "the carrier"}.`}
          {bench.booked?.warnings.map((w) => (
            <span key={w} className={s.warning}>
              {w}
            </span>
          ))}
        </Alert>
      )}

      {st.kind === "failed" && <Alert tone="danger">{st.message}</Alert>}
      {st.kind === "loading" && (
        <Card>
          <Skeleton width="60%" />
        </Card>
      )}
      {st.kind === "ready" && (
        <>
          <Waiting screen={st.screen} bench={bench} />
          {bench.booked && bench.booked.lines.length > 0 && <Manifest booked={bench.booked} />}
          {st.screen.booked.map((c) => (
            <Booked key={c.consignment_id} consignment={c} bench={bench} />
          ))}
          <GoneToday gone={st.screen.gone_today} />
        </>
      )}
    </div>
  );
}

/* ---- waiting to consign ---- */

function Waiting({ screen, bench }: { screen: DespatchScreen; bench: DespatchBench }) {
  const [carrier, setCarrier] = useState(screen.carriers[0]?.id ?? "");
  const [service, setService] = useState(screen.carriers[0]?.services[0]?.id ?? "");
  const services = screen.carriers.find((c) => c.id === carrier)?.services ?? [];

  const columns: Column<WaitingJob>[] = [
    { key: "ref", header: "Fulfilment", cell: (j) => <strong>{j.reference}</strong>, sort: (j) => j.reference, mono: true, width: "120px" },
    { key: "order", header: "Order", cell: (j) => j.order_reference, sort: (j) => j.order_reference, mono: true, width: "110px" },
    { key: "customer", header: "Customer", cell: (j) => j.customer, sort: (j) => j.customer, grow: true },
    { key: "cartons", header: "Cartons", cell: (j) => j.cartons.length, sort: (j) => j.cartons.length, align: "right", width: "80px" },
    { key: "gross", header: "Gross", cell: (j) => kg(j.gross_weight_g), sort: (j) => j.gross_weight_g, align: "right", width: "110px" },
    {
      key: "weighed",
      header: "Weighed",
      cell: (j) => {
        // Said before it leaves: an unweighed carton is weighed by the
        // carrier, and their figure arrives on their invoice.
        const missing = j.cartons.filter((c) => !c.weighed).length;
        return missing ? <Badge tone="warning">{missing} not weighed</Badge> : <Badge tone="success">All</Badge>;
      },
      width: "130px",
    },
    {
      key: "action",
      header: "",
      cell: (j) => (
        <Button
          size="sm"
          icon={<Send />}
          disabled={bench.busy || !carrier}
          onClick={() =>
            void bench.consign({
              packages: j.cartons.map((c) => c.id),
              ...(carrier ? { carrier } : {}),
              ...(service ? { service } : {}),
            })
          }
        >
          Consign {j.cartons.length}
        </Button>
      ),
      align: "right",
      width: "130px",
    },
  ];

  return (
    <Card title="Waiting to consign" description={screen.providers.length ? `Booked through ${screen.providers.map((p) => p.name).join(", ")}` : "No freight provider configured"} padded={false}>
      <div className={s.toolbar}>
        <div className={s.select}>
          <Select
            label="Carrier"
            value={carrier || undefined}
            onValueChange={(next) => {
              setCarrier(next);
              setService(screen.carriers.find((c) => c.id === next)?.services[0]?.id ?? "");
            }}
            options={screen.carriers.map((c) => ({ value: c.id, label: c.name }))}
            placeholder="No carriers"
            size="sm"
          />
        </div>
        <div className={s.select}>
          <Select
            label="Service"
            value={service || undefined}
            onValueChange={setService}
            options={services.map((x) => ({ value: x.id, label: x.name }))}
            placeholder="No services"
            size="sm"
          />
        </div>
      </div>
      <DataTable
        aria-label="Waiting to consign"
        columns={columns}
        rows={screen.waiting}
        rowKey={(j) => j.fulfilment_id}
        empty={<EmptyState icon={<PackageCheck />} title="Dock clear" description="Every sealed carton is booked." />}
      />
    </Card>
  );
}

/* ---- booked ---- */

function Booked({ consignment: c, bench }: { consignment: BookedConsignment; bench: DespatchBench }) {
  const gone = c.package_count - c.awaiting_despatch;
  const columns: Column<BookedCarton>[] = [
    { key: "ref", header: "Carton", cell: (p) => p.reference, mono: true, grow: true },
    { key: "seq", header: "Sequence", cell: (p) => p.sequence, align: "right", width: "100px" },
    {
      key: "state",
      header: "",
      cell: (p) =>
        p.despatched ? (
          <Badge tone="success" dot>
            Gone
          </Badge>
        ) : (
          <Button size="sm" disabled={bench.busy} onClick={() => void bench.despatchCarton(p)}>
            Despatch
          </Button>
        ),
      align: "right",
      width: "120px",
    },
  ];

  return (
    <Card
      title={
        <span className={s.consignmentTitle}>
          <Truck aria-hidden />
          {c.carrier_name ?? "No carrier named"}
          {c.carrier_service_name && <Badge>{c.carrier_service_name}</Badge>}
          {c.awaiting_despatch === 0 && (
            <Badge tone="success" dot>
              Gone
            </Badge>
          )}
        </span>
      }
      actions={
        c.awaiting_despatch > 0 ? (
          <Button variant="primary" loading={bench.busy} onClick={() => void bench.despatchAll(c.packages)}>
            Despatch {c.awaiting_despatch}
          </Button>
        ) : undefined
      }
      padded={false}
    >
      <div className={s.stats}>
        <Stat label="Cartons" value={String(c.package_count)} />
        <Stat label="Gone" value={`${gone} / ${c.package_count}`} />
        <Stat label="Gross" value={kg(c.gross_weight_g)} />
        <Stat label="Despatch" value={c.despatch_at ? dateTime(c.despatch_at) : "—"} />
        <Stat label="Carrier status" value={c.status ? sentence(c.status) : "—"} />
      </div>
      <DataTable aria-label="Cartons" columns={columns} rows={c.packages} rowKey={(p) => p.id} />
    </Card>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className={s.stat}>
      <span className={s.statLabel}>{label}</span>
      <span className={s.statValue}>{value}</span>
    </div>
  );
}

/* ---- the manifest just booked ---- */

const MANIFEST_COLUMNS: Column<CarrierLine>[] = [
  { key: "code", header: "Code", cell: (l) => l.carrier_package_code ?? <Faint>—</Faint>, mono: true, width: "90px" },
  { key: "type", header: "Package", cell: (l) => l.package_type ?? <Faint>Unstated</Faint>, grow: true },
  { key: "count", header: "Cartons", cell: (l) => l.package_count, align: "right", width: "80px" },
  { key: "gross", header: "Gross", cell: (l) => kg(l.gross_weight_g), align: "right", width: "110px" },
  {
    key: "dims",
    header: "Dimensions (mm)",
    cell: (l) =>
      l.length_mm === null ? (
        <Faint>—</Faint>
      ) : (
        <>
          {millimetres(l.length_mm)} × {l.width_mm === null ? "—" : millimetres(l.width_mm)} ×{" "}
          {l.height_mm === null ? "—" : millimetres(l.height_mm)} {l.uniform ? "" : <Badge tone="warning">Sizes differ</Badge>}
        </>
      ),
    width: "240px",
  },
];

function Manifest({ booked }: { booked: NonNullable<DespatchBench["booked"]> }) {
  return (
    <Card
      title="Consignment manifest"
      description="Recorded here only. No carrier has been contacted yet."
      actions={booked.total_gross_weight_g !== null ? <Badge tone="accent">Total {kg(booked.total_gross_weight_g)}</Badge> : undefined}
      padded={false}
    >
      <DataTable aria-label="Manifest" columns={MANIFEST_COLUMNS} rows={booked.lines} rowKey={(l) => `${l.carrier_package_code}-${l.package_type}-${l.package_count}`} />
    </Card>
  );
}

/* ---- gone today ---- */

const GONE_COLUMNS: Column<GoneConsignment>[] = [
  { key: "carrier", header: "Carrier", cell: (g) => g.carrier_name ?? <Faint>No carrier named</Faint>, grow: true },
  { key: "cartons", header: "Cartons", cell: (g) => g.package_count, align: "right", width: "90px" },
  {
    key: "at",
    header: "Last despatched",
    cell: (g) => (g.last_despatched_at ? dateTime(g.last_despatched_at) : <Faint>—</Faint>),
    align: "right",
    width: "170px",
  },
];

function GoneToday({ gone }: { gone: GoneConsignment[] }) {
  return (
    <Card title="Despatched today" padded={false}>
      <DataTable
        aria-label="Despatched today"
        columns={GONE_COLUMNS}
        rows={gone}
        rowKey={(g) => g.consignment_id}
        empty={<EmptyState icon={<Truck />} title="Nothing has left today" />}
      />
    </Card>
  );
}

import { Scale } from "lucide-react";

import { Alert, Badge, Button, Card, DataTable, EmptyState, Fact, Facts, Page, PageHeader, Select, Skeleton, TextField, type Column } from "@ui/index";
import { kg } from "@app/common/format";
import type { ToWeigh } from "@domain/types";
import { Faint, sentence } from "@app/common/cells";

import { current, type WeighBench } from "./useWeigh";
import s from "./weigh.module.css";


/**
 * Weigh (D171): items whose held weight is missing or out of date, one at a
 * time on the scale. A reading that disagrees with what was held keeps both
 * and records a finding.
 */
export function WeighPage({ bench }: { bench: WeighBench }) {
  const st = bench.status;
  const queue = st.kind === "ready" ? st.queue : [];
  const subject = current(st, bench.at);
  const left = Math.max(0, queue.length - bench.at);
  const next = queue.slice(bench.at + 1);

  return (
    <Page>
      <PageHeader
        title="Weigh"
        description="Reweigh items whose recorded weight is missing or out of date."
        actions={st.kind === "ready" ? <Badge tone={left ? "accent" : "success"}>{left ? `${left} to weigh` : "All in date"}</Badge> : undefined}
      />

      {bench.problem && (
        <Alert tone="danger" onDismiss={bench.dismiss}>
          {bench.problem}
        </Alert>
      )}
      {bench.recorded && !bench.problem && (
        <Alert tone={bench.recorded.disagreed ? "warning" : "success"} onDismiss={bench.dismiss}>
          {bench.recorded.code} weighed {kg(bench.recorded.recorded_g)}.
          {bench.recorded.disagreed &&
            ` That disagrees with the ${kg(bench.recorded.previous_g)} held${
              bench.recorded.previous_method ? ` (${bench.recorded.previous_method})` : ""
            }. Both weights are kept and a finding records the difference.`}
        </Alert>
      )}
      {st.kind === "failed" && <Alert tone="danger">{st.message}</Alert>}

      <div className={s.split}>
        <Card title="On the scale">
          {st.kind === "loading" ? (
            <Skeleton width="50%" />
          ) : subject ? (
            <OnTheScale bench={bench} subject={subject} />
          ) : (
            <EmptyState icon={<Scale />} title="Nothing to weigh" description="All weights are up to date." />
          )}
        </Card>

        <Card title="Up next" description={next.length ? `${next.length} after this one` : undefined} padded={false}>
          <DataTable
            aria-label="Up next"
            columns={NEXT_COLUMNS}
            rows={next.slice(0, 12)}
            rowKey={(x) => `${x.item_id ?? x.item_style_id}/${x.packaging_level}`}
            loading={st.kind === "loading"}
            empty={<EmptyState title="No more items" />}
          />
        </Card>
      </div>
    </Page>
  );
}

function OnTheScale({ bench, subject }: { bench: WeighBench; subject: ToWeigh }) {
  return (
    <form
      className={s.scale}
      onSubmit={(e) => {
        e.preventDefault();
        if (bench.reading.trim()) void bench.record();
      }}
    >
      <div>
        <div className={s.code}>{subject.code}</div>
        {subject.description && <div className={s.description}>{subject.description}</div>}
        <div className={s.badges}>
          <Badge>{sentence(subject.packaging_level)}</Badge>
          <Badge tone="warning">{subject.because === "never" ? "Never weighed" : "Overdue"}</Badge>
          {subject.demand > 0 && <Badge tone="info">{subject.demand} on order</Badge>}
        </div>
      </div>

      <div className={s.held}>
        <Facts>
          <Fact label="Held weight">
            <span className={s.heldValue}>{kg(subject.held_g)}</span>
          </Fact>
          <Fact label="Measured by" always>
            {subject.held_method ? sentence(subject.held_method) : <Faint>—</Faint>}
          </Fact>
        </Facts>
      </div>

      <div className={s.reading}>
        <div className={s.readingField}>
          <TextField
            label="Scale reading"
            inputMode="decimal"
            autoComplete="off"
            value={bench.reading}
            onChange={(e) => bench.type(e.target.value)}
            disabled={bench.busy}
            autoFocus
          />
        </div>
        <div className={s.unit}>
          <Select
            label="Unit"
            value={bench.unit}
            onValueChange={bench.setUnit}
            options={[
              { value: "kg", label: "kg" },
              { value: "g", label: "g" },
            ]}
            disabled={bench.busy}
          />
        </div>
      </div>

      <div className={s.actions}>
        <Button disabled={bench.busy} onClick={bench.skip}>
          Skip
        </Button>
        <Button type="submit" variant="primary" loading={bench.busy} disabled={bench.reading.trim() === ""}>
          Record weight
        </Button>
      </div>
    </form>
  );
}

const NEXT_COLUMNS: Column<ToWeigh>[] = [
  { key: "code", header: "Item", cell: (x) => x.code, mono: true, grow: true },
  { key: "level", header: "Level", cell: (x) => sentence(x.packaging_level), width: "90px" },
  {
    key: "why",
    header: "Why",
    cell: (x) => <Badge tone={x.because === "never" ? "warning" : "neutral"}>{x.because === "never" ? "Never" : "Overdue"}</Badge>,
    width: "100px",
  },
  { key: "demand", header: "On order", cell: (x) => x.demand || <Faint>—</Faint>, align: "right", width: "90px" },
];

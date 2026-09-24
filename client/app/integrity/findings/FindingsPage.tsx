import { Camera, CircleCheck, Search } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  DataTable,
  Drawer,
  EmptyState,
  Fact,
  Facts,
  Link,
  Page,
  PageHeader,
  Section,
  Stack,
  Stat,
  StatGrid,
  Tabs,
  TextField,
  Toolbar,
  cx,
  type Column,
  type Tone,
} from "@ui/index";
import { imageUrl } from "@domain/api";
import type { DiscrepancyRow } from "@domain/types";
import { Faint, ago, dateTime, sentence, signed } from "@app/common/cells";

import { evidenceSubjectName, type FindingsDesk } from "./useFindings";
import { VIEWS, type ViewKey } from "./views";
import s from "./findings-page.module.css";

const STATE_TONE: Record<string, Tone> = {
  open: "warning",
  investigating: "info",
  accepted: "neutral",
  resolved: "success",
};

/**
 * Findings (D8, D171): where a scan disagreed with the record. The table is
 * the queue; a row opens the evidence and the acts in a drawer. The finding
 * is in the path (D135), so a link to one opens it here.
 */
export function FindingsPage({ desk }: { desk: FindingsDesk }) {
  const st = desk.status;
  const rows = st.kind === "ready" ? st.findings : [];

  return (
    <Page>
      <PageHeader title="Findings" description="Where a scan disagreed with the record, with the evidence." />

      {desk.problem && !desk.selected && (
        <Alert tone="danger" onDismiss={desk.dismiss}>
          {desk.problem}
        </Alert>
      )}

      <Card padded={false}>
        <Toolbar>
          <Tabs
            aria-label="Which findings"
            value={desk.view}
            onValueChange={(v) => desk.look(v as ViewKey)}
            items={VIEWS.map((v) => ({
              value: v.key,
              label: v.label,
              count: v.key === desk.view && st.kind === "ready" ? rows.length : undefined,
            }))}
          />
        </Toolbar>
        {st.kind === "failed" ? (
          <div className={s.inset}>
            <Alert tone="danger">{st.message}</Alert>
          </div>
        ) : (
          <DataTable
            aria-label="Findings"
            columns={COLUMNS}
            rows={rows}
            rowKey={(f) => f.id}
            loading={st.kind === "loading"}
            onRowClick={(f) => desk.select(f)}
            selectedKey={desk.selected?.id}
            empty={<EmptyState icon={<CircleCheck />} title="No findings here" description="Everything in this view agrees with the ledger." />}
          />
        )}
      </Card>

      <Drawer
        open={desk.selected !== null}
        onOpenChange={(o) => {
          if (!o) desk.select(null);
        }}
        title={desk.selected ? sentence(desk.selected.kind) : ""}
        description={desk.selected ? subjectLine(desk.selected) : undefined}
        width={520}
        footer={desk.selected ? <Footer desk={desk} f={desk.selected} /> : undefined}
      >
        {desk.selected && <Detail desk={desk} f={desk.selected} />}
      </Drawer>
    </Page>
  );
}

function isOpen(f: DiscrepancyRow) {
  return f.state === "open" || f.state === "investigating";
}

function subjectLine(f: DiscrepancyRow): string {
  return [f.item_code, f.location_code, f.package_barcode].filter(Boolean).join(" · ") || "—";
}

/* ---- the drawer ---- */

function Detail({ desk, f }: { desk: FindingsDesk; f: DiscrepancyRow }) {
  const subject = evidenceSubjectName(f);
  const pair = f.expected_quantity !== null || f.observed_quantity !== null;

  return (
    <Stack gap={5}>
      {(desk.problem || desk.said) && (
        <Alert tone={desk.problem ? "danger" : "success"} onDismiss={desk.dismiss}>
          {desk.problem ?? desk.said?.join(" ")}
        </Alert>
      )}

      <div className={s.status}>
        <Badge tone={STATE_TONE[f.state] ?? "neutral"} dot>
          {sentence(f.state)}
        </Badge>
        <span className={s.muted}>
          Found {ago(f.detected_at)} ago by {f.detected_by_name ?? "a scheduled check"}
        </span>
      </div>

      {pair && (
        <Card padded={false}>
          <StatGrid>
            <Stat label="Expected" value={f.expected_quantity ?? "—"} />
            <Stat label="Counted" value={f.observed_quantity ?? "—"} />
            <Stat label="Difference" value={f.variance ? signed(f.variance) : "—"} tone={f.variance ? "warning" : undefined} />
          </StatGrid>
        </Card>
      )}

      <Facts>
        <Fact label="Item" mono>{f.item_code}</Fact>
        <Fact label="Bin" mono>{f.location_code}</Fact>
        <Fact label="Carton" mono>{f.package_barcode}</Fact>
        <Fact label="Found">{dateTime(f.detected_at)}</Fact>
        <Fact label="Closed">{f.resolved_at ? dateTime(f.resolved_at) : null}</Fact>
        <Fact label="Closed by">{f.resolved_by_name}</Fact>
        <Fact label="Reason" wide>{f.resolution_reason}</Fact>
        <Fact label="Detail" wide>{f.detail}</Fact>
      </Facts>

      <Section
        title="Photographs"
        count={f.evidence.length}
        actions={
          subject && (
            <label className={cx(s.upload, desk.busy && s.uploadBusy)}>
              <Camera aria-hidden />
              <span>Add photo of {subject}</span>
              <input
                type="file"
                accept="image/*"
                capture="environment"
                disabled={desk.busy}
                onChange={(e) => {
                  const file = e.currentTarget.files?.[0];
                  // Cleared so the same file twice still fires: a second
                  // photograph of the same thing is a second row.
                  e.currentTarget.value = "";
                  if (file) void desk.attach(file);
                }}
              />
            </label>
          )
        }
      >
        {f.evidence.length > 0 ? (
          <div className={s.photos}>
            {f.evidence.map((digest) => (
              <Link key={digest} variant="plain" className={s.photo} href={imageUrl(digest)} target="_blank" rel="noreferrer">
                <img src={imageUrl(digest)} alt="Evidence" loading="lazy" />
              </Link>
            ))}
          </div>
        ) : (
          <p className={s.muted}>{subject ? "No photographs yet." : "Nothing to photograph: this finding names no item, bin or carton."}</p>
        )}
      </Section>

      {isOpen(f) ? (
        <Section title="Accept">
          <TextField
            label="Reason"
            hint="Accepting closes the finding without moving stock."
            value={desk.reason}
            onChange={(e) => desk.typeReason(e.target.value)}
            disabled={desk.busy}
            onKeyDown={(e) => {
              if (e.key === "Enter" && desk.reason.trim()) void desk.accept(f.id);
            }}
          />
        </Section>
      ) : (
        <p className={s.muted}>Closed findings are kept for the record and cannot be reopened here.</p>
      )}
    </Stack>
  );
}

function Footer({ desk, f }: { desk: FindingsDesk; f: DiscrepancyRow }) {
  if (!isOpen(f)) return <Button onClick={() => desk.select(null)}>Close</Button>;
  return (
    <>
      {f.state === "open" && (
        <Button icon={<Search />} disabled={desk.busy} onClick={() => void desk.investigate(f.id)}>
          Investigate
        </Button>
      )}
      <Button
        variant="primary"
        loading={desk.busy}
        disabled={desk.reason.trim() === ""}
        onClick={() => void desk.accept(f.id)}
      >
        Accept
      </Button>
    </>
  );
}


/* ---- the table ---- */

const COLUMNS: Column<DiscrepancyRow>[] = [
  {
    key: "kind",
    header: "Finding",
    cell: (f) => (
      <span className={s.kind}>
        <span className={cx(s.dot, s[`dot_${f.state}`])} aria-hidden />
        {sentence(f.kind)}
      </span>
    ),
    sort: (f) => f.kind,
    width: "190px",
  },
  {
    key: "subject",
    header: "Concerns",
    cell: (f) => <span className={s.mono}>{subjectLine(f)}</span>,
    sort: (f) => f.item_code ?? f.location_code ?? f.package_barcode,
    grow: true,
  },
  { key: "expected", header: "Expected", cell: (f) => f.expected_quantity ?? <Faint>—</Faint>, align: "right", width: "90px" },
  { key: "counted", header: "Counted", cell: (f) => f.observed_quantity ?? <Faint>—</Faint>, align: "right", width: "90px" },
  {
    key: "variance",
    header: "Difference",
    cell: (f) => (f.variance ? <strong className={s.variance}>{signed(f.variance)}</strong> : <Faint>—</Faint>),
    sort: (f) => (f.variance ? Math.abs(Number(f.variance)) : null),
    align: "right",
    width: "100px",
  },
  {
    key: "state",
    header: "Status",
    cell: (f) => (
      <Badge tone={STATE_TONE[f.state] ?? "neutral"} dot>
        {sentence(f.state)}
      </Badge>
    ),
    sort: (f) => f.state,
    width: "130px",
  },
  { key: "by", header: "Found by", cell: (f) => f.detected_by_name ?? <Faint>Scheduled check</Faint>, width: "140px" },
  {
    key: "age",
    header: "Age",
    cell: (f) => ago(f.detected_at),
    sort: (f) => -new Date(f.detected_at).getTime(),
    align: "right",
    width: "80px",
  },
];

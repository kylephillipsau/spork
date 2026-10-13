import { PackageCheck, RefreshCw } from "lucide-react";

import { Alert, Button, Card, EmptyState, Page, PageHeader, Skeleton, Stack, Tabs, Tooltip, Toolbar } from "@ui/index";
import { ago } from "@app/common/cells";
import type { PackerCounts } from "@domain/types";

import type { AdministrationBench, Span } from "./useAdministration";
import s from "./administration.module.css";

const SPANS: readonly { value: Span; label: string; none: string }[] = [
  { value: "today", label: "Today", none: "Nobody has packed an order today." },
  { value: "this_week", label: "This week", none: "Nobody has packed an order this week." },
  { value: "last_30_days", label: "Last 30 days", none: "Nobody has packed an order in the last 30 days." },
];

/** What NetSuite's Packed By names nobody is called here. */
const NOBODY = "Not recorded";

/**
 * Administration (D254): for a workspace's administrators, how its people's
 * work is going. Orders packed per person, by the site's own day, week and
 * month, as NetSuite's Packed By says.
 */
export function AdministrationPage({ bench }: { bench: AdministrationBench }) {
  const st = bench.state;
  return (
    <Page>
      <PageHeader
        title="Administration"
        description="How the work is going, person by person."
        actions={
          <Button size="sm" icon={<RefreshCw />} loading={bench.refreshing} onClick={() => void bench.refresh()}>
            Refresh
          </Button>
        }
      />
      {st.kind === "failed" ? (
        <Alert tone="danger">{st.message}</Alert>
      ) : (
        <Card
          title="Orders packed"
          description={
            st.kind === "ready" && st.packed.as_at
              ? `By NetSuite's Packed By. Updated ${ago(st.packed.as_at)} ago.`
              : "By NetSuite's Packed By."
          }
          padded={false}
        >
          <Toolbar>
            <Tabs
              aria-label="Over"
              value={bench.span}
              onValueChange={(v) => bench.choose(v as Span)}
              items={SPANS.map((p) => ({ value: p.value, label: p.label }))}
            />
          </Toolbar>
          <div className={s.body}>
            {st.kind === "loading" ? (
              <Stack gap={3}>
                <Skeleton width="70%" />
                <Skeleton width="45%" />
                <Skeleton width="30%" />
              </Stack>
            ) : st.packed.no_site ? (
              <Alert tone="info">Choose where you are working to see who packed there.</Alert>
            ) : st.packed.as_at === null ? (
              <EmptyState
                icon={<PackageCheck />}
                title="Nothing from NetSuite yet"
                description="The Spork Bridge sends who packed each order. Update it in Tampermonkey if this stays empty."
              />
            ) : (
              <Packers packers={st.packed.packers} span={bench.span} />
            )}
          </div>
        </Card>
      )}
    </Page>
  );
}

/** One bar a person, most first, the count at the bar's end; nobody named last. */
function Packers({ packers, span }: { packers: readonly PackerCounts[]; span: Span }) {
  const rows = packers
    .filter((p) => p[span] > 0)
    .sort((a, b) => Number(a.packed_by === null) - Number(b.packed_by === null) || b[span] - a[span] || (a.packed_by ?? "").localeCompare(b.packed_by ?? ""));
  const total = rows.reduce((n, p) => n + p[span], 0);
  const most = Math.max(1, ...rows.map((p) => p[span]));
  if (rows.length === 0) {
    return <EmptyState icon={<PackageCheck />} title={SPANS.find((p) => p.value === span)!.none} />;
  }
  const people = rows.filter((p) => p.packed_by !== null).length;
  return (
    <Stack gap={3}>
      <p className={s.total}>
        <strong>{total.toLocaleString()}</strong> {total === 1 ? "order" : "orders"}
        {people > 0 && ` by ${people === 1 ? "1 person" : `${people} people`}`}
      </p>
      <ul className={s.bars} aria-label="Orders packed by each person">
        {rows.map((p) => (
          <Tooltip
            key={p.packed_by ?? ""}
            side="top"
            content={
              <>
                <strong>{p[span].toLocaleString()}</strong> · {Math.round((p[span] / total) * 100)}% of {total.toLocaleString()}
              </>
            }
          >
            <li className={s.row} tabIndex={0}>
              <span className={p.packed_by === null ? s.nobody : s.name}>{p.packed_by ?? NOBODY}</span>
              <span className={s.track}>
                <span
                  className={p.packed_by === null ? s.barNobody : s.bar}
                  style={{ ["--share" as string]: p[span] / most }}
                  aria-hidden
                />
                <span className={s.count}>{p[span].toLocaleString()}</span>
              </span>
            </li>
          </Tooltip>
        ))}
      </ul>
    </Stack>
  );
}

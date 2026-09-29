import { Boxes, SquareDashed, Wand2 } from "lucide-react";

import {
  Alert,
  Button,
  Card,
  DataTable,
  EmptyState,
  Inline,
  Page,
  PageHeader,
  Section,
  Skeleton,
  Stack,
  Stat,
  StatGrid,
  type Column,
} from "@ui/index";
import { useNavigate } from "@app/routing/Router";
import type { DraftReport, DraftedPlace, LayoutPlace, LayoutView } from "@domain/types";

import type { LayoutDesk } from "./useLayout";
import s from "./layout.module.css";

/**
 * A site's layout, as a list (D173). **Never an empty canvas**: a site with
 * bins and no layout opens on one action, drafting it from the bin list, and
 * the draft is previewed before anything is made.
 */
export function LayoutPage({ desk }: { desk: LayoutDesk }) {
  const navigate = useNavigate();
  const read = desk.read;

  if (read.kind !== "ready") {
    return (
      <Page>
        <PageHeader title="Layout" />
        {read.kind === "failed" ? (
          <Alert tone="danger">{read.message}</Alert>
        ) : (
          <Card>
            <Stack gap={3}>
              <Skeleton width="40%" />
              <Skeleton width="70%" />
            </Stack>
          </Card>
        )}
      </Page>
    );
  }

  const v = read.value;
  const draft = desk.draft;
  const busy = draft.kind === "working";
  const quiet = draft.kind === "idle" || draft.kind === "failed";
  const drafting = (label: string, primary: boolean) => (
    <Button
      variant={primary ? "primary" : "secondary"}
      icon={<Wand2 />}
      loading={busy && draft.what === "preview"}
      disabled={busy}
      onClick={() => void desk.preview()}
    >
      {label}
    </Button>
  );

  return (
    <Page>
      <PageHeader
        title="Layout"
        description={`Where every bin at ${v.site_code} is. Open a place to see its bins.`}
        actions={v.places.length > 0 && v.unplaced > 0 && quiet ? drafting("Draft the rest", false) : undefined}
      />
      <Stack gap={5}>
        {draft.kind === "failed" && (
          <Alert tone="danger" onDismiss={desk.dismiss}>
            {draft.message}
          </Alert>
        )}
        {draft.kind === "applied" && (
          <Alert tone="success" onDismiss={desk.dismiss}>
            {applied(draft.report)}
          </Alert>
        )}
        {draft.kind === "previewed" && <Preview report={draft.report} desk={desk} />}

        {v.places.length === 0 && quiet && (
          <Card>
            <EmptyState
              icon={<Boxes />}
              title="This warehouse has no layout yet"
              description="Spork can draft one from the bin list: a rack for each family of bin codes, laid out in rows. You can see what it will make before it makes anything."
              action={v.bins > 0 ? drafting("Draft from bin list", true) : undefined}
            />
          </Card>
        )}

        {v.places.length > 0 && (
          <Card padded={false}>
            <StatGrid>
              <Stat label="Places" value={v.places.length.toLocaleString()} />
              <Stat label="Bins on the layout" value={(v.bins - v.unplaced).toLocaleString()} />
              <Stat label="Not on it yet" value={v.unplaced.toLocaleString()} tone={v.unplaced ? undefined : "muted"} />
            </StatGrid>
            <DataTable
              aria-label="Places"
              columns={PLACE_COLUMNS}
              rows={tree(v)}
              rowKey={(r) => r.place.place_id}
              onRowClick={(r) => navigate(`/places/${r.place.place_id}`)}
            />
          </Card>
        )}

        {v.places.length > 0 && v.unplaced > 0 && (
          <Section title="Not on the layout" count={v.unplaced}>
            <Card>
              <p className={s.note}>
                {sample(v.unplaced_sample, v.unplaced)}. Codes that follow no pattern can&apos;t be drafted; they wait here to be
                put in a place by hand.
              </p>
            </Card>
          </Section>
        )}
      </Stack>
    </Page>
  );
}

function Preview({ report, desk }: { report: DraftReport; desk: LayoutDesk }) {
  const busy = desk.draft.kind === "working";
  return (
    <Card title="Draft: nothing made yet" padded={false}>
      <StatGrid>
        <Stat label="New places" value={report.places.length.toLocaleString()} />
        <Stat label="Bins placed" value={(report.bins_placed + report.bins_filled).toLocaleString()} />
        <Stat label="Left over" value={report.unplaced.toLocaleString()} tone={report.unplaced ? undefined : "muted"} />
      </StatGrid>
      {report.places.length > 0 && (
        <DataTable aria-label="Places the draft would make" columns={DRAFT_COLUMNS} rows={report.places} rowKey={(p) => p.name} />
      )}
      <div className={s.previewFoot}>
        <p className={s.note}>
          {report.inside
            ? `${report.inside_created ? "A new place called" : "Inside"} ${report.inside}${report.inside_created ? " holds them" : ""}, in rows you can arrange afterwards.`
            : "Nothing new to draw."}
          {report.bins_filled > 0 && ` ${report.bins_filled} bins drop into places already there.`}
          {report.unplaced > 0 && ` Left over: ${sample(report.unplaced_sample, report.unplaced)}.`}
        </p>
        <Inline gap={2}>
          <Button variant="primary" loading={busy} disabled={busy} onClick={() => void desk.apply()}>
            Make these places
          </Button>
          <Button variant="secondary" disabled={busy} onClick={desk.dismiss}>
            Cancel
          </Button>
        </Inline>
      </div>
    </Card>
  );
}

function applied(r: DraftReport): string {
  const made = r.places.length === 1 ? "1 place" : `${r.places.length} places`;
  const placed = r.bins_placed + r.bins_filled;
  return `Made ${made} and put ${placed.toLocaleString()} ${placed === 1 ? "bin" : "bins"} on the layout.`;
}

/** A few codes and how many more, so a long list reads as a number. */
function sample(codes: string[], total: number): string {
  const more = total - codes.length;
  return codes.join(", ") + (more > 0 ? ` and ${more.toLocaleString()} more` : "");
}

interface TreeRow {
  place: LayoutPlace;
  depth: number;
}

/** The places in tree order, each under its parent. */
function tree(v: LayoutView): TreeRow[] {
  const kids = new Map<string | null, LayoutPlace[]>();
  const ids = new Set(v.places.map((p) => p.place_id));
  for (const p of v.places) {
    // A parent that is not in the list is treated as the site, so nothing is lost.
    const parent = p.parent_id && ids.has(p.parent_id) ? p.parent_id : null;
    kids.set(parent, [...(kids.get(parent) ?? []), p]);
  }
  const out: TreeRow[] = [];
  const seen = new Set<string>();
  const walk = (parent: string | null, depth: number) => {
    for (const p of kids.get(parent) ?? []) {
      if (seen.has(p.place_id)) continue;
      seen.add(p.place_id);
      out.push({ place: p, depth });
      walk(p.place_id, depth + 1);
    }
  };
  walk(null, 0);
  return out;
}

const PLACE_COLUMNS: Column<TreeRow>[] = [
  {
    key: "name",
    header: "Place",
    cell: (r) => (
      <span className={s.treeName} style={{ paddingLeft: `calc(var(--ui-space-5) * ${r.depth})` }}>
        {r.place.solid ? <Boxes aria-hidden className={s.treeIcon} /> : <SquareDashed aria-hidden className={s.treeIcon} />}
        {r.place.name}
      </span>
    ),
    grow: true,
  },
  {
    key: "grid",
    header: "Grid",
    cell: (r) => (r.place.bays * r.place.levels * r.place.rows > 1 ? `${r.place.bays} × ${r.place.levels}` : "—"),
    width: "110px",
  },
  { key: "pattern", header: "Bin names", cell: (r) => r.place.pattern ?? "—", mono: true, width: "200px" },
  { key: "bins", header: "Bins", cell: (r) => r.place.bins.toLocaleString(), align: "right", width: "90px" },
];

const DRAFT_COLUMNS: Column<DraftedPlace>[] = [
  { key: "name", header: "Place", cell: (p) => p.name, grow: true },
  { key: "grid", header: "Bays × levels", cell: (p) => `${p.bays} × ${p.levels}`, width: "130px" },
  { key: "pattern", header: "Bin names", cell: (p) => p.pattern, mono: true, width: "220px" },
  { key: "bins", header: "Bins", cell: (p) => p.bins.toLocaleString(), align: "right", width: "90px" },
];

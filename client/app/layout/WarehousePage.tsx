import { Component, Suspense, lazy, useState, type ReactNode } from "react";
import { Boxes, Inbox, PencilRuler, Rotate3d, SquareDashed, Wand2 } from "lucide-react";

import {
  Alert,
  Button,
  Card,
  Checkbox,
  DataTable,
  EmptyState,
  Inline,
  Page,
  PageHeader,
  SearchField,
  Skeleton,
  Stack,
  Stat,
  StatGrid,
  cx,
  type Column,
} from "@ui/index";
import { useNavigate } from "@app/routing/Router";
import { Faint } from "@app/common/cells";
import type { BinRow, DraftReport, DraftedPlace, LayoutPlace, LayoutView, PlanShape } from "@domain/types";

import type { Chosen, WarehouseDesk } from "./useWarehouse";
import s from "./layout.module.css";

// three.js is its own chunk, fetched the first time the 3D view is shown.
const Site3D = lazy(() => import("./Site3D"));

/**
 * The warehouse: every place on the site, where it stands, and the bins in
 * each with what NetSuite last said is on them (D173).
 *
 * **Never an empty canvas**: a site with bins and no layout opens on one
 * action, drafting it from the bin list, and the draft is previewed before
 * anything is made. Once it has places, choosing one on the list or the plan
 * shows its bins; a search looks across the whole site.
 */
export function WarehousePage({ desk }: { desk: WarehouseDesk }) {
  const read = desk.read;
  const navigate = useNavigate();

  if (read.kind !== "ready") {
    return (
      <Page>
        <PageHeader title="Warehouse" />
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
        title="Warehouse"
        description={`Where every bin at ${v.site_code} is, and what NetSuite says is in it.`}
        actions={
          v.places.length > 0 ? (
            <>
              {v.unplaced > 0 && quiet && drafting("Draft the rest", false)}
              <Button icon={<PencilRuler />} onClick={() => navigate("/warehouse/edit")}>
                Edit layout
              </Button>
            </>
          ) : undefined
        }
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
          <div className={s.desk}>
            <Card title="Places" padded={false}>
              <StatGrid>
                <Stat label="On the layout" value={(v.bins - v.unplaced).toLocaleString()} />
                <Stat label="Not on it yet" value={v.unplaced.toLocaleString()} tone={v.unplaced ? undefined : "muted"} />
              </StatGrid>
              <PlaceList rows={rowsOf(v)} chosen={desk.asked.trim() ? null : desk.chosen} choose={desk.choose} />
            </Card>
            <div className={s.chosen}>
              <SitePlan shapes={v.plan} places={v.places} chosen={desk.asked.trim() ? null : desk.chosen} choose={desk.choose} />
              <Bins v={v} desk={desk} />
            </div>
          </div>
        )}
      </Stack>
    </Page>
  );
}

/**
 * The bins of what is chosen, or of the search. A row opens the bin, which
 * draws it on the face of its rack.
 */
function Bins({ v, desk }: { v: LayoutView; desk: WarehouseDesk }) {
  const navigate = useNavigate();
  const bins = desk.bins;
  const ready = bins.kind === "ready" ? bins.value : null;
  const searching = desk.asked.trim();
  const place = v.places.find((p) => p.place_id === desk.chosen);
  const title = searching
    ? `Bins matching “${searching}”`
    : desk.chosen === "unplaced"
      ? "Not on the layout"
      : place
        ? `Bins in ${place.name}`
        : "Bins";
  const description = searching
    ? "Across the whole site."
    : desk.chosen === "unplaced"
      ? "Codes that follow no pattern can’t be drafted; they wait here to be put in a place by hand."
      : place?.pattern
        ? `Named ${place.pattern}`
        : undefined;

  return (
    <Card
      title={title}
      description={description}
      padded={false}
      actions={
        <form
          className={s.search}
          onSubmit={(e) => {
            e.preventDefault();
            desk.search();
          }}
        >
          <SearchField aria-label="Find a bin" placeholder="Find a bin" value={desk.typed} onChange={(e) => desk.type(e.target.value)} />
          <Button type="submit">Find</Button>
        </form>
      }
    >
      {bins.kind === "failed" ? (
        <div className={s.inset}>
          <Alert tone="danger">{bins.message}</Alert>
        </div>
      ) : (
        <DataTable
          aria-label="Bins"
          columns={searching ? BIN_COLUMNS_ANYWHERE : desk.chosen === "unplaced" ? BIN_COLUMNS_NOWHERE : BIN_COLUMNS}
          rows={ready?.bins ?? []}
          rowKey={(b) => b.location_id}
          loading={bins.kind === "loading"}
          onRowClick={(b) => navigate(`/bins/${b.location_id}`)}
          empty={
            <EmptyState
              icon={<Inbox />}
              title={searching ? "No bin matches" : bins.kind === "idle" ? "Choose a place" : "No bins here"}
              description={
                searching
                  ? "Try part of the code."
                  : bins.kind === "idle"
                    ? "Choose a place on the list or the plan to see its bins."
                    : "Nothing on the bin list is in this place's own cells."
              }
            />
          }
        />
      )}
      {ready && ready.total > ready.bins.length && (
        <div className={s.foot}>
          <Faint>
            The first {ready.bins.length.toLocaleString()} of {ready.total.toLocaleString()} bins. Search for a code to find the rest.
          </Faint>
        </div>
      )}
    </Card>
  );
}

/**
 * The whole site from above, the same way up every time. Choosing a place on
 * it is choosing it on the list; the outermost place, which everything else is
 * inside, is the background rather than a target.
 *
 * The 3D view sits beside it when shown, drawing the same places and sharing
 * the same choice. 2D answers where; 3D confirms what it looks like.
 */
function SitePlan({
  shapes,
  places,
  chosen,
  choose,
}: {
  shapes: PlanShape[];
  places: LayoutPlace[];
  chosen: Chosen;
  choose: (c: Chosen) => void;
}) {
  const [shown, toggle] = useShown3D();
  if (shapes.length < 2) return null;
  const xs = shapes.flatMap((sh) => sh.corners.map((c) => c[0]));
  const ys = shapes.flatMap((sh) => sh.corners.map((c) => c[1]));
  const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const pad = Math.max(maxX - minX, maxY - minY) * 0.03 || 1;
  // The site's y runs up the page; SVG's runs down, so it is drawn negated.
  const box = `${minX - pad} ${-maxY - pad} ${maxX - minX + 2 * pad} ${maxY - minY + 2 * pad}`;
  // Outermost first, so what is inside is drawn over it, and the chosen place
  // last so its outline is never under a neighbour's.
  const drawn = [...shapes].sort((a, b) => Number(a.place_id === chosen) - Number(b.place_id === chosen) || a.nesting - b.nesting);
  const named = shapes.find((sh) => sh.place_id === chosen);

  return (
    <Card
      title="Site plan"
      actions={
        <Button size="sm" icon={<Rotate3d />} onClick={toggle}>
          {shown ? "Hide 3D" : "Show 3D"}
        </Button>
      }
    >
      <div className={cx(s.views, shown && s.split)}>
        <svg
          className={cx(s.plan, s.sitePlan)}
          viewBox={box}
          role="img"
          aria-label={named ? `Plan of the site, with ${named.name} marked` : "Plan of the site"}
        >
          {drawn.map((sh) => (
            <polygon
              key={sh.place_id}
              points={sh.corners.map(([x, y]) => `${x},${-y}`).join(" ")}
              vectorEffect="non-scaling-stroke"
              className={cx(
                s.shape,
                sh.nesting === 0 ? s.outer : sh.solid ? s.solid : s.floor,
                sh.nesting > 0 && s.target,
                sh.place_id === chosen && s.focus,
              )}
              onClick={sh.nesting > 0 ? () => choose(sh.place_id) : undefined}
            >
              <title>{sh.name}</title>
            </polygon>
          ))}
        </svg>
        {shown && (
          <Unless3D>
            <Suspense fallback={<div className={s.scene} />}>
              <Site3D plan={shapes} places={places} chosen={chosen === "unplaced" ? null : chosen} choose={choose} />
            </Suspense>
          </Unless3D>
        )}
      </div>
    </Card>
  );
}

const SHOWN_3D = "spork.warehouse.3d";

/**
 * Whether the 3D view is shown: as this browser last left it, and at first
 * only where there is room for it beside the plan. Remembered per browser, as
 * the sidebar is.
 */
function useShown3D(): [boolean, () => void] {
  const [shown, setShown] = useState(() => {
    try {
      const v = localStorage.getItem(SHOWN_3D);
      if (v === "1" || v === "0") return v === "1";
    } catch {
      // not remembered: fall through to the default
    }
    return typeof matchMedia === "function" && matchMedia("(min-width: 1100px)").matches;
  });
  const toggle = () =>
    setShown((was) => {
      try {
        localStorage.setItem(SHOWN_3D, was ? "0" : "1");
      } catch {
        // not remembered, still toggled
      }
      return !was;
    });
  return [shown, toggle];
}

/** If the 3D view cannot load, it says so in its own pane and the plan stays. */
class Unless3D extends Component<{ children: ReactNode }, { failed: boolean }> {
  override state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  override render() {
    return this.state.failed ? (
      <div className={s.scene}>
        <p className={s.sceneNote}>The 3D view could not be loaded. The plan beside it is complete.</p>
      </div>
    ) : (
      this.props.children
    );
  }
}

/**
 * What the draft would make, before anything is made. Each place can be left
 * out, for a family of codes that is no rack at all: its bins stay in the tray
 * with the codes no pattern fitted. A rack can be made with a face on each
 * side, numbered round it (its last bay behind its first). The counts follow
 * the ticks.
 */
function Preview({ report, desk }: { report: DraftReport; desk: WarehouseDesk }) {
  const busy = desk.draft.kind === "working";
  const making = report.places.filter((p) => !desk.leftOut.has(p.name));
  const splits = (p: DraftedPlace) => p.split !== null && desk.twoSided.has(p.name);
  const leftBins = report.places.filter((p) => desk.leftOut.has(p.name)).reduce((n, p) => n + p.bins, 0);
  const placed = report.bins_filled + making.reduce((n, p) => n + p.bins, 0);
  const over = report.unplaced + leftBins;
  const places = making.length;
  const racks = making.filter((p) => p.split !== null).map((p) => p.name);
  const everyTwo = racks.length > 0 && racks.every((n) => desk.twoSided.has(n));
  const columns: Column<DraftedPlace>[] = [
    {
      key: "name",
      header: "Make",
      cell: (p) => <Checkbox label={p.name} checked={!desk.leftOut.has(p.name)} onCheckedChange={(on) => desk.leaveOut(p.name, !on)} />,
      grow: true,
    },
    {
      key: "sides",
      header: "Two sides",
      cell: (p) =>
        p.split ? (
          <Checkbox
            label={`${p.split[0]} front, ${p.split[1]} back`}
            checked={splits(p)}
            disabled={desk.leftOut.has(p.name)}
            onCheckedChange={(on) => desk.setTwoSided([p.name], on)}
          />
        ) : (
          <Faint>One side</Faint>
        ),
      width: "240px",
    },
    ...DRAFT_COLUMNS,
  ];
  return (
    <Card
      title="Draft: nothing made yet"
      padded={false}
      actions={
        racks.length > 1 ? (
          <Button size="sm" disabled={busy} onClick={() => desk.setTwoSided(racks, !everyTwo)}>
            {everyTwo ? "One side for every rack" : "Two sides for every rack"}
          </Button>
        ) : undefined
      }
    >
      <StatGrid>
        <Stat label="New places" value={places.toLocaleString()} />
        <Stat label="Bins placed" value={placed.toLocaleString()} />
        <Stat label="Left over" value={over.toLocaleString()} tone={over ? undefined : "muted"} />
      </StatGrid>
      {report.places.length > 0 && (
        <DataTable aria-label="Places the draft would make" columns={columns} rows={report.places} rowKey={(p) => p.name} />
      )}
      <div className={s.previewFoot}>
        <p className={s.note}>
          {places > 0 && report.inside
            ? `${report.inside_created ? "A new place called" : "Inside"} ${report.inside}${report.inside_created ? " holds them" : ""}, in rows you can arrange afterwards.`
            : "Nothing new to draw."}
          {report.places.length > 0 &&
            " Untick a place that is no rack, and its bins stay off the layout. Tick Two sides for a rack with bins on both faces."}
          {report.bins_filled > 0 && ` ${report.bins_filled} bins drop into places already there.`}
          {report.unplaced > 0 && ` Left over: ${sample(report.unplaced_sample, report.unplaced)}.`}
        </p>
        <Inline gap={2}>
          <Button
            variant="primary"
            loading={busy}
            disabled={busy || placed === 0}
            onClick={() => void desk.apply()}
          >
            {places === 0 ? "Place these bins" : places === 1 ? "Make 1 place" : `Make ${places} places`}
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
  const left =
    r.left_out.length > 0
      ? ` Left out ${r.left_out.join(", ")}; ${r.left_out.length === 1 ? "its" : "their"} bins are under Not on the layout.`
      : "";
  return `Made ${made} and put ${placed.toLocaleString()} ${placed === 1 ? "bin" : "bins"} on the layout.${left}`;
}

/** A few codes and how many more, so a long list reads as a number. */
function sample(codes: string[], total: number): string {
  const more = total - codes.length;
  return codes.join(", ") + (more > 0 ? ` and ${more.toLocaleString()} more` : "");
}

/** A row on the places list: a place, indented under its parent, or the tray. */
interface PlaceRow {
  key: string;
  name: string;
  depth: number;
  icon: "solid" | "floor" | "tray";
  bins: number;
}

/** The places in tree order, each under its parent, and then the tray. */
function rowsOf(v: LayoutView): PlaceRow[] {
  const kids = new Map<string | null, LayoutView["places"]>();
  const ids = new Set(v.places.map((p) => p.place_id));
  for (const p of v.places) {
    // A parent that is not in the list is treated as the site, so nothing is lost.
    const parent = p.parent_id && ids.has(p.parent_id) ? p.parent_id : null;
    kids.set(parent, [...(kids.get(parent) ?? []), p]);
  }
  const out: PlaceRow[] = [];
  const seen = new Set<string>();
  const walk = (parent: string | null, depth: number) => {
    for (const p of kids.get(parent) ?? []) {
      if (seen.has(p.place_id)) continue;
      seen.add(p.place_id);
      out.push({ key: p.place_id, name: p.name, depth, icon: p.solid ? "solid" : "floor", bins: p.bins });
      walk(p.place_id, depth + 1);
    }
  };
  walk(null, 0);
  if (v.unplaced > 0) out.push({ key: "unplaced", name: "Not on the layout", depth: 0, icon: "tray", bins: v.unplaced });
  return out;
}

const ICONS = { solid: Boxes, floor: SquareDashed, tray: Inbox };

/**
 * The places, each under its parent. A list rather than a table: it is one
 * name and one count, in a column too narrow for a table to stay a table.
 */
function PlaceList({ rows, chosen, choose }: { rows: PlaceRow[]; chosen: Chosen; choose: (c: Chosen) => void }) {
  return (
    <ul className={s.children} aria-label="Places">
      {rows.map((r) => {
        const Icon = ICONS[r.icon];
        return (
          <li key={r.key}>
            <button
              type="button"
              className={cx(s.child, s.pick, chosen === r.key && s.picked)}
              style={{ paddingLeft: `calc(var(--ui-space-4) + var(--ui-space-5) * ${r.depth})` }}
              aria-pressed={chosen === r.key}
              onClick={() => choose(r.key)}
            >
              <Icon aria-hidden className={s.childIcon} />
              <span className={s.childName}>{r.name}</span>
              <span className={s.childMeta}>{r.bins ? `${r.bins.toLocaleString()} ${r.bins === 1 ? "bin" : "bins"}` : "No bins"}</span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

/** What NetSuite last put on the shelf: the biggest first, and how many more. */
function Reported({ bin }: { bin: BinRow }) {
  if (bin.reported_items === 0) return <Faint>—</Faint>;
  const [top] = bin.reported;
  const more = bin.reported_items - 1;
  return (
    <span className={s.reported}>
      <span className={s.code}>{top?.item_code}</span> × {top?.on_hand}
      {more > 0 && <Faint> · {more === 1 ? "1 more item" : `${more} more items`}</Faint>}
    </span>
  );
}

/** The bin grows: stacked on a handheld, the growing column is each row's title. */
const BIN: Column<BinRow> = { key: "bin", header: "Bin", cell: (b) => <span className={s.code}>{b.code}</span>, grow: true };
const REPORTED: Column<BinRow> = { key: "netsuite", header: "NetSuite says", cell: (b) => <Reported bin={b} />, width: "240px" };
const HELD: Column<BinRow> = {
  key: "held",
  header: "Spork holds",
  cell: (b) => (b.held ? b.held.toLocaleString() : <Faint>—</Faint>),
  align: "right",
  mono: true,
  width: "110px",
};

const BIN_COLUMNS: Column<BinRow>[] = [
  BIN,
  { key: "where", header: "Where", cell: (b) => b.whereabouts ?? <Faint>—</Faint>, width: "170px" },
  REPORTED,
  HELD,
];

/** Found across the site, a bin says which place it is in as well. */
const BIN_COLUMNS_ANYWHERE: Column<BinRow>[] = [
  BIN,
  {
    key: "where",
    header: "Where",
    cell: (b) =>
      b.place_name ? (
        <span className={s.named}>
          <span>{b.place_name}</span>
          {b.whereabouts && <Faint>{b.whereabouts}</Faint>}
        </span>
      ) : (
        <Faint>Not on the layout</Faint>
      ),
    width: "170px",
  },
  REPORTED,
  HELD,
];

/** The tray has no cells, so nothing to say where. */
const BIN_COLUMNS_NOWHERE: Column<BinRow>[] = [BIN, REPORTED, HELD];

/** After the place's own column, which ticks it in or out. */
const DRAFT_COLUMNS: Column<DraftedPlace>[] = [
  { key: "grid", header: "Bays × levels", cell: (p) => `${p.bays} × ${p.levels}`, width: "130px" },
  { key: "pattern", header: "Bin names", cell: (p) => p.pattern, mono: true, width: "220px" },
  { key: "bins", header: "Bins", cell: (p) => p.bins.toLocaleString(), align: "right", width: "90px" },
];

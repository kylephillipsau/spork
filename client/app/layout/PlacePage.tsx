import { Fragment, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { Boxes, ChevronRight, MapPin, SquareDashed } from "lucide-react";

import { Alert, Card, EmptyState, Link, Page, PageHeader, Section, Skeleton, Stack, Tabs, cx } from "@ui/index";
import type { GridCell, PlaceView } from "@domain/types";

import type { BinDesk, PlaceDesk, Read } from "./usePlace";
import s from "./layout.module.css";

/**
 * Where a bin is, drawn as the face of the rack that holds it (D173).
 *
 * **A scan lands here.** The worker sees the way into the place, the face of
 * the rack with the bin's spot lit, labelled the way the rack's own labels
 * read, and a small plan to find the rack on, marked on the side to stand. 2D,
 * because finding which bay and which level is a question about relative
 * position, which a flat drawing answers better than a 3D one; nothing here
 * can be edited.
 *
 * A rack with a face on each side opens on the side the bin is on, drawn as
 * you would see it standing there, with the other side a tap away.
 */
export function BinPage({ desk }: { desk: BinDesk }) {
  const read = desk.read;
  if (read.kind !== "ready") return <Waiting title="Bin" read={read} />;
  const bin = read.value;
  const place = bin.place;

  if (!place || !bin.cell) {
    return (
      <Page>
        <PageHeader title={<span className={s.code}>{bin.code}</span>} description="Not on the layout yet." />
        <Card>
          <EmptyState
            icon={<MapPin />}
            title="This bin hasn't been put on the layout yet"
            description="Once it has, scanning it shows the rack it is on, with its spot lit up."
          />
        </Card>
      </Page>
    );
  }

  return (
    <Page>
      <Trail place={place} />
      <PageHeader title={<span className={s.code}>{bin.code}</span>} description={sayWhere(place, bin.cell)} />
      <Located place={place} target={{ cell: bin.cell, code: bin.code }} holds />
    </Page>
  );
}

/** One place: its face if it holds bins, what is inside it, and the plan. */
export function PlacePage({ desk }: { desk: PlaceDesk }) {
  const read = desk.read;
  if (read.kind !== "ready") return <Waiting title="Place" read={read} />;
  const place = read.value;
  const holds = place.bins.length > 0 || place.bays * place.levels * place.rows > 1;

  return (
    <Page>
      <Trail place={place} />
      <PageHeader title={place.name} description={describe(place)} />
      <Located place={place} target={null} holds={holds}>
        {place.children.length > 0 && (
          <Section title="Inside" count={place.children.length}>
            <Card padded={false}>
              <ul className={s.children}>
                {place.children.map((c) => (
                  <li key={c.place_id}>
                    <Link variant="plain" href={`/places/${c.place_id}`} className={s.child}>
                      {c.solid ? <Boxes aria-hidden className={s.childIcon} /> : <SquareDashed aria-hidden className={s.childIcon} />}
                      <span className={s.childName}>{c.name}</span>
                      <span className={s.childMeta}>{c.bins === 1 ? "1 bin" : `${c.bins} bins`}</span>
                      <ChevronRight aria-hidden className={s.childIcon} />
                    </Link>
                  </li>
                ))}
              </ul>
            </Card>
          </Section>
        )}
      </Located>
    </Page>
  );
}

/**
 * A place's face and where it stands, sharing which side is shown: turning to
 * the back of a rack moves the plan's mark to the aisle behind it.
 */
function Located({
  place,
  target,
  holds,
  children,
}: {
  place: PlaceView;
  target: { cell: GridCell; code: string } | null;
  holds: boolean;
  children?: ReactNode;
}) {
  const [side, setSide] = useState(target?.cell.side ?? 1);
  return (
    <Stack gap={5}>
      {holds && <Face place={place} target={target} side={side} setSide={setSide} />}
      {children}
      <Plan place={place} side={side} />
    </Stack>
  );
}

function Waiting({ title, read }: { title: string; read: Exclude<Read<unknown>, { kind: "ready" }> }) {
  return (
    <Page>
      <PageHeader title={title} />
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

/**
 * "Rack C · bay 05, level 3": plain words, never left or right (D173). On a
 * rack with two sides, which: "Rack E · back, bay 36, level 01".
 */
export function sayWhere(place: PlaceView, cell: GridCell): string {
  const parts = place.sides > 1 ? [cell.side === 2 ? "back" : "front"] : [];
  parts.push(`bay ${bayLabel(place, cell.side, cell.bay)}`);
  if (place.levels > 1) parts.push(`level ${place.level_labels[cell.level - 1] ?? cell.level}`);
  if ((place.positions[cell.level - 1] ?? 1) > 1) parts.push(`position ${cell.position}`);
  if (place.rows > 1) parts.push(cell.row === 1 ? "front row" : `row ${cell.row}`);
  return `${place.name} · ${parts.join(", ")}`;
}

/** A column's label on a side: the back's read from the far end, as you face it. */
function bayLabel(place: PlaceView, side: number, bay: number): string {
  const label = side === 2 ? place.back_labels[place.bays - bay] : place.bay_labels[bay - 1];
  return label ?? String(bay);
}

function describe(place: PlaceView): string {
  const bins = place.bins.length === 1 ? "1 bin" : `${place.bins.length} bins`;
  if (place.bays * place.levels * place.rows * place.sides > 1) {
    const bays = `${place.bays} ${place.bays === 1 ? "bay" : "bays"}`;
    const grid = [place.sides > 1 ? `two sides of ${bays}` : bays];
    if (place.levels > 1) grid.push(`${place.levels} levels`);
    if (place.rows > 1) grid.push(`${place.rows} rows`);
    return `${grid.join(", ")} · ${bins}`;
  }
  if (place.children.length > 0) {
    return place.children.length === 1 ? "1 place inside" : `${place.children.length} places inside`;
  }
  return place.solid ? "Solid" : "Floor";
}

/** The way up out of a place, as big chips a thumb can hit. */
function Trail({ place }: { place: PlaceView }) {
  if (place.trail.length === 0) return null;
  return (
    <nav aria-label="Where this is" className={s.trail}>
      {place.trail.map((c) => (
        <Fragment key={c.place_id}>
          <Link variant="plain" href={`/places/${c.place_id}`} className={s.chip}>
            {c.name}
          </Link>
          <ChevronRight aria-hidden className={s.trailSep} />
        </Fragment>
      ))}
      <span className={cx(s.chip, s.here)} aria-current="location">
        {place.name}
      </span>
    </nav>
  );
}

const range = (n: number) => Array.from({ length: n }, (_, i) => i + 1);
const key = (c: GridCell) => `${c.side}.${c.bay}.${c.level}.${c.row}.${c.position}`;

/** "01–18", from a side's labels as you face it. */
const span = (labels: string[]) => (labels.length > 1 ? `${labels[0]}–${labels[labels.length - 1]}` : (labels[0] ?? ""));

/**
 * A face of a place: bays across, levels up, the top level at the top, with
 * the labels written as the rack prints them. The target's spot is filled
 * with the accent and scrolled into view; an empty cell is hatched.
 *
 * The back of a rack is drawn as you would stand facing it: its columns run
 * from the far end, so its labels read left to right in order.
 */
function Face({
  place,
  target,
  side,
  setSide,
}: {
  place: PlaceView;
  target: { cell: GridCell; code: string } | null;
  side: number;
  setSide: (side: number) => void;
}) {
  const [row, setRow] = useState(target?.cell.row ?? 1);
  const lit = useRef<HTMLDivElement>(null);
  const byCell = useMemo(() => new Map(place.bins.map((b) => [key(b.cell), b])), [place.bins]);

  useEffect(() => {
    lit.current?.scrollIntoView?.({ block: "nearest", inline: "center" });
  }, [side]);

  const levels = range(place.levels).reverse();
  const back = side === 2;
  const columns = back ? range(place.bays).reverse() : range(place.bays);
  const labels = back ? place.back_labels : place.bay_labels;
  const style = { "--bays": place.bays } as CSSProperties;
  const elsewhere = target !== null && target.cell.side !== side;

  return (
    <Card padded={false}>
      {place.sides > 1 && (
        <div className={s.rows}>
          <Tabs
            aria-label="Side"
            value={String(side)}
            onValueChange={(v) => setSide(Number(v))}
            items={[
              { value: "1", label: `Front · ${span(place.bay_labels)}` },
              { value: "2", label: `Back · ${span(place.back_labels)}` },
            ]}
          />
        </div>
      )}
      {place.rows > 1 && (
        <div className={s.rows}>
          <Tabs
            aria-label="Row"
            value={String(row)}
            onValueChange={(v) => setRow(Number(v))}
            items={range(place.rows).map((r) => ({ value: String(r), label: r === 1 ? "Front row" : `Row ${r}` }))}
          />
        </div>
      )}
      <div className={s.faceScroll}>
        <div className={s.face} style={style} role="group" aria-label={`${back ? "Back" : "Front"} of ${place.name}`}>
          {levels.map((level) => (
            <Fragment key={level}>
              <div className={s.levelLabel}>{place.level_labels[level - 1]}</div>
              {columns.map((bay) => (
                <div key={bay} className={s.bay}>
                  {range(place.positions[level - 1] ?? 1).map((position, _, all) => {
                    const cell = { bay, level, row, position, side };
                    const bin = byCell.get(key(cell));
                    const on = target !== null && key(target.cell) === key(cell);
                    return (
                      <div
                        key={position}
                        ref={on ? lit : undefined}
                        className={cx(s.slot, !bin && s.empty, on && s.lit)}
                        aria-current={on ? "location" : undefined}
                        title={bin?.code}
                      >
                        {bin ? (
                          // A bay split into positions has no room for a whole code
                          // in each; the position is what tells them apart.
                          <span className={s.slotCode}>{all.length > 1 ? position : bin.code}</span>
                        ) : (
                          <span className={s.hidden}>empty</span>
                        )}
                      </div>
                    );
                  })}
                </div>
              ))}
            </Fragment>
          ))}
          <div className={s.axis}>Bay</div>
          {labels.map((label, i) => (
            <div key={i} className={s.bayLabel}>
              {label}
            </div>
          ))}
        </div>
      </div>
      {target && (
        <p className={s.tip}>
          {elsewhere
            ? `${target.code} is on the ${target.cell.side === 2 ? "back" : "front"} of this rack.`
            : "The lit square is where the bin is. Levels go up the side, bays along the bottom."}
        </p>
      )}
    </Card>
  );
}

/**
 * Where the place is in the building: the outermost place it is inside and
 * everything in that, top-down, the same way up every time so a worker learns
 * one map. A rack is marked in the aisle on the side shown: where to stand.
 */
function Plan({ place, side }: { place: PlaceView; side: number }) {
  const shapes = place.plan;
  const outer = shapes.find((sh) => sh.nesting === 0);
  if (!outer || shapes.length < 2) return null;

  const xs = shapes.flatMap((sh) => sh.corners.map((c) => c[0]));
  const ys = shapes.flatMap((sh) => sh.corners.map((c) => c[1]));
  const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const pad = Math.max(maxX - minX, maxY - minY) * 0.03 || 1;
  // The site's y runs up the page; SVG's runs down, so it is drawn negated.
  const box = `${minX - pad} ${-maxY - pad} ${maxX - minX + 2 * pad} ${maxY - minY + 2 * pad}`;
  // Outermost first, so what is inside is drawn over it, and the place itself
  // last unless it is the one everything else is inside.
  const last = (sh: (typeof shapes)[number]) => (sh.place_id === place.place_id && sh.nesting > 0 ? 1 : 0);
  const drawn = [...shapes].sort((a, b) => last(a) - last(b) || a.nesting - b.nesting);
  const me = shapes.find((sh) => sh.place_id === place.place_id && sh.nesting > 0);
  const stand = me && place.solid ? standing(me.corners, side) : null;
  const facing = stand ? `, from its ${side === 2 ? "back" : "front"}` : "";

  return (
    <Section title="On the plan">
      <Card>
        <svg className={s.plan} viewBox={box} role="img" aria-label={`Plan of ${outer.name}, with ${place.name} marked${facing}`}>
          {drawn.map((sh) => (
            <polygon
              key={sh.place_id}
              points={sh.corners.map(([x, y]) => `${x},${-y}`).join(" ")}
              vectorEffect="non-scaling-stroke"
              className={cx(
                s.shape,
                sh.nesting === 0 ? s.outer : sh.solid ? s.solid : s.floor,
                sh.place_id === place.place_id && s.focus,
              )}
            />
          ))}
          {stand && (
            <line
              x1={stand[0][0]}
              y1={-stand[0][1]}
              x2={stand[1][0]}
              y2={-stand[1][1]}
              vectorEffect="non-scaling-stroke"
              className={s.stand}
            />
          )}
        </svg>
      </Card>
    </Section>
  );
}

/**
 * Where to stand to face a side of a rectangle: a line along that face, half
 * a cell out into the aisle. The front is the edge from its first corner to
 * its second, the back the edge across from it.
 */
function standing(corners: [number, number][], side: number): [[number, number], [number, number]] | null {
  if (corners.length !== 4) return null;
  const [a, b, c, d] = corners as [[number, number], [number, number], [number, number], [number, number]];
  const [p, q, inner] = side === 2 ? [c, d, a] : [a, b, d];
  // Out from the face, away from the side across from it.
  const [ox, oy] = side === 2 ? [d[0] - inner[0], d[1] - inner[1]] : [a[0] - inner[0], a[1] - inner[1]];
  const length = Math.hypot(ox, oy) || 1;
  const [dx, dy] = [(ox / length) * 0.5, (oy / length) * 0.5];
  return [
    [p[0] + dx, p[1] + dy],
    [q[0] + dx, q[1] + dy],
  ];
}

import { Fragment, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { Boxes, ChevronRight, MapPin, SquareDashed } from "lucide-react";

import { Alert, Card, EmptyState, Link, Page, PageHeader, Section, Skeleton, Stack, Tabs, cx } from "@ui/index";
import type { GridCell, PlaceView } from "@domain/types";

import type { BinDesk, PlaceDesk, Read } from "./usePlace";
import s from "./layout.module.css";

/**
 * Where a bin is, drawn as the face of the rack that holds it (D173).
 *
 * **A scan lands here.** The worker sees the way into the place, the front of
 * the rack with the bin's spot lit, labelled the way the rack's own labels
 * read, and a small plan to find the rack on. 2D, because finding which bay and
 * which level is a question about relative position, which a flat drawing
 * answers better than a 3D one; nothing here can be edited.
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
      <Stack gap={5}>
        <Face place={place} target={{ cell: bin.cell, code: bin.code }} />
        <Plan place={place} />
      </Stack>
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
      <Stack gap={5}>
        {holds && <Face place={place} target={null} />}
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
        <Plan place={place} />
      </Stack>
    </Page>
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

/** "Rack C · bay 05, level 3": plain words, never left or right (D173). */
export function sayWhere(place: PlaceView, cell: GridCell): string {
  const parts = [`bay ${place.bay_labels[cell.bay - 1] ?? cell.bay}`];
  if (place.levels > 1) parts.push(`level ${place.level_labels[cell.level - 1] ?? cell.level}`);
  if ((place.positions[cell.level - 1] ?? 1) > 1) parts.push(`position ${cell.position}`);
  if (place.rows > 1) parts.push(cell.row === 1 ? "front row" : `row ${cell.row}`);
  return `${place.name} · ${parts.join(", ")}`;
}

function describe(place: PlaceView): string {
  const bins = place.bins.length === 1 ? "1 bin" : `${place.bins.length} bins`;
  if (place.bays * place.levels * place.rows > 1) {
    const grid = [`${place.bays} ${place.bays === 1 ? "bay" : "bays"}`];
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
const key = (c: GridCell) => `${c.bay}.${c.level}.${c.row}.${c.position}`;

/**
 * The front of a place: bays across, levels up, the top level at the top, with
 * the labels written as the rack prints them. The target's spot is filled
 * with the accent and scrolled into view; an empty cell is hatched.
 */
function Face({ place, target }: { place: PlaceView; target: { cell: GridCell; code: string } | null }) {
  const [row, setRow] = useState(target?.cell.row ?? 1);
  const lit = useRef<HTMLDivElement>(null);
  const byCell = useMemo(() => new Map(place.bins.map((b) => [key(b.cell), b])), [place.bins]);

  useEffect(() => {
    lit.current?.scrollIntoView?.({ block: "nearest", inline: "center" });
  }, []);

  const levels = range(place.levels).reverse();
  const style = { "--bays": place.bays } as CSSProperties;

  return (
    <Card padded={false}>
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
        <div className={s.face} style={style} role="group" aria-label={`Front of ${place.name}`}>
          {levels.map((level) => (
            <Fragment key={level}>
              <div className={s.levelLabel}>{place.level_labels[level - 1]}</div>
              {range(place.bays).map((bay) => (
                <div key={bay} className={s.bay}>
                  {range(place.positions[level - 1] ?? 1).map((position, _, all) => {
                    const cell = { bay, level, row, position };
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
          {place.bay_labels.map((label, i) => (
            <div key={i} className={s.bayLabel}>
              {label}
            </div>
          ))}
        </div>
      </div>
      {target && <p className={s.tip}>The lit square is where the bin is. Levels go up the side, bays along the bottom.</p>}
    </Card>
  );
}

/**
 * Where the place is in the building: the outermost place it is inside and
 * everything in that, top-down, the same way up every time so a worker learns
 * one map.
 */
function Plan({ place }: { place: PlaceView }) {
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

  return (
    <Section title="On the plan">
      <Card>
        <svg className={s.plan} viewBox={box} role="img" aria-label={`Plan of ${outer.name}, with ${place.name} marked`}>
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
        </svg>
      </Card>
    </Section>
  );
}

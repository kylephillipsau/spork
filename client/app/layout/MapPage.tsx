import { Suspense, lazy, useMemo, useState } from "react";
import { ArrowRight } from "lucide-react";

import { Alert, Badge, Checkbox, Link, Page, PageHeader, SearchField, Select, Skeleton, Tabs } from "@ui/index";
import { href } from "@app/routing/location";
import { ago } from "@app/common/cells";
import { ItemDrawer, ItemLine } from "@app/items/ItemProperties";
import type { BinView, MapBin, Uuid, WalkRoute } from "@domain/types";

import { findBins, LAYERS, LEGEND, swatch, type Layer } from "./layers";
import { sayWhere } from "./PlacePage";
import type { MapDesk } from "./useMap";
import s from "./map.module.css";

const Map3D = lazy(() => import("./Map3D"));

/**
 * The bin map (D208): every bin on the site where it sits, coloured by what is
 * on it or by whether it can be reached from the floor. Search for a bin and
 * the view flies to its face; click one and its card says what is in it, each
 * item opening beside the map as it does on the packing bench (D221). The card
 * is also where a rack's reach is set, a rack at a time.
 */
export function MapPage({ desk }: { desk: MapDesk }) {
  const [typed, setTyped] = useState("");
  const site = desk.read.kind === "ready" ? desk.read.value : null;
  const found = useMemo(() => (site ? findBins(site.bins.bins, typed) : []), [site, typed]);
  const go = (bin: MapBin) => {
    desk.find(bin);
    setTyped("");
  };

  return (
    <Page>
      <PageHeader title="Bin map" description="Every bin where it sits, coloured by what's on it." />
      {desk.read.kind === "failed" && <Alert tone="danger">{desk.read.message}</Alert>}

      <div className={s.map}>
        <section className={s.stage} aria-label="The site">
          {site ? (
            <Suspense fallback={<div className={s.canvas} />}>
              <Map3D
                plan={site.layout.plan}
                places={site.layout.places}
                bins={site.bins.bins}
                layer={desk.layer}
                chosen={desk.chosen?.location_id ?? null}
                choose={desk.choose}
                flight={desk.flight}
                route={desk.showWalk && desk.walk.kind === "ready" ? (desk.walk.value?.path ?? null) : null}
              />
            </Suspense>
          ) : (
            <div className={s.canvas}>
              <Skeleton width="40%" />
            </div>
          )}

          <div className={s.finder}>
            <form
              role="search"
              onSubmit={(e) => {
                e.preventDefault();
                if (found[0]) go(found[0]);
              }}
            >
              <SearchField
                aria-label="Find a bin"
                placeholder="Find a bin"
                autoComplete="off"
                value={typed}
                onChange={(e) => setTyped(e.target.value)}
                disabled={!site}
              />
            </form>
            {typed.trim() !== "" && site && (
              <ul className={s.found} aria-label="Bins found">
                {found.length === 0 ? (
                  <li className={s.foundNone}>No bin on the map is called that.</li>
                ) : (
                  found.map((bin) => (
                    <li key={bin.location_id}>
                      <button type="button" className={s.foundItem} onClick={() => go(bin)}>
                        <span className={s.code}>{bin.code}</span>
                        <span className={s.foundWhere}>{site.layout.places.find((p) => p.place_id === bin.place_id)?.name}</span>
                      </button>
                    </li>
                  ))
                )}
              </ul>
            )}
          </div>

          <div className={s.legend} aria-label="What the colours mean">
            <Tabs
              aria-label="Colour by"
              value={desk.layer}
              onValueChange={(v) => desk.setLayer(v as Layer)}
              items={LAYERS.map((l) => ({ value: l.id, label: l.label }))}
            />
            <ul className={s.keys}>
              {LEGEND[desk.layer].map((k) => (
                <li key={k.tone}>
                  <span className={s.swatch} style={swatch(k.tone)} aria-hidden />
                  {k.label}
                </li>
              ))}
            </ul>
            {desk.layer === "stock" && <p className={s.source}>{countSentence(site?.bins.reported_as_at ?? null)}</p>}
            <div className={s.walk}>
              <Checkbox checked={desk.showWalk} onCheckedChange={(v) => desk.setShowWalk(v === true)} label="Today's walk" />
              {desk.showWalk && <span className={s.source}>{walkSentence(desk.walk)}</span>}
            </div>
          </div>
        </section>

        <aside className={s.side}>
          <Chosen desk={desk} />
        </aside>
      </div>

      {site && site.bins.unplaced > 0 && (
        <p className={s.unplaced}>
          {site.bins.unplaced === 1 ? "1 bin isn't" : `${site.bins.unplaced.toLocaleString()} bins aren't`} on the layout, so the map
          can't show {site.bins.unplaced === 1 ? "it" : "them"}. <Link href={href("/warehouse?place=unplaced")}>See which</Link>
        </p>
      )}
    </Page>
  );
}

/** The chosen bin's card, or what to do when none is. */
function Chosen({ desk }: { desk: MapDesk }) {
  const bin = desk.chosen;
  const site = desk.read.kind === "ready" ? desk.read.value : null;
  if (!bin) {
    const bins = site?.bins.bins ?? [];
    const stocked = bins.filter((b) => b.reported_items > 0 || b.held > 0).length;
    return (
      <div className={s.card}>
        <h2 className={s.cardTitle}>Find a bin</h2>
        <p className={s.muted}>Search for one, or click one on the map. Drag to turn the view, and scroll to zoom.</p>
        {site && (
          <dl className={s.facts}>
            <dt>On the map</dt>
            <dd>{bins.length.toLocaleString()} bins</dd>
            <dt>With stock</dt>
            <dd>{stocked.toLocaleString()}</dd>
          </dl>
        )}
      </div>
    );
  }

  const place = desk.place;
  const detail = desk.detail.kind === "ready" ? desk.detail.value : null;
  const levels = place?.levels ?? 0;
  const reach = [
    { value: "0", label: "None of it" },
    ...Array.from({ length: levels }, (_, i) => ({
      value: String(i + 1),
      label: i === 0 ? "Level 1" : i + 1 === levels ? "All of it" : `Levels 1 to ${i + 1}`,
    })),
  ];

  return (
    <div className={s.card}>
      <div className={s.cardHead}>
        <h2 className={s.cardCode}>{bin.code}</h2>
        <Badge tone={bin.within_reach ? "success" : "warning"}>{bin.within_reach ? "From the floor" : "Ladder or forklift"}</Badge>
      </div>
      <p className={s.muted}>{detail?.place && detail.cell ? sayWhere(detail.place, detail.cell) : place?.name}</p>

      <h3 className={s.cardSection}>On this shelf</h3>
      {desk.detail.kind === "loading" ? (
        <Skeleton width="70%" />
      ) : desk.detail.kind === "failed" ? (
        <p className={s.muted}>{desk.detail.message}</p>
      ) : detail && detail.contents.length > 0 ? (
        <Contents bin={detail} />
      ) : (
        <p className={s.muted}>Nothing on this shelf.</p>
      )}
      <p className={s.source}>{countSentence(site?.bins.reported_as_at ?? null)}</p>

      {place && (
        <div className={s.reach}>
          <Select
            label={`Reached from the floor on ${place.name}`}
            value={String(place.reach_levels)}
            options={reach}
            disabled={desk.busy}
            onValueChange={(v) => void desk.setReach(Number(v))}
          />
        </div>
      )}
      {desk.problem && (
        <Alert tone="danger" onDismiss={desk.dismiss}>
          {desk.problem}
        </Alert>
      )}

      <Link href={href(`/bins/${bin.location_id}`)} className={s.open}>
        Open the rack face <ArrowRight aria-hidden />
      </Link>
    </div>
  );
}

/**
 * What is on the chosen bin, an item to a row as the packing bench draws
 * them: its photo, its code, which opens its properties beside the map, and
 * what it is; then NetSuite's count of it, and Spork's when it holds some.
 */
function Contents({ bin }: { bin: BinView }) {
  const [looking, setLooking] = useState<Uuid | null>(null);
  const items = bin.contents;
  const at = items.findIndex((i) => i.item_id === looking);
  const step = (by: number) => {
    const to = items[at + by];
    return to ? () => setLooking(to.item_id) : null;
  };
  const more = bin.contents_total - items.length;
  return (
    <>
      <ul className={s.items}>
        {items.map((i) => (
          <li key={i.item_id}>
            <ItemLine code={i.item_code} description={i.description} picture={i.picture} onOpen={() => setLooking(i.item_id)} />
            <span className={s.counts}>
              <span className={s.qty}>{i.on_hand === null ? "—" : Number(i.on_hand).toLocaleString()}</span>
              {i.held > 0 && <span className={s.held}>Spork {i.held.toLocaleString()}</span>}
            </span>
          </li>
        ))}
        {more > 0 && <li className={s.muted}>and {more.toLocaleString()} more</li>}
      </ul>
      <ItemDrawer itemId={looking} onClose={() => setLooking(null)} previous={step(-1)} next={step(1)} />
    </>
  );
}

/** What today's walk is, in a few words (D211). */
function walkSentence(walk: MapDesk["walk"]): string {
  if (walk.kind === "loading" || walk.kind === "idle") return "Reading it…";
  if (walk.kind === "failed") return walk.message;
  const route: WalkRoute | null = walk.value;
  if (!route) return "None of it is on the layout, or there's nothing to pick.";
  const stops = `${route.stops} ${route.stops === 1 ? "stop" : "stops"}`;
  const off = route.off_route > 0 ? `, and ${route.off_route} off the layout` : "";
  if (!route.cell_mm) return `${stops}${off}.`;
  return `${stops}, ${Math.round((route.walked * route.cell_mm) / 1000)} m${off}.`;
}

/**
 * Where the colours come from, and how old they are (D212): a number from
 * another system says how old it is, or it is read as current forever.
 */
function countSentence(asAt: string | null): string {
  if (!asAt) return "From Spork's own records: NetSuite's count hasn't been loaded.";
  return `From NetSuite's count ${ago(asAt)} ago, and Spork's own records.`;
}

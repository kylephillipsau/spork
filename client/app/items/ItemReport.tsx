import { useCallback, useEffect, useState, type ReactNode } from "react";

import { Alert, Button, Spinner, applyTheme } from "@ui/index";
import { imageUrl } from "@domain/api";
import type { BoundBarcode, CaptureSubject, ExportLevel, ItemView, PackagingType } from "@domain/types";
import { Faint, shortDate } from "@app/common/cells";

import { faceName, facesToAsk } from "./box";
import {
  NETSUITE_ROLES,
  ROLE_ORDER,
  artNo,
  differs,
  packedInWords,
  provenance,
  recorded,
  shipsWords,
  sizeOf,
  soldAs,
  wayUpWords,
  weightOf,
} from "./ItemProperties";
import { holdsOf, isOwnCarton, isOwnPack, nameOf, packHolds, photosOf, shown, type Face } from "./subjects";
import type { ReportItem, ReportRead } from "./useItemReport";
import s from "./report.module.css";

/**
 * Everything recorded of a list's items, to print or keep as a PDF (D242): an
 * item to a page, each of its cards with every figure, what it is packed in,
 * how it ships, who recorded it and when, and every photograph; what NetSuite
 * says of it; its barcodes. Said in the item page's own words, by its own
 * code, so the two never disagree.
 *
 * **Pictures at the size they print.** Each is drawn onto a canvas a few
 * hundred pixels across before it is printed, so a list of twenty items is a
 * PDF of a few megabytes, not of every photograph at full size. Printing
 * waits until they are all drawn.
 */
export function ItemReport({ read }: { read: ReportRead }) {
  const [drawn, setDrawn] = useState(0);
  const done = useCallback(() => setDrawn((n) => n + 1), []);
  // Paper is white: printed in the light theme, whatever the screen is in.
  useEffect(() => {
    applyTheme("light");
    return () => applyTheme();
  }, []);
  if (read.kind === "failed") {
    return (
      <main className={s.screen}>
        <Alert tone="danger">{read.message}</Alert>
      </main>
    );
  }
  if (read.kind === "loading") {
    return (
      <main className={s.screen}>
        <p className={s.status}>
          <Spinner size={16} label="Reading" /> {read.of ? `Reading ${read.done} of ${read.of} items…` : "Reading the list…"}
        </p>
      </main>
    );
  }
  const pictures = read.items.reduce((n, r) => n + picturesOf(r).length, 0);
  const ready = drawn >= pictures;
  return (
    <main className={s.report}>
      <div className={s.toolbar}>
        <div>
          <h1 className={s.title}>{read.title}</h1>
          <Faint>
            {read.items.length} {read.items.length === 1 ? "item" : "items"} · as recorded at {read.at.toLocaleString()}
          </Faint>
        </div>
        <Button variant="primary" disabled={!ready} loading={!ready} onClick={() => window.print()}>
          {ready ? "Print or save as PDF" : `Preparing pictures, ${drawn} of ${pictures}`}
        </Button>
      </div>
      {read.items.map((r) => (
        <ItemPage key={r.item.item_id} r={r} types={read.types} title={read.title} done={done} />
      ))}
    </main>
  );
}

/** The cards worth printing: everything with something on it, and what it is sold as even with nothing. */
function cardsOf(item: ItemView): CaptureSubject[] {
  return item.subjects.filter((x) => recorded(x) || x.is_unit || (x.wrap !== null && x.wrap !== undefined));
}

/** A picture on the page: what it is of, by its content address. */
interface Shown {
  key: string;
  digest: string;
  label: string;
  wide?: boolean;
}

/** An item's pictures as the page prints them: its main one, then each card's sides and its wrapping. */
function picturesOf(r: ReportItem): Shown[] {
  const { item } = r;
  const main = item.box_picture?.digest ?? item.main_picture ?? item.picture?.digest ?? null;
  return [...(main ? [{ key: "main", digest: main, label: "Main picture" }] : []), ...cardsOf(item).flatMap((c) => cardPictures(item, c))];
}

/** A card's pictures: each side photographed, cut where it is, in the order they are taken; then its wrapping. */
function cardPictures(item: ItemView, card: CaptureSubject): Shown[] {
  const photos = photosOf(item, card);
  const faces = [...facesToAsk(card, true), "detail" as Face].filter((f, i, all) => all.indexOf(f) === i);
  const sides = faces.flatMap((face) => {
    const photo = photos.get(face);
    if (!photo) return [];
    const like = photo.same_as ? `, same as ${photo.same_as}` : "";
    return [{ key: `${card.packaging_level}:${face}`, digest: shown(photo), label: `${faceName(face, card)}${like}${photo.cut ? "" : ", as taken"}` }];
  });
  const w = card.wrap;
  const wrapped: Shown[] = w
    ? [
        { key: `${card.packaging_level}:wrap`, digest: w.side, label: "Its side, unwrapped", wide: true },
        ...([
          ["lid", w.lid, "Lid"],
          ["inside", w.inside, "Inside, unwrapped"],
          ["floor", w.floor, "Floor"],
          ["base", w.base, "Base"],
        ] as const).flatMap(([part, digest, label]) => (digest ? [{ key: `${card.packaging_level}:${part}`, digest, label, wide: part === "inside" }] : [])),
      ]
    : [];
  return [...sides, ...wrapped];
}

function ItemPage({ r, types, title, done }: { r: ReportItem; types: PackagingType[]; title: string; done: () => void }) {
  const { item, row, barcodes } = r;
  const main = picturesOf(r).find((p) => p.key === "main");
  const art = artNo(item);
  return (
    <article className={s.item}>
      <p className={s.running}>{title}</p>
      <header className={s.head}>
        <div className={s.headText}>
          <h2 className={s.code}>{item.code}</h2>
          <p className={s.description}>{item.description}</p>
          <dl className={s.facts}>
            {art && <Fact label="Art No.">{art}</Fact>}
            <Fact label="Sold as">{soldAs(item)}</Fact>
            {item.style && <Fact label="Family">{item.style.code}</Fact>}
            <Fact label="Where">
              {row.netsuite_bins ?? <Faint>No bin in NetSuite</Faint>}
              {row.netsuite_on_hand && <Faint> · {row.netsuite_on_hand} on hand by NetSuite</Faint>}
            </Fact>
            {row.open_flags && <Fact label="Said on the floor">{row.open_flags}</Fact>}
          </dl>
        </div>
        {main && <Picture shown={main} done={done} size={360} />}
      </header>

      {cardsOf(item).map((card) => (
        <Card key={`${card.packaging_level}:${card.lot_id ?? card.item_part_id ?? card.item_style_id ?? ""}`} item={item} card={card} level={levelOf(r, card)} barcodes={barcodes} types={types} done={done} />
      ))}

      <NetSuite item={item} />
      {barcodes.length > 0 && (
        <section className={s.section}>
          <h3 className={s.sectionTitle}>Barcodes</h3>
          <table className={s.table}>
            <tbody>
              {barcodes.map((b) => (
                <tr key={b.id}>
                  <td className={s.mono}>{b.barcode}</td>
                  <td>{b.packaging_level === "inner" ? "pack" : b.packaging_level}</td>
                  <td>{b.scheme === "gtin" ? "GTIN" : "Spork's own"}</td>
                  <td>{b.quantity === null ? "weighed" : `${b.quantity} a scan`}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </article>
  );
}

/** What the export says of a card of the item's own: who recorded it, how it was arranged, whether it was put right. */
function levelOf(r: ReportItem, card: CaptureSubject): ExportLevel | null {
  if (card.item_id !== r.item.item_id || card.lot_id || card.item_part_id) return null;
  return card.packaging_level === "each" ? r.row.each : card.packaging_level === "inner" ? r.row.inner : card.packaging_level === "carton" ? r.row.carton : null;
}

function Card({
  item,
  card,
  level,
  barcodes,
  types,
  done,
}: {
  item: ItemView;
  card: CaptureSubject;
  level: ExportLevel | null;
  barcodes: BoundBarcode[];
  types: PackagingType[];
  done: () => void;
}) {
  const pictures = cardPictures(item, card);
  const own = card.item_id === item.item_id && !card.lot_id && !card.item_part_id && !card.item_style_id;
  const scanned = own ? barcodes.filter((b) => b.packaging_level === card.packaging_level) : [];
  const notes = [level?.corrected && "a figure was put right", level?.moved && "figures were moved here from another card"].filter(Boolean);
  const holds = isOwnCarton(card) ? holdsOf(item) : isOwnPack(card) ? (packHolds(item) ?? "Not said") : null;
  return (
    <section className={s.card}>
      <h3 className={s.cardTitle}>
        {nameOf(card, item)}
        {card.packaging_level && <span className={s.level}> · {card.packaging_level === "inner" ? "pack" : card.packaging_level}</span>}
        {card.is_unit && <span className={s.tag}>One in NetSuite</span>}
      </h3>
      <dl className={s.grid}>
        <Fact label="Weight">{weightOf(card)}</Fact>
        <Fact label={card.diameter_mm !== null ? "Size" : "Size (L × W × H)"}>{sizeOf(card)}</Fact>
        <Fact label="Packed in">{packedInWords(card, types)}</Fact>
        {holds && <Fact label="Holds">{holds}</Fact>}
        {card.packaging_level && <Fact label="Ships">{shipsWords(card)}</Fact>}
        {card.packaging_level && <Fact label="Way up">{wayUpWords(card)}</Fact>}
        {level?.arranged && <Fact label="Arranged">{level.arranged}</Fact>}
        <Fact label="Recorded">
          {provenance(card)}
          {level?.by && ` · by ${level.by}`}
        </Fact>
        {notes.length > 0 && <Fact label="Note">{notes.join("; ")}</Fact>}
        {scanned.length > 0 && <Fact label="Barcodes">{scanned.map((b) => b.barcode).join(", ")}</Fact>}
      </dl>
      {pictures.length > 0 && (
        <div className={s.pictures}>
          {pictures.map((p) => (
            <Picture key={p.key} shown={p} done={done} size={p.wide ? 1400 : 420} />
          ))}
        </div>
      )}
    </section>
  );
}

/** What NetSuite says of it, as the item page lists it (D238). */
function NetSuite({ item }: { item: ItemView }) {
  const ns = item.netsuite;
  if (!ns) return null;
  const fields = ns.fields
    .filter((f) => f.role !== "picture")
    .sort((a, b) => ROLE_ORDER.indexOf(a.role) - ROLE_ORDER.indexOf(b.role) || a.field.localeCompare(b.field));
  if (fields.length === 0) return null;
  const said = (role: string) => NETSUITE_ROLES.find((r) => r.role === role)?.label ?? role;
  return (
    <section className={s.section}>
      <h3 className={s.sectionTitle}>What NetSuite says</h3>
      <table className={s.table}>
        <tbody>
          {fields.map((f) => (
            <tr key={f.field}>
              <th scope="row">{f.field}</th>
              <td>
                {f.value}
                {differs(ns, f.role) && <strong className={s.differs}> · {differs(ns, f.role)}</strong>}
              </td>
              <td>
                <Faint>
                  {said(f.role)}
                  {f.level ? `, the ${f.level === "inner" ? "pack" : f.level}’s` : ""} · since {shortDate(f.since)}
                </Faint>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={s.fact}>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

/** How a picture is kept on the page: a JPEG, which a PDF holds as it is, a few tens of kilobytes. */
const QUALITY = 0.85;

/**
 * A picture drawn at the size it prints, `size` pixels on its longest side,
 * as a JPEG, with what it is under it. Tells the page once it is drawn, or
 * could not be. A canvas would be kept in the PDF without loss, ten times the
 * size.
 */
function Picture({ shown: p, size, done }: { shown: Shown; size: number; done: () => void }) {
  const [src, setSrc] = useState<string | null>(null);
  const [missing, setMissing] = useState(false);
  useEffect(() => {
    let live = true;
    const image = new Image();
    image.src = imageUrl(p.digest);
    image
      .decode()
      .then(() => {
        if (!live) return;
        const scale = Math.min(1, size / Math.max(image.naturalWidth, image.naturalHeight));
        const c = document.createElement("canvas");
        c.width = Math.round(image.naturalWidth * scale);
        c.height = Math.round(image.naturalHeight * scale);
        const g = c.getContext("2d")!;
        // A JPEG has no transparency: a disc's corners are the page.
        g.fillStyle = getComputedStyle(document.body).backgroundColor;
        g.fillRect(0, 0, c.width, c.height);
        g.imageSmoothingQuality = "high";
        g.drawImage(image, 0, 0, c.width, c.height);
        setSrc(c.toDataURL("image/jpeg", QUALITY));
        c.width = c.height = 0;
      })
      .catch(() => live && setMissing(true))
      .finally(() => live && done());
    return () => {
      live = false;
    };
  }, [p.digest, size, done]);
  return (
    <figure className={p.wide ? s.wide : p.key === "main" ? s.main : s.picture}>
      {missing ? <div className={s.missing}>Not on file</div> : src && <img src={src} alt={p.label} />}
      <figcaption>{p.label}</figcaption>
    </figure>
  );
}

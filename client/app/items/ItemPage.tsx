import { useState } from "react";
import { Camera, ImageOff, MapPin, Ruler } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  DataTable,
  EmptyState,
  Fact,
  Facts,
  Link,
  Page,
  PageHeader,
  Section,
  Skeleton,
  Stack,
  type Column,
} from "@ui/index";
import { imageUrl } from "@domain/api";
import type { ItemHeld, ItemMeasurements, ItemPacking, ItemReported, ItemView } from "@domain/types";
import { useNavigate } from "@app/routing/Router";
import { Faint, ago, dateTime, sentence } from "@app/common/cells";
import { centimetres, kg } from "@app/common/format";

import type { ItemDesk } from "./useItem";
import s from "./items.module.css";

/**
 * An item's own page: what it is, what it looks like, what a carton of it
 * holds and measures, and where it is.
 *
 * **Two answers to "where", never merged.** Spork's own record is its ledger;
 * NetSuite's is a report somebody exported, with an age. Drawn as one list,
 * the report would read as something Spork counted (migration 86).
 */
export function ItemPage({ desk }: { desk: ItemDesk }) {
  const navigate = useNavigate();
  const read = desk.read;

  if (read.kind !== "ready") {
    return (
      <Page>
        <PageHeader title="Item" />
        {read.kind === "failed" ? (
          <Alert tone="danger">{read.message}</Alert>
        ) : (
          <Card>
            <Stack gap={3}>
              <Skeleton width="40%" />
              <Skeleton width="70%" />
              <Skeleton width="55%" />
            </Stack>
          </Card>
        )}
      </Page>
    );
  }

  const item = read.item;
  const capture = (
    <Button icon={<Camera />} onClick={() => navigate("/capture")}>
      Measure and photograph
    </Button>
  );

  return (
    <Page>
      <PageHeader
        title={
          <span className={s.title}>
            <span className={s.code}>{item.code}</span>
            {!item.active && <Badge tone="warning">Inactive</Badge>}
          </span>
        }
        description={item.description}
        actions={capture}
      />

      <Stack gap={5}>
        <Card>
          <div className={s.summary}>
            <Photo key={item.item_id} item={item} />
            <Facts columns={1}>
              <Fact label="Family" always>
                {item.style ? (
                  <>
                    <span className={s.code}>{item.style.code}</span>
                    <Faint> · {item.style.variants === 1 ? "1 code" : `${item.style.variants} codes`}</Faint>
                  </>
                ) : (
                  <Faint>Not part of a family</Faint>
                )}
              </Fact>
              {item.packing && <Fact label="A carton holds">{holds(item.packing)}</Fact>}
            </Facts>
          </div>
        </Card>

        <Section title="Where it is">
          <Where item={item} />
        </Section>

        <Section title="Measurements" count={item.measurements.length}>
          {item.measurements.length > 0 ? (
            <Card padded={false}>
              <DataTable
                aria-label="Measurements"
                columns={MEASUREMENT_COLUMNS}
                rows={item.measurements}
                rowKey={(m) => m.packaging_level}
              />
            </Card>
          ) : (
            <Card>
              <EmptyState
                icon={<Ruler />}
                title="Not measured yet"
                description="Weigh, measure and photograph it on the capture screen."
                action={capture}
              />
            </Card>
          )}
        </Section>
      </Stack>
    </Page>
  );
}

/** The front, or a tile saying there is none. A family's photo says so (D141). */
function Photo({ item }: { item: ItemView }) {
  const [missing, setMissing] = useState(false);
  const picture = item.picture;
  if (!picture || missing) {
    return (
      <div className={s.photo} role="img" aria-label="No photo yet">
        <ImageOff aria-hidden />
        <span>No photo yet</span>
      </div>
    );
  }
  return (
    <figure className={s.figure}>
      <img className={s.photo} src={imageUrl(picture.digest)} alt={`${item.code}, front`} onError={() => setMissing(true)} />
      {picture.source !== "own" && (
        <figcaption className={s.caption}>{item.style ? `Photo of the ${item.style.code} family` : "Photo of its family"}</figcaption>
      )}
    </figure>
  );
}

/** Where it is, by each record, kept apart. */
function Where({ item }: { item: ItemView }) {
  if (item.held.length === 0 && item.reported.length === 0) {
    return (
      <Card>
        <EmptyState
          icon={<MapPin />}
          title="No record of where this is"
          description="Spork hasn't recorded any, and NetSuite's last inventory balance has none of it here."
        />
      </Card>
    );
  }

  const newest = item.reported.reduce<string | null>((n, r) => (n === null || r.as_at > n ? r.as_at : n), null);
  const sites = new Set([...item.held.map((h) => h.site_code), ...item.reported.map((r) => r.site_code)]);
  const many = sites.size > 1;

  return (
    <Stack gap={4}>
      {item.reported.length > 0 && (
        <Card
          title="NetSuite says"
          description={
            newest && (
              <>
                Its inventory balance as at {dateTime(newest)}, {ago(newest)} ago. A report from NetSuite, not a count
                Spork made.
              </>
            )
          }
          padded={false}
        >
          <DataTable
            aria-label="Where NetSuite says it is"
            columns={reportedColumns(many)}
            rows={item.reported}
            rowKey={(r) => `${r.source}:${r.site_code}:${r.location_id ?? "none"}`}
          />
        </Card>
      )}
      {item.held.length > 0 ? (
        <Card title="Spork's record" padded={false}>
          <DataTable
            aria-label="Where Spork holds it"
            columns={heldColumns(many)}
            rows={item.held}
            rowKey={(h) => `${h.site_code ?? ""}:${h.location_id ?? "none"}`}
          />
        </Card>
      ) : (
        <p className={s.note}>Spork hasn't recorded any of this item itself yet.</p>
      )}
    </Stack>
  );
}

function BinCell({ id, code }: { id: string | null; code: string | null }) {
  if (!id || !code) return <Faint>No bin</Faint>;
  return (
    <Link href={`/bins/${id}`} className={s.code}>
      {code}
    </Link>
  );
}

/**
 * **Not sortable.** The rows arrive in walking order, which is the order a
 * person on the floor wants, and a sort button in a header is a target too small
 * to hit with a gloved thumb.
 */
function reportedColumns(many: boolean): Column<ItemReported>[] {
  const cols: Column<ItemReported>[] = [
    {
      key: "bin",
      header: "Bin",
      cell: (r) => (
        <>
          <BinCell id={r.location_id} code={r.bin_code} />
          {r.status && r.status !== "Good" && <Faint> · {r.status}</Faint>}
        </>
      ),
      grow: true,
    },
    { key: "on_hand", header: "On hand", cell: (r) => r.on_hand, align: "right", mono: true, width: "100px" },
    { key: "available", header: "Available", cell: (r) => r.available ?? "—", align: "right", mono: true, width: "100px" },
  ];
  if (many) cols.unshift({ key: "site", header: "Warehouse", cell: (r) => r.site_code, width: "110px" });
  return cols;
}

function heldColumns(many: boolean): Column<ItemHeld>[] {
  const cols: Column<ItemHeld>[] = [
    { key: "bin", header: "Bin", cell: (h) => <BinCell id={h.location_id} code={h.bin_code} />, grow: true },
    { key: "quantity", header: "Quantity", cell: (h) => h.quantity, align: "right", mono: true, width: "100px" },
    { key: "allocated", header: "Allocated", cell: (h) => h.allocated_quantity, align: "right", mono: true, width: "100px" },
  ];
  if (many) cols.unshift({ key: "site", header: "Warehouse", cell: (h) => h.site_code ?? "—", width: "110px" });
  return cols;
}

/** What a carton holds, in words: "12", "10 boxes of 100 (1,000)", or that nobody said. */
function holds(p: ItemPacking): string {
  const n = p.inners_per_carton;
  const u = p.units_per_inner;
  if (n === null) return "Not recorded";
  if (u === 1) return n === 1 ? "1 unit" : `${n} units`;
  if (u === null) return `${n} inner packs, how many in each not recorded`;
  return `${n} inner packs of ${u} (${(n * u).toLocaleString()} units)`;
}

const METHOD: Record<string, string> = {
  instrument: "Measured",
  scan: "Scanned",
  keyed: "Typed in",
  derived: "Worked out",
  estimated: "Estimated",
  transcribed: "Copied from a list",
  asserted: "Stated",
  photographed: "From a photo",
};

function size(m: ItemMeasurements): string {
  const d = [m.length_mm, m.width_mm, m.height_mm];
  if (d.every((v) => v === null)) return "—";
  return `${d.map((v) => (v === null ? "?" : centimetres(v))).join(" × ")} cm`;
}

function whose(m: ItemMeasurements): string {
  if (m.source === "own") return "This code";
  const family = m.style_code ? `the ${m.style_code} family` : "its family";
  return m.source === "style" ? sentence(family) : `Partly ${family}`;
}

const MEASUREMENT_COLUMNS: Column<ItemMeasurements>[] = [
  { key: "level", header: "What", cell: (m) => sentence(m.packaging_level), width: "90px" },
  { key: "size", header: "Size (L × W × H)", cell: size, mono: true, width: "190px" },
  { key: "weight", header: "Weight", cell: (m) => kg(m.gross_weight_g), align: "right", mono: true, width: "110px" },
  { key: "how", header: "How", cell: (m) => (m.method ? METHOD[m.method] ?? sentence(m.method) : "—"), width: "150px" },
  { key: "whose", header: "Recorded against", cell: whose, grow: true },
];

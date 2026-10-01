import { MapPin } from "lucide-react";

import {
  Alert,
  Badge,
  Card,
  DataTable,
  EmptyState,
  Link,
  Page,
  PageHeader,
  Section,
  Skeleton,
  Stack,
  type Column,
} from "@ui/index";
import type { ItemHeld, ItemReported, ItemView } from "@domain/types";
import { Faint, ago, dateTime } from "@app/common/cells";

import { ItemProperties, ItemSummary } from "./ItemProperties";
import type { PropertiesDesk } from "./useItemProperties";
import s from "./items.module.css";

/**
 * An item's own page: what it is, what it looks like, what gets measured for
 * it and what is known of each, and where it is. Where a scanned item lands
 * (D111), so on a handheld this is where an item is weighed, measured and
 * photographed (D174).
 *
 * **Two answers to "where", never merged.** Spork's own record is its ledger;
 * NetSuite's is a report somebody exported, with an age. Drawn as one list,
 * the report would read as something Spork counted (migration 86).
 */
export function ItemPage({ desk }: { desk: PropertiesDesk }) {
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
      />

      <Stack gap={5}>
        <Card>
          <ItemSummary item={item} />
        </Card>

        <Section title="Size, weight and photos">
          <ItemProperties item={item} desk={desk} />
        </Section>

        <Section title="Where it is">
          <Where item={item} />
        </Section>
      </Stack>
    </Page>
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

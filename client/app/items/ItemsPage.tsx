import { Boxes, Camera, Ruler, Scale } from "lucide-react";
import type { ReactNode } from "react";

import {
  Alert,
  Button,
  Card,
  DataTable,
  EmptyState,
  Page,
  PageHeader,
  SearchField,
  Select,
  Spacer,
  Tabs,
  Toolbar,
  cx,
  type Column,
} from "@ui/index";
import type { ItemRow } from "@domain/types";
import { Thumb } from "@app/common/Thumb";
import { Faint } from "@app/common/cells";

import { ItemDrawer } from "./ItemProperties";
import type { PropertiesDesk } from "./useItemProperties";
import type { ItemsDesk, Needs, Order, Stock } from "./useItems";
import s from "./items.module.css";

/**
 * Every item, and what is known about each: what it looks like, how many are
 * here by each record, and whether its weight, size and photo are recorded.
 *
 * **Where an item's properties are found and changed** (D174). Narrowed to
 * what needs weighing, measuring or a photo, and ordered by how often it is
 * ordered or by the walk, it is the worklist the Weigh and Capture screens
 * were. A row opens the item beside the list, with Previous and Next to work
 * down it. `panel` is for fixtures, which draw the drawer with no network.
 */
export function ItemsPage({ desk, panel }: { desk: ItemsDesk; panel?: PropertiesDesk | undefined }) {
  const st = desk.state;
  const ready = st.kind === "ready" ? st : null;
  const narrowed = desk.asked.q.trim() || desk.asked.stock || desk.asked.needs;
  const rows = ready?.items ?? [];
  const at = rows.findIndex((r) => r.item_id === desk.chosen);
  const step = (by: number) => {
    const row = at >= 0 ? rows[at + by] : undefined;
    return row ? () => desk.choose(row.item_id) : null;
  };

  return (
    <Page>
      <PageHeader title="Items" description="Everything NetSuite has sent: where it is, and its weight, size and photos." />

      <Card padded={false}>
        <Toolbar>
          <Tabs
            aria-label="Where"
            value={desk.asked.stock || "all"}
            onValueChange={(v) => desk.narrow({ stock: (v === "here" ? "here" : "") as Stock })}
            items={[
              { value: "all", label: "All items" },
              { value: "here", label: "In stock here" },
            ]}
          />
          <div className={s.needs}>
            <Select
              aria-label="Needs"
              size="sm"
              value={desk.asked.needs || "any"}
              onValueChange={(v) => desk.narrow({ needs: (v === "any" ? "" : v) as Needs })}
              options={[
                { value: "any", label: "Whatever’s recorded" },
                { value: "weighing", label: "Needs weighing" },
                { value: "measuring", label: "Needs measuring" },
                { value: "photo", label: "Needs a photo" },
              ]}
            />
          </div>
          <div className={s.order}>
            <Select
              aria-label="Order"
              size="sm"
              value={desk.asked.order || "code"}
              onValueChange={(v) => desk.narrow({ order: (v === "code" ? "" : v) as Order })}
              options={[
                { value: "code", label: "By code" },
                { value: "demand", label: "Most ordered first" },
                { value: "walk", label: "In walking order" },
              ]}
            />
          </div>
          <Spacer />
          <form
            className={s.search}
            onSubmit={(e) => {
              e.preventDefault();
              desk.search();
            }}
          >
            <SearchField
              aria-label="Search items"
              placeholder="Code, description or barcode"
              value={desk.typed}
              onChange={(e) => desk.type(e.target.value)}
            />
            <Button type="submit">Search</Button>
          </form>
        </Toolbar>

        {st.kind === "failed" ? (
          <div className={s.inset}>
            <Alert tone="danger">{st.message}</Alert>
          </div>
        ) : (
          <DataTable
            aria-label="Items"
            columns={COLUMNS}
            rows={rows}
            rowKey={(i) => i.item_id}
            selectedKey={desk.chosen ?? undefined}
            loading={st.kind === "loading"}
            onRowClick={(i) => desk.choose(i.item_id)}
            empty={
              <EmptyState
                icon={<Boxes />}
                title={narrowed ? "Nothing matches" : "No items yet"}
                description={narrowed ? "Try another search, or widen the list." : "Import the item master from NetSuite to fill this list."}
              />
            }
          />
        )}

        {ready && ready.items.length > 0 && (
          <div className={s.foot}>
            <Faint>
              {ready.items.length === ready.total
                ? `${ready.total.toLocaleString()} ${ready.total === 1 ? "item" : "items"}`
                : `${ready.items.length.toLocaleString()} of ${ready.total.toLocaleString()} items`}
            </Faint>
            {ready.next && (
              <Button size="sm" loading={ready.more} onClick={() => void desk.more()}>
                Show more
              </Button>
            )}
          </div>
        )}
      </Card>

      <ItemDrawer itemId={desk.chosen} onClose={() => desk.choose(null)} previous={step(-1)} next={step(1)} desk={panel} />
    </Page>
  );
}

/** One mark for what is recorded: measured here, copied from a list, or not yet. */
function Mark({ icon, what, state }: { icon: ReactNode; what: string; state: "measured" | "listed" | "none" }) {
  const said = state === "measured" ? `${what} recorded` : state === "listed" ? `${what} from a list only` : `No ${what.toLowerCase()} yet`;
  return (
    <span
      className={cx(s.mark, state === "measured" ? s.markDone : state === "listed" ? s.markListed : s.markMissing)}
      role="img"
      aria-label={said}
      title={said}
    >
      {icon}
    </span>
  );
}

/**
 * **Not sortable.** The server pages in the order asked for, so a sort here
 * would sort only the rows already shown, and read as the whole list sorted.
 */
const COLUMNS: Column<ItemRow>[] = [
  { key: "photo", header: "", cell: (i) => <Thumb picture={i.picture} alt={i.code} />, width: "64px" },
  {
    key: "item",
    header: "Item",
    cell: (i) => (
      <span className={s.named}>
        <span className={s.code}>{i.code}</span>
        {i.description !== i.code && <span className={s.described}>{i.description}</span>}
      </span>
    ),
    grow: true,
  },
  { key: "bin", header: "Most of it in", cell: (i) => (i.bin_code ? <span className={s.code}>{i.bin_code}</span> : <Faint>—</Faint>), width: "130px" },
  {
    key: "netsuite",
    header: "NetSuite says",
    cell: (i) =>
      i.reported_on_hand === null ? (
        <Faint>—</Faint>
      ) : (
        <span>
          {i.reported_on_hand}
          {i.reported_bins > 0 && <Faint> · {i.reported_bins === 1 ? "1 bin" : `${i.reported_bins} bins`}</Faint>}
        </span>
      ),
    align: "right",
    mono: true,
    width: "150px",
  },
  { key: "held", header: "Spork holds", cell: (i) => (i.held ? i.held : <Faint>—</Faint>), align: "right", mono: true, width: "110px" },
  {
    key: "recorded",
    header: "Recorded",
    label: "Weight, size and photo",
    cell: (i) => (
      <span className={s.recorded}>
        <Mark icon={<Scale />} what="Weight" state={i.weight} />
        <Mark icon={<Ruler />} what="Size" state={i.size} />
        <Mark icon={<Camera />} what="Photo" state={i.picture ? "measured" : "none"} />
      </span>
    ),
    width: "140px",
  },
];

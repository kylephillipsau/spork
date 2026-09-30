import { Boxes } from "lucide-react";

import {
  Alert,
  Badge,
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
  type Column,
} from "@ui/index";
import type { ItemRow } from "@domain/types";
import { useNavigate } from "@app/routing/Router";
import { Thumb } from "@app/common/Thumb";
import { Faint } from "@app/common/cells";

import type { ItemsDesk, Needs, Stock } from "./useItems";
import s from "./items.module.css";

/**
 * Every item, and what is known about each: what it looks like, how many are
 * here by each record, and whether anybody has measured it. Narrowed to what
 * is here and still needs measuring or a photo, it is the list the capture
 * work is done from. A row opens the item.
 */
export function ItemsPage({ desk }: { desk: ItemsDesk }) {
  const navigate = useNavigate();
  const st = desk.state;
  const ready = st.kind === "ready" ? st : null;
  const narrowed = desk.asked.q.trim() || desk.asked.stock || desk.asked.needs;

  return (
    <Page>
      <PageHeader title="Items" description="Everything NetSuite has sent: where it is, and what has been recorded about it." />

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
                { value: "measuring", label: "Needs measuring" },
                { value: "photo", label: "Needs a photo" },
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
            rows={ready?.items ?? []}
            rowKey={(i) => i.item_id}
            loading={st.kind === "loading"}
            onRowClick={(i) => navigate(`/items/${i.item_id}`)}
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
    </Page>
  );
}

/** What has been recorded about its size and weight, in words. */
function Figures({ item }: { item: ItemRow }) {
  if (item.figures === "measured") return <Badge tone="success">Measured</Badge>;
  if (item.figures === "listed") return <Badge>From a list</Badge>;
  return <Faint>Not measured</Faint>;
}

/**
 * **Not sortable.** The server pages in code order, so a sort here would sort
 * only the rows already shown, and read as the whole list sorted.
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
  { key: "family", header: "Family", cell: (i) => (i.style_code ? <span className={s.code}>{i.style_code}</span> : <Faint>—</Faint>), width: "130px" },
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
  { key: "figures", header: "Size and weight", cell: (i) => <Figures item={i} />, width: "140px" },
];

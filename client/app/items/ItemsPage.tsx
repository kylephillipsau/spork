import {
  Boxes,
  Camera,
  ClipboardList,
  Download,
  FileSpreadsheet,
  FileText,
  ListPlus,
  MoreHorizontal,
  Pencil,
  Ruler,
  Scale,
  Trash2,
  X,
} from "lucide-react";
import { useState, type ReactNode } from "react";

import {
  Alert,
  Button,
  Card,
  DataTable,
  Dialog,
  EmptyState,
  IconButton,
  Menu,
  MenuItem,
  Page,
  PageHeader,
  SearchField,
  Select,
  Spacer,
  Stack,
  Tabs,
  TextArea,
  TextField,
  Toolbar,
  cx,
  type Column,
} from "@ui/index";
import type { ItemListRow, ItemRow } from "@domain/types";
import { Thumb } from "@app/common/Thumb";
import { Faint, dateTime } from "@app/common/cells";

import { ItemDrawer } from "./ItemProperties";
import type { PropertiesDesk } from "./useItemProperties";
import type { Has, ItemsDesk, Needs, Order, Stock } from "./useItems";
import s from "./items.module.css";

/**
 * The list as asked, every row of it, as a file (D216), or as the capture
 * sheet it replaces with what is recorded in its boxes (D217): what is shown,
 * whatever has been loaded of it, and every item when nothing narrows it.
 * Followed as a link, so the browser saves it as it arrives. Not in the app,
 * whose session is not a cookie a link carries.
 */
function Export({ desk, narrowed }: { desk: ItemsDesk; narrowed: boolean }) {
  const get = (format: "csv" | "xlsx" | "pdf") => () => window.location.assign(desk.exportUrl(format));
  return (
    <Menu
      trigger={
        <Button size="sm" icon={<Download />}>
          {narrowed ? "Export these" : "Export all"}
        </Button>
      }
    >
      <MenuItem icon={<FileSpreadsheet />} onSelect={get("xlsx")}>
        Excel, with pictures
      </MenuItem>
      <MenuItem icon={<FileText />} onSelect={get("csv")}>
        CSV
      </MenuItem>
      <MenuItem icon={<ClipboardList />} onSelect={get("pdf")}>
        Capture sheet (PDF)
      </MenuItem>
    </Menu>
  );
}

/**
 * Every item, and what is known about each: what it looks like, how many are
 * here by each record, and whether its weight, size and photo are recorded.
 *
 * **Where an item's properties are found and changed** (D174). Narrowed to
 * what needs weighing, measuring or a photo, and ordered by how often it is
 * ordered or by the walk, it is the worklist the Weigh and Capture screens
 * were. A row opens the item beside the list, with Previous and Next to work
 * down it. `panel` is for fixtures, which draw the drawer with no network.
 *
 * **A sheet somebody handed over is a list here** (D179): its codes pasted in
 * once at a desk, then picked on a phone and worked in the sheet's own order.
 * Chosen, it says how much of it is left, and is renamed, added to, taken
 * from or put away from its own bar (D235).
 */
export function ItemsPage({ desk, panel }: { desk: ItemsDesk; panel?: PropertiesDesk | undefined }) {
  const st = desk.state;
  const ready = st.kind === "ready" ? st : null;
  const narrowed = desk.asked.q.trim() || desk.asked.stock || desk.asked.needs || desk.asked.has || desk.asked.list;
  const [dialog, setDialog] = useState<ListDialogKind | "remove" | null>(null);
  const rows = ready?.items ?? [];
  const at = rows.findIndex((r) => r.item_id === desk.chosen);
  const step = (by: number) => {
    const row = at >= 0 ? rows[at + by] : undefined;
    return row ? () => desk.choose(row.item_id) : null;
  };

  return (
    <Page>
      <PageHeader
        title="Items"
        description="Everything NetSuite has sent: where it is, and its weight, size and photos."
        actions={import.meta.env.MODE !== "mobile" && <Export desk={desk} narrowed={!!narrowed} />}
      />

      <Card padded={false}>
        <Toolbar>
          <div className={s.lists}>
            <Select
              aria-label="List"
              size="sm"
              value={desk.asked.list || "none"}
              onValueChange={(v) => desk.pick(v === "none" ? "" : v)}
              options={[
                { value: "none", label: "Every item" },
                ...(desk.lists ?? []).map((l) => ({ value: l.item_list_id, label: `${l.name} · ${progress(l)}` })),
              ]}
            />
          </div>
          <Button size="sm" icon={<ListPlus />} onClick={() => setDialog("new")}>
            New list
          </Button>
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
            {/* What still needs doing, or what has been done to look it over:
                one question at a time, so choosing one clears the other. */}
            <Select
              aria-label="Recorded"
              size="sm"
              value={desk.asked.needs ? `needs:${desk.asked.needs}` : desk.asked.has ? `has:${desk.asked.has}` : "any"}
              onValueChange={(v) => {
                const [side, what = ""] = v.split(":");
                desk.narrow({
                  needs: (side === "needs" ? what : "") as Needs,
                  has: (side === "has" ? what : "") as Has,
                  // The packing worklist reads best most needed first (D197).
                  ...(what === "packing" && !desk.asked.order ? { order: "packing" as Order } : {}),
                });
              }}
              options={[
                { value: "any", label: "Whatever’s recorded" },
                { value: "has:measured", label: "Measured" },
                { value: "has:photographed", label: "Photographed" },
                { value: "has:both", label: "Measured and photographed" },
                { value: "needs:weighing", label: "Needs weighing" },
                { value: "needs:measuring", label: "Needs measuring" },
                { value: "needs:photo", label: "Needs a photo" },
                { value: "needs:packing", label: "Needs a size for packing" },
                { value: "needs:netsuite", label: "Differs from NetSuite" },
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
                ...(desk.asked.list ? [{ value: "list", label: "As on the list" }] : []),
                { value: "code", label: "By code" },
                { value: "demand", label: "Most ordered first" },
                { value: "packing", label: "Most needed for packing first" },
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

        {desk.list && <ListBar list={desk.list} desk={desk} onDo={setDialog} />}

        {st.kind === "failed" ? (
          <div className={s.inset}>
            <Alert tone="danger">{st.message}</Alert>
          </div>
        ) : (
          <DataTable
            aria-label="Items"
            columns={desk.asked.list ? listed(desk) : COLUMNS}
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
      {dialog === "remove" && desk.list && <RemoveList list={desk.list} desk={desk} onClose={() => setDialog(null)} />}
      {dialog && dialog !== "remove" && <ListDialog kind={dialog} desk={desk} onClose={() => setDialog(null)} />}
    </Page>
  );
}

/** How much of a list is left: "9 of 21 left", or that it is done (D235). */
function progress(l: Pick<ItemListRow, "items" | "done">): string {
  if (l.items > 0 && l.done >= l.items) return "done";
  return `${(l.items - l.done).toLocaleString()} of ${l.items.toLocaleString()} left`;
}

/**
 * The list narrowed to (D235): its name, how much of it is left and whose it
 * is, and what can be done to it. Taking one item off is on its row.
 */
function ListBar({ list, desk, onDo }: { list: ItemListRow; desk: ItemsDesk; onDo: (what: ListDialogKind | "remove") => void }) {
  return (
    <div className={s.listBar}>
      <span className={s.listName}>{list.name}</span>
      <Faint>
        {progress(list)} · made {list.recorded_by_name ? `by ${list.recorded_by_name} ` : ""}
        {dateTime(list.recorded_at)}
      </Faint>
      <Spacer />
      {desk.making.problem && !desk.making.busy && (
        <Alert tone="danger" onDismiss={desk.making.dismiss}>
          {desk.making.problem}
        </Alert>
      )}
      <Menu trigger={<IconButton label="Change the list" icon={<MoreHorizontal />} noTooltip />}>
        <MenuItem icon={<Pencil />} onSelect={() => onDo("rename")}>
          Rename…
        </MenuItem>
        <MenuItem icon={<ListPlus />} onSelect={() => onDo("add")}>
          Add items…
        </MenuItem>
        <MenuItem icon={<Trash2 />} danger onSelect={() => onDo("remove")}>
          Delete the list…
        </MenuItem>
      </Menu>
    </div>
  );
}

type ListDialogKind = "new" | "rename" | "add";

/**
 * A list made from a sheet: its name and its codes, pasted one a line (a
 * spreadsheet's column pastes that way), in the sheet's order. A code nobody
 * knows refuses the list and is named, so no row goes missing quietly.
 *
 * The same form renames the list narrowed to, or adds a sheet's further codes
 * to its end (D235): the name alone, or the codes alone.
 */
export function ListDialog({ kind, desk, onClose }: { kind: ListDialogKind; desk: ItemsDesk; onClose: () => void }) {
  const [name, setName] = useState(kind === "rename" ? (desk.list?.name ?? "") : "");
  const [pasted, setPasted] = useState("");
  const close = () => {
    desk.making.dismiss();
    onClose();
  };
  const save = async () => {
    const done =
      kind === "new" ? await desk.makeList(name, pasted) : kind === "rename" ? await desk.renameList(name) : await desk.addToList(pasted);
    if (done) onClose();
  };
  const said = {
    new: { title: "New list", description: "A sheet of items to work through, in its order.", button: "Make the list" },
    rename: { title: "Rename the list", description: "What the sheet is called.", button: "Rename" },
    add: {
      title: `Add to ${desk.list?.name ?? "the list"}`,
      description: "More of the sheet’s codes, after those on it. Those on it already stay where they are.",
      button: "Add to the list",
    },
  }[kind];
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && close()}
      width={480}
      title={said.title}
      description={said.description}
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" loading={desk.making.busy} onClick={() => void save()}>
            {said.button}
          </Button>
        </>
      }
    >
      <Stack gap={3}>
        {desk.making.problem && (
          <Alert tone="danger" onDismiss={desk.making.dismiss}>
            {desk.making.problem}
          </Alert>
        )}
        {kind !== "add" && (
          <TextField label="Name" autoComplete="off" autoFocus placeholder="Weights and sizes, 1 Oct" value={name} onChange={(e) => setName(e.target.value)} />
        )}
        {kind !== "rename" && (
          <TextArea
            label="Item codes"
            hint="One a line, in the order on the sheet."
            rows={10}
            spellCheck={false}
            autoCapitalize="characters"
            autoFocus={kind === "add"}
            value={pasted}
            onChange={(e) => setPasted(e.target.value)}
          />
        )}
      </Stack>
    </Dialog>
  );
}

/**
 * Delete the list narrowed to (D235): it goes from everyone's picker, and is
 * kept, with who put it away. Nothing recorded on its items changes.
 */
function RemoveList({ list, desk, onClose }: { list: ItemListRow; desk: ItemsDesk; onClose: () => void }) {
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      width={440}
      title={`Delete “${list.name}”?`}
      description="It goes from everyone’s list of lists. Its items, and everything weighed, measured and photographed of them, stay."
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="danger" loading={desk.making.busy} onClick={() => void desk.removeList().then((ok) => ok && onClose())}>
            Delete the list
          </Button>
        </>
      }
    >
      {desk.making.problem && (
        <Alert tone="danger" onDismiss={desk.making.dismiss}>
          {desk.making.problem}
        </Alert>
      )}
    </Dialog>
  );
}

/** "48 to pack on 3 orders": what open orders are waiting on (D197). */
function toPack(i: ItemRow): string {
  return `${i.to_pack.toLocaleString()} to pack on ${i.to_pack_lines === 1 ? "1 order" : `${i.to_pack_lines} orders`}`;
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
        {i.art_no && <span className={s.art}>Art {i.art_no}</span>}
        {i.to_pack > 0 && <Faint>{toPack(i)}</Faint>}
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

/**
 * Narrowed to a list: its place on the sheet first, to find the row on the
 * paper, and last, taking it off the list (D235).
 */
function listed(desk: ItemsDesk): Column<ItemRow>[] {
  return [
    { key: "place", header: "#", cell: (i) => i.list_position ?? <Faint>—</Faint>, align: "right", mono: true, width: "48px" },
    ...COLUMNS,
    {
      key: "off",
      header: "",
      label: "Take off the list",
      cell: (i) => (
        <IconButton
          size="sm"
          label="Take off the list"
          icon={<X />}
          disabled={desk.making.busy}
          onClick={(e) => {
            // The row opens the item; this only takes it off.
            e.stopPropagation();
            void desk.takeOffList(i.item_id);
          }}
        />
      ),
      width: "48px",
    },
  ];
}

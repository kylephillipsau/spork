import { useState } from "react";
import { MapPin, MapPinPlus } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  DataTable,
  Dialog,
  EmptyState,
  Link,
  Page,
  PageHeader,
  Section,
  Skeleton,
  Stack,
  TextField,
  type Column,
} from "@ui/index";
import type { BinFlagged, ItemFlag, ItemHeld, ItemReported, ItemView } from "@domain/types";
import { Faint, ago, dateTime } from "@app/common/cells";

import { ItemProperties, ItemSummary } from "./ItemProperties";
import type { BinFlag, PropertiesDesk } from "./useItemProperties";
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
 *
 * **What the floor says against NetSuite's bins** (D215): not in a bin it
 * lists, found in one it doesn't. Each is a finding, for someone to put right
 * in NetSuite; the page shows those still open.
 *
 * `asking` is for fixtures, which draw it with a question open.
 */
export function ItemPage({ desk, asking }: { desk: PropertiesDesk; asking?: Asking }) {
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
          <ItemSummary item={item} desk={desk} />
        </Card>

        <Section title="Size, weight and photos">
          <ItemProperties item={item} desk={desk} />
        </Section>

        <Section title="Where it is">
          <Where item={item} desk={desk} opened={asking ?? null} />
        </Section>
      </Stack>
    </Page>
  );
}

/** A question about a bin, open (D215): not in one NetSuite lists, or found in another. */
export type Asking = { said: "not_here"; row: ItemReported } | { said: "found_here" };

/** The question's title: a bin of NetSuite's, or another bin than those it lists. */
const titleOf = (asking: Asking, listed: boolean) =>
  asking.said === "not_here" ? `Not in ${asking.row.bin_code}?` : listed ? "Found it in another bin" : "Found it in a bin";

/** Where it is, by each record, kept apart; and what the floor says against NetSuite's bins. */
function Where({ item, desk, opened }: { item: ItemView; desk: PropertiesDesk; opened: Asking | null }) {
  const [asking, setAsking] = useState<Asking | null>(opened);
  const [flagged, setFlagged] = useState<BinFlagged | null>(null);
  const ask = (next: Asking) => {
    desk.dismiss();
    setFlagged(null);
    setAsking(next);
  };
  const flag = async (said: BinFlag) => {
    const made = await desk.flagBin(said);
    if (made) {
      setFlagged(made);
      setAsking(null);
    }
  };
  const notHere = new Map(
    item.flags.filter((f) => f.kind === "not_in_listed_bin" && f.location_id).map((f) => [f.location_id!, f]),
  );
  const foundIn = item.flags.filter((f) => f.kind === "found_in_unlisted_bin");
  const listed = item.reported.some((r) => r.location_id && Number(r.on_hand) > 0);
  const found = (
    <Button size="sm" icon={<MapPinPlus />} onClick={() => ask({ said: "found_here" })}>
      {listed ? "Found it in another bin" : "Found it in a bin"}
    </Button>
  );
  const said = flagged && (
    <Alert tone="success" onDismiss={() => setFlagged(null)}>
      {flagged.already
        ? `Already said of ${flagged.bin_code}, and still open on Findings.`
        : `Flagged ${flagged.bin_code}. It's on Findings for someone to put right in NetSuite.`}{" "}
      <Link href={`/findings/${flagged.discrepancy_id}`}>Open the finding</Link>
    </Alert>
  );
  const dialog = asking && (
    <FlagDialog
      asking={asking}
      title={titleOf(asking, listed)}
      desk={desk}
      onClose={() => setAsking(null)}
      onFlag={(f) => void flag(f)}
    />
  );

  if (item.held.length === 0 && item.reported.length === 0 && foundIn.length === 0) {
    return (
      <Stack gap={4}>
        {said}
        <Card>
          <EmptyState
            icon={<MapPin />}
            title="No record of where this is"
            description="Spork hasn't recorded any, and NetSuite's last inventory balance has none of it here."
            action={found}
          />
        </Card>
        {dialog}
      </Stack>
    );
  }

  const newest = item.reported.reduce<string | null>((n, r) => (n === null || r.as_at > n ? r.as_at : n), null);
  const sites = new Set([...item.held.map((h) => h.site_code), ...item.reported.map((r) => r.site_code)]);
  const many = sites.size > 1;

  const flagColumn: Column<ItemReported> = {
    key: "flag",
    header: "",
    cell: (r) => {
      if (!r.location_id || !(Number(r.on_hand) > 0)) return null;
      const f = notHere.get(r.location_id);
      if (f) {
        return (
          <Link href={`/findings/${f.discrepancy_id}`}>
            <Badge tone="warning">Said not here</Badge>
          </Link>
        );
      }
      return (
        <Button size="sm" onClick={() => ask({ said: "not_here", row: r })}>
          Not here
        </Button>
      );
    },
    align: "right",
    width: "140px",
  };

  return (
    <Stack gap={4}>
      {said}
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
            columns={[...reportedColumns(many), flagColumn]}
            rows={item.reported}
            rowKey={(r) => `${r.source}:${r.site_code}:${r.location_id ?? "none"}`}
          />
        </Card>
      )}
      {foundIn.length > 0 && (
        <Card
          title="Found where NetSuite doesn't list it"
          description="Said on the floor, and open on Findings until it is put right in NetSuite."
          padded={false}
        >
          <DataTable aria-label="Found where NetSuite doesn't list it" columns={FOUND_COLUMNS} rows={foundIn} rowKey={(f) => f.discrepancy_id} />
        </Card>
      )}
      <div>{found}</div>
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
      {dialog}
    </Stack>
  );
}

const FOUND_COLUMNS: Column<ItemFlag>[] = [
  { key: "bin", header: "Bin", cell: (f) => <BinCell id={f.location_id} code={f.bin_code} />, grow: true },
  { key: "found", header: "Found", cell: (f) => f.found ?? <Faint>Not counted</Faint>, align: "right", mono: true, width: "100px" },
  {
    key: "said",
    header: "Said",
    cell: (f) => (
      <Link href={`/findings/${f.discrepancy_id}`}>
        {f.detected_by ?? "Someone"}, {ago(f.detected_at)} ago
      </Link>
    ),
    width: "180px",
  },
];

/**
 * Not in a bin NetSuite lists, or found in one it doesn't (D215). Either is a
 * finding, so it asks before it says, and says what will happen.
 */
function FlagDialog({
  asking,
  title,
  desk,
  onClose,
  onFlag,
}: {
  asking: Asking;
  title: string;
  desk: PropertiesDesk;
  onClose: () => void;
  onFlag: (said: BinFlag) => void;
}) {
  const [bin, setBin] = useState("");
  const [count, setCount] = useState("");
  const [note, setNote] = useState("");
  const notHere = asking.said === "not_here";
  const typed = count.trim();
  const quantity = typed === "" ? null : /^\d+$/.test(typed) && Number(typed) >= 1 ? Number(typed) : Number.NaN;
  const ready = notHere || (bin.trim() !== "" && !Number.isNaN(quantity));
  const send = () => {
    if (!ready) return;
    const said: BinFlag =
      asking.said === "not_here"
        ? { said: "not_here", location_id: asking.row.location_id!, note: note.trim() || undefined }
        : { said: "found_here", bin_code: bin.trim(), quantity, note: note.trim() || undefined };
    onFlag(said);
  };
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && !desk.busy && onClose()}
      width={440}
      title={title}
      description={
        asking.said === "not_here"
          ? `NetSuite lists ${asking.row.on_hand} here. Saying none is here raises a finding for someone to put right in NetSuite.`
          : "The bin it's in, where NetSuite lists none. Saying so raises a finding for someone to put right in NetSuite."
      }
      footer={
        <>
          <Button onClick={onClose} disabled={desk.busy}>
            Cancel
          </Button>
          <Button variant="primary" loading={desk.busy} disabled={!ready} onClick={send}>
            {notHere ? "None here" : "Flag it"}
          </Button>
        </>
      }
    >
      <Stack gap={3}>
        {desk.problem && (
          <Alert tone="danger" onDismiss={desk.dismiss}>
            {desk.problem}
          </Alert>
        )}
        {!notHere && (
          <>
            <TextField
              label="Bin"
              autoFocus
              autoComplete="off"
              autoCapitalize="characters"
              spellCheck={false}
              placeholder="D-03-2"
              value={bin}
              onChange={(e) => setBin(e.target.value)}
            />
            <TextField
              label="How many there"
              hint="Blank if not counted"
              inputMode="numeric"
              autoComplete="off"
              trailing="× each"
              value={count}
              error={Number.isNaN(quantity) ? "A whole number, 1 or more" : undefined}
              onChange={(e) => setCount(e.target.value)}
            />
          </>
        )}
        <TextField
          label="Note"
          hint="Anything that helps put it right"
          autoComplete="off"
          value={note}
          onChange={(e) => setNote(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") send();
          }}
        />
      </Stack>
    </Dialog>
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

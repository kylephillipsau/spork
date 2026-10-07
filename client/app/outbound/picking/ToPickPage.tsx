import { useMemo, useState } from "react";
import { ClipboardList, Footprints, MessageSquareText, Printer } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  Checkbox,
  DataTable,
  Drawer,
  EmptyState,
  Page,
  PageHeader,
  Select,
  Spacer,
  Tabs,
  TextArea,
  Toolbar,
  type Column,
} from "@ui/index";
import { ago, Faint } from "@app/common/cells";
import { href } from "@app/routing/location";
import type { AskedOrder, AskedState, PickPlan } from "@domain/types";

import { PER_TRIP, PICKERS, type ToPickDesk } from "./useToPick";
import { WalkView } from "./WalkView";
import s from "./to-pick.module.css";

/** The most waiting orders the server shares out at once (`to_pick::MOST_PLANNED`). */
const MOST_PLANNED = 40;

const STATE: Record<AskedState, { label: string; tone: "accent" | "info" | "success" | "neutral" | "warning" }> = {
  waiting: { label: "Waiting", tone: "accent" },
  part_picked: { label: "Part picked", tone: "info" },
  picked: { label: "Picked", tone: "success" },
  packed: { label: "Packed", tone: "success" },
  shipped: { label: "Shipped", tone: "neutral" },
  closed: { label: "Closed", tone: "neutral" },
  elsewhere: { label: "Another site", tone: "neutral" },
  unknown: { label: "Not known", tone: "warning" },
};

type View = "all" | "waiting" | "done" | "unknown";
const VIEW_OF: Record<AskedState, Exclude<View, "all">> = {
  waiting: "waiting",
  part_picked: "waiting",
  picked: "done",
  packed: "done",
  shipped: "done",
  closed: "done",
  elsewhere: "unknown",
  unknown: "unknown",
};
const waiting = (o: AskedOrder) => VIEW_OF[o.state] === "waiting";

/** Each order's group and trip, by number, when the batch is shared out. */
function groupsOf(plan: PickPlan | null): Map<string, string> {
  const out = new Map<string, string>();
  plan?.pickers.forEach((trips, g) =>
    trips.forEach((trip, t) => {
      const name = plan.pickers.length > 1 ? `Group ${String.fromCharCode(65 + g)}` : "Trip";
      for (const o of trip.orders) out.set(o, plan.pickers.length > 1 ? `${name} · trip ${t + 1}` : `${name} ${t + 1}`);
    }),
  );
  return out;
}

const units = (o: AskedOrder) => o.lines.filter((l) => !l.kit).reduce((n, l) => n + l.to_pick, 0);
const bins = (o: AskedOrder) => [...new Set(o.lines.flatMap((l) => l.takes.map((t) => t.bin)))];

/**
 * To pick (D231, D230): paste the row of order numbers the tickets went out
 * on, see which are still waiting and where each is picked from, share them
 * out between the people picking, and print their tickets.
 */
export function ToPickPage({ desk }: { desk: ToPickDesk }) {
  const [view, setView] = useState<View>("all");
  const [open, setOpen] = useState<AskedOrder | null>(null);
  const batch = desk.state.kind === "ready" ? desk.state.batch : null;
  const orders = batch?.orders ?? [];
  const groups = useMemo(() => groupsOf(batch?.plan ?? null), [batch]);
  const counts = useMemo(() => {
    const c: Record<View, number> = { all: orders.length, waiting: 0, done: 0, unknown: 0 };
    for (const o of orders) c[VIEW_OF[o.state]] += 1;
    return c;
  }, [orders]);
  const rows = view === "all" ? orders : orders.filter((o) => VIEW_OF[o.state] === view);
  const print = (walk: boolean) => {
    const at = desk.printed(walk);
    if (at) window.open(href(at), "_blank");
  };

  const columns: Column<AskedOrder>[] = [
    { key: "order", header: "Order", cell: (o) => o.number ?? o.asked, sort: (o) => o.number ?? o.asked, mono: true, width: "110px" },
    {
      key: "state",
      header: "State",
      cell: (o) => <Badge tone={STATE[o.state].tone}>{STATE[o.state].label}</Badge>,
      sort: (o) => Object.keys(STATE).indexOf(o.state),
      width: "130px",
    },
    { key: "customer", header: "Customer", cell: (o) => o.customer ?? <Faint>—</Faint>, sort: (o) => o.customer, grow: true },
    {
      key: "bins",
      header: "Bins",
      cell: (o) => {
        const b = bins(o);
        return b.length ? `${b.slice(0, 3).join(", ")}${b.length > 3 ? ` +${b.length - 3}` : ""}` : <Faint>—</Faint>;
      },
      mono: true,
      width: "220px",
    },
    { key: "units", header: "To pick", cell: (o) => (waiting(o) ? units(o) : <Faint>—</Faint>), sort: units, align: "right", width: "80px" },
    { key: "group", header: "Group", cell: (o) => groups.get(o.number ?? "") ?? <Faint>—</Faint>, width: "130px" },
    {
      key: "notes",
      header: "Notes",
      cell: (o) => (o.picking_instructions || o.customer_notes ? <MessageSquareText size={16} aria-label="Has notes" /> : null),
      width: "64px",
    },
  ];

  const waitingCount = counts.waiting;
  if (desk.walking) return <WalkView desk={desk} walk={desk.walking} />;
  return (
    <Page>
      <PageHeader
        title="To pick"
        description="Paste a row of order numbers from the picking sheet: which are still waiting, where to pick them from, and their tickets."
      />

      <Card>
        <form
          className={s.paste}
          onSubmit={(e) => {
            e.preventDefault();
            desk.lookUp();
          }}
        >
          <TextArea
            label="Order numbers"
            rows={2}
            placeholder="Paste a row from the sheet, e.g. S268281 S268299 S268300"
            value={desk.pasted}
            onChange={(e) => desk.paste(e.target.value)}
          />
          <Toolbar>
            <Button type="submit" variant="primary" disabled={!desk.pasted.trim()}>
              Look up
            </Button>
            <Button onClick={desk.everything}>Everything open</Button>
            <Spacer />
            {batch && (
              <Faint>
                {batch.orders_as_at ? `NetSuite's open orders ${ago(batch.orders_as_at)} ago` : "NetSuite hasn't sent its open orders yet"}
                {batch.balance_as_at && ` · bins counted ${ago(batch.balance_as_at)} ago`}
              </Faint>
            )}
          </Toolbar>
        </form>
      </Card>

      {desk.state.kind === "failed" && <Alert tone="danger">{desk.state.message}</Alert>}

      {batch && waitingCount > 0 && (
        <Card
          title="Share out"
          description="For picking without the handheld: who walks which orders, nearest together."
          actions={
            <>
              <Button icon={<Printer size={16} />} onClick={() => print(false)}>
                Print tickets
              </Button>
              {batch.plan && (
                <Button variant="primary" icon={<Printer size={16} />} onClick={() => print(true)}>
                  Print with walk sheets
                </Button>
              )}
            </>
          }
        >
          <div className={s.settings}>
            <Select
              label="People picking"
              value={String(desk.pickers)}
              onValueChange={(v) => desk.setPickers(Number(v))}
              options={PICKERS.map((n) => ({ value: n, label: n }))}
            />
            <Select
              label="Most orders a trip"
              value={desk.perTrip === null ? "none" : String(desk.perTrip)}
              onValueChange={(v) => desk.setPerTrip(v === "none" ? null : Number(v))}
              options={PER_TRIP.map((n) => ({ value: n, label: n === "none" ? "No limit" : n }))}
            />
            <Checkbox
              label="Walk to a shelf once when several groups want it, and sort at the bench"
              checked={desk.gather}
              onCheckedChange={desk.setGather}
            />
          </div>
          {batch.plan ? (
            <ul className={s.groups}>
              {batch.plan.pickers.map((trips, g) =>
                trips.length === 0 ? null : (
                  <li key={g}>
                    <strong>{batch.plan!.pickers.length > 1 ? `Group ${String.fromCharCode(65 + g)}` : "One picker"}</strong>
                    {trips.map((t, i) => {
                      const label = batch.plan!.pickers.length > 1 ? `Group ${String.fromCharCode(65 + g)} · trip ${i + 1}` : `Trip ${i + 1}`;
                      return (
                        <span key={i} className={s.trip}>
                          Trip {i + 1}: <span className={s.mono}>{t.orders.join(", ")}</span>
                          <Faint>
                            {" "}
                            · {t.stops.length} {t.stops.length === 1 ? "stop" : "stops"}
                            {t.minutes !== null && ` · about ${Math.max(1, Math.round(t.minutes))} min`}
                          </Faint>{" "}
                          {t.stops.length > 0 && (
                            <Button size="sm" icon={<Footprints size={14} />} onClick={() => desk.walkTrip(label, t)}>
                              Walk
                            </Button>
                          )}
                        </span>
                      );
                    })}
                  </li>
                ),
              )}
            </ul>
          ) : (
            waitingCount > MOST_PLANNED && <Faint>Paste a batch of up to {MOST_PLANNED} orders to share it out.</Faint>
          )}
        </Card>
      )}

      <Card padded={false}>
        <Toolbar>
          <Tabs
            aria-label="Where they stand"
            value={view}
            onValueChange={(v) => setView(v as View)}
            items={[
              { value: "all", label: "All", count: counts.all },
              { value: "waiting", label: "Still to pick", count: counts.waiting },
              { value: "done", label: "Picked or gone", count: counts.done },
              { value: "unknown", label: "Not known here", count: counts.unknown },
            ]}
          />
        </Toolbar>
        <DataTable
          aria-label="Orders"
          columns={columns}
          rows={rows}
          rowKey={(o) => o.asked}
          loading={desk.state.kind === "loading"}
          onRowClick={setOpen}
          empty={
            <EmptyState
              icon={<ClipboardList />}
              title={desk.asked.length ? "None of those orders" : "Nothing open to pick"}
              description={desk.asked.length ? undefined : "NetSuite has no open orders with something left to pick here."}
            />
          }
        />
      </Card>

      <Drawer
        open={open !== null}
        onOpenChange={(o) => !o && setOpen(null)}
        title={open ? `${open.number ?? open.asked} · ${open.customer ?? "—"}` : ""}
        description={open ? STATE[open.state].label : undefined}
        width={560}
      >
        {open && <OrderDetail order={open} />}
      </Drawer>
    </Page>
  );
}

/** What a ticket says of one order: its notes, then each line and where it's picked from. */
function OrderDetail({ order }: { order: AskedOrder }) {
  return (
    <div className={s.detail}>
      {(order.picking_instructions || order.customer_notes) && (
        <div className={s.notes}>
          {order.picking_instructions && (
            <p>
              <b>Picking instructions</b>
              {order.picking_instructions}
            </p>
          )}
          {order.customer_notes && (
            <p>
              <b>Customer notes</b>
              {order.customer_notes}
            </p>
          )}
        </div>
      )}
      {order.ship_to && <p className={s.address}>{order.ship_to}</p>}
      {order.lines.length === 0 ? (
        <Faint>Nothing left to pick here.</Faint>
      ) : (
        <table className={s.lines}>
          <thead>
            <tr>
              <th>Bin</th>
              <th className={s.qty}>Qty</th>
              <th>Item</th>
            </tr>
          </thead>
          <tbody>
            {order.lines.map((l) =>
              l.kit ? (
                <tr key={l.line_key} className={s.kit}>
                  <td colSpan={3}>
                    {l.code} × {l.to_pick}, picked as its parts:
                  </td>
                </tr>
              ) : (
                [
                  ...l.takes.map((t) => (
                    <tr key={`${l.line_key}-${t.location_id}`}>
                      <td className={s.bin}>
                        {t.bin}
                        {!t.within_reach && <Badge tone="warning">High</Badge>}
                      </td>
                      <td className={s.qty}>{t.quantity}</td>
                      <td>
                        <span className={s.mono}>{l.code}</span> {l.description && <Faint>{l.description}</Faint>}
                      </td>
                    </tr>
                  )),
                  l.short > 0 && (
                    <tr key={`${l.line_key}-short`} className={s.short}>
                      <td>No stock in a bin</td>
                      <td className={s.qty}>{l.short}</td>
                      <td>
                        <span className={s.mono}>{l.code}</span> {l.description && <Faint>{l.description}</Faint>}
                      </td>
                    </tr>
                  ),
                ]
              ),
            )}
          </tbody>
        </table>
      )}
    </div>
  );
}

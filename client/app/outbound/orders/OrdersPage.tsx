import { useState, type ReactNode } from "react";
import { CircleAlert, ClipboardList, History, Package } from "lucide-react";

import {
  Badge,
  Button,
  Card,
  DataTable,
  Drawer,
  EmptyState,
  Link,
  PageHeader,
  SearchField,
  Stack,
  type Column,
} from "@ui/index";
import { href } from "@app/routing/location";
import { useNavigate } from "@app/routing/Router";
import { Faint, Progress, StateBadge, dateTime, shortDate } from "@app/common/cells";
import type { FulfilmentSummary, OrderMatch } from "@domain/types";

import type { OrdersDesk } from "./useOrders";
import s from "./orders.module.css";

/** Lines and quantities across an order's fulfilments. */
export function orderTotals(o: OrderMatch) {
  return o.fulfilments.reduce(
    (t, f) => ({
      lines: t.lines + f.line_count,
      committed: t.committed + f.committed_quantity,
      picked: t.picked + f.picked_quantity,
      packed: t.packed + f.packed_quantity,
      despatched: t.despatched + f.despatched_quantity,
    }),
    { lines: 0, committed: 0, picked: 0, packed: 0, despatched: 0 },
  );
}

/**
 * Orders (D150, D171): the latest at this site, or the ones a confirmation
 * number or reference names — one reference can name a cancelled order and
 * the one that replaced it. A row opens the order in a drawer.
 */
export function OrdersPage({ desk }: { desk: OrdersDesk }) {
  const [open, setOpen] = useState<OrderMatch | null>(null);
  const navigate = useNavigate();
  const st = desk.state;
  const busy = st.kind === "searching";
  const orders = st.kind === "found" || st.kind === "listed" ? st.orders : [];

  return (
    <div className={s.page}>
      <PageHeader title="Orders" description="Sales orders sent from NetSuite, and what has been committed against them." />

      <Card padded={false}>
        <form
          className={s.toolbar}
          onSubmit={(e) => {
            e.preventDefault();
            void desk.search();
          }}
        >
          <div className={s.search}>
            <SearchField
              aria-label="Confirmation number or reference"
              placeholder="Confirmation number or reference"
              value={desk.reference}
              onChange={(e) => desk.type(e.target.value)}
              disabled={busy}
            />
          </div>
          <Button type="submit" variant="primary" disabled={!desk.reference.trim() || busy}>
            Find
          </Button>
          {st.kind !== "listed" && st.kind !== "idle" && (
            <Button icon={<History />} disabled={busy} onClick={() => void desk.list()}>
              Latest
            </Button>
          )}
          <span className={s.caption}>
            {st.kind === "found" ? (
              <>
                Results for <code>{st.reference}</code>
              </>
            ) : st.kind === "listed" ? (
              "Latest orders"
            ) : null}
          </span>
        </form>

        {st.kind === "failed" ? (
          <p className={s.failed} role="alert">
            <CircleAlert aria-hidden /> {st.message}
          </p>
        ) : (
          <DataTable
            aria-label="Orders"
            columns={COLUMNS}
            rows={orders}
            rowKey={(o) => o.order_id}
            loading={busy || st.kind === "idle"}
            onRowClick={setOpen}
            selectedKey={open?.order_id}
            empty={
              st.kind === "found" ? (
                <EmptyState
                  icon={<ClipboardList />}
                  title="No matching order"
                  description={
                    <>
                      Nothing answers to <code>{st.reference}</code> as a confirmation number or external reference.
                    </>
                  }
                />
              ) : (
                <EmptyState icon={<ClipboardList />} title="No orders yet" description="Orders sent from NetSuite appear here." />
              )
            }
          />
        )}
      </Card>

      <Drawer
        open={open !== null}
        onOpenChange={(o) => {
          if (!o) setOpen(null);
        }}
        title={open ? (open.confirmation_number ?? open.external_ref ?? "Order") : ""}
        description={open?.customer_name ?? undefined}
        width={560}
        footer={
          open && open.fulfilments.length === 1 ? (
            <Button
              variant="primary"
              icon={<Package />}
              onClick={() => navigate(`/pack/${open.fulfilments[0]!.fulfilment_id}`)}
            >
              Open pack bench
            </Button>
          ) : undefined
        }
      >
        {open && <OrderDetail order={open} />}
      </Drawer>
    </div>
  );
}

function OrderDetail({ order }: { order: OrderMatch }) {
  const t = orderTotals(order);
  return (
    <Stack gap={5}>
      <dl className={s.facts}>
        <Fact label="Status">
          <StateBadge state={order.state} />
          {order.supersedes_order_id && <Badge>Replaces an earlier order</Badge>}
        </Fact>
        <Fact label="Confirmation">{order.confirmation_number ?? <Faint>—</Faint>}</Fact>
        <Fact label="External reference">{order.external_ref ?? <Faint>—</Faint>}</Fact>
        <Fact label="Placed">{order.placed_at ? dateTime(order.placed_at) : <Faint>—</Faint>}</Fact>
        <Fact label="Promised">{order.promised_to ? dateTime(order.promised_to) : <Faint>—</Faint>}</Fact>
        <Fact label="Picked">
          <span className={s.factProgress}>
            <Progress done={t.picked} of={t.committed} />
          </span>
        </Fact>
      </dl>

      <section className={s.section}>
        <h3 className={s.sectionTitle}>Fulfilments</h3>
        {order.fulfilments.length === 0 ? (
          <p className={s.none}>Nothing is committed against this order yet.</p>
        ) : (
          <div className={s.tableBox}>
            <DataTable
              aria-label="Fulfilments"
              columns={FULFILMENT_COLUMNS}
              rows={order.fulfilments}
              rowKey={(f) => f.fulfilment_id}
            />
          </div>
        )}
      </section>
    </Stack>
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

const COLUMNS: Column<OrderMatch>[] = [
  {
    key: "order",
    header: "Order",
    cell: (o) => <span className={s.ref}>{o.confirmation_number ?? o.external_ref ?? "—"}</span>,
    sort: (o) => o.confirmation_number ?? o.external_ref,
    mono: true,
    width: "130px",
  },
  { key: "customer", header: "Customer", cell: (o) => o.customer_name ?? <Faint>—</Faint>, sort: (o) => o.customer_name, grow: true },
  {
    key: "site",
    header: "Site",
    cell: (o) => [...new Set(o.fulfilments.map((f) => f.site_code).filter(Boolean))].join(", ") || <Faint>—</Faint>,
    width: "90px",
  },
  { key: "lines", header: "Lines", cell: (o) => orderTotals(o).lines, sort: (o) => orderTotals(o).lines, align: "right", width: "70px" },
  {
    key: "picked",
    header: "Picked",
    cell: (o) => {
      const t = orderTotals(o);
      return <Progress done={t.picked} of={t.committed} />;
    },
    sort: (o) => {
      const t = orderTotals(o);
      return t.committed ? t.picked / t.committed : 0;
    },
    width: "150px",
  },
  { key: "packed", header: "Packed", cell: (o) => orderTotals(o).packed, align: "right", width: "80px" },
  { key: "despatched", header: "Despatched", cell: (o) => orderTotals(o).despatched, align: "right", width: "100px" },
  { key: "state", header: "Status", cell: (o) => <StateBadge state={o.state} />, sort: (o) => o.state, width: "120px" },
  {
    key: "placed",
    header: "Placed",
    cell: (o) => (o.placed_at ? shortDate(o.placed_at) : <Faint>—</Faint>),
    sort: (o) => o.placed_at,
    align: "right",
    width: "90px",
  },
];

const FULFILMENT_COLUMNS: Column<FulfilmentSummary>[] = [
  {
    key: "site",
    header: "Site",
    cell: (f) => <Link href={href(`/pack/${f.fulfilment_id}`)}>{f.site_code ?? "—"}</Link>,
    width: "70px",
  },
  { key: "lines", header: "Lines", cell: (f) => f.line_count, align: "right", width: "60px" },
  { key: "picked", header: "Picked", cell: (f) => <Progress done={f.picked_quantity} of={f.committed_quantity} />, grow: true },
  { key: "packed", header: "Packed", cell: (f) => f.packed_quantity, align: "right", width: "70px" },
  { key: "sent", header: "Sent", cell: (f) => f.despatched_quantity, align: "right", width: "60px" },
];

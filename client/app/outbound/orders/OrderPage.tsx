import { Package } from "lucide-react";

import { Alert, Button, Card, DataTable, Page, PageHeader, Section, Skeleton, Stack, type Column } from "@ui/index";
import { useNavigate } from "@app/routing/Router";
import { Progress } from "@app/common/cells";
import type { OrderLineView } from "@domain/types";

import { OrderFacts, OrderFulfilments } from "./OrdersPage";
import type { OrderDesk } from "./useOrder";
import s from "./orders.module.css";

/**
 * An order's own page: what it is, what was asked for line by line, and the
 * fulfilments serving it. Reached from the dashboard, the orders list, or a
 * link somebody sent.
 */
export function OrderPage({ desk }: { desk: OrderDesk }) {
  const navigate = useNavigate();
  const read = desk.read;

  if (read.kind !== "ready") {
    return (
      <Page>
        <PageHeader title="Order" />
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

  const order = read.order;
  const only = order.fulfilments.length === 1 ? order.fulfilments[0]! : null;

  return (
    <Page>
      <PageHeader
        title={
          <>
            Order <span className={s.ref}>{order.confirmation_number ?? order.external_ref ?? "—"}</span>
          </>
        }
        description={order.customer_name ?? undefined}
        actions={
          only && (
            <Button variant="primary" icon={<Package />} onClick={() => navigate(`/pack/${only.fulfilment_id}`)}>
              Open pack bench
            </Button>
          )
        }
      />

      <Stack gap={5}>
        <Card>
          <OrderFacts order={order} />
        </Card>

        <Section title="Lines" count={order.lines.length}>
          <Card padded={false}>
            <DataTable
              aria-label="Order lines"
              columns={LINE_COLUMNS}
              rows={order.lines}
              rowKey={(l) => l.order_line_id}
              empty={<p className={s.none}>This order has no lines.</p>}
            />
          </Card>
        </Section>

        <Section title="Fulfilments" count={order.fulfilments.length}>
          <OrderFulfilments order={order} />
        </Section>
      </Stack>
    </Page>
  );
}

const LINE_COLUMNS: Column<OrderLineView>[] = [
  { key: "item", header: "Item", cell: (l) => l.item_code, sort: (l) => l.item_code, mono: true, width: "140px" },
  { key: "description", header: "Description", cell: (l) => l.description, sort: (l) => l.description, grow: true },
  { key: "ordered", header: "Ordered", cell: (l) => l.ordered_quantity, align: "right", width: "80px" },
  {
    key: "committed",
    header: "Committed",
    cell: (l) =>
      l.committed_quantity < l.ordered_quantity ? (
        <span className={s.short} title="Less committed than ordered">
          {l.committed_quantity}
        </span>
      ) : (
        l.committed_quantity
      ),
    align: "right",
    width: "100px",
  },
  {
    key: "picked",
    header: "Picked",
    // Picked here or reported picked elsewhere (D172): either way it is off the shelf.
    cell: (l) => <Progress done={Math.max(l.picked_quantity, l.external_picked_quantity)} of={l.committed_quantity} />,
    sort: (l) => (l.committed_quantity ? Math.max(l.picked_quantity, l.external_picked_quantity) / l.committed_quantity : 0),
    width: "150px",
  },
  { key: "packed", header: "Packed", cell: (l) => l.packed_quantity, align: "right", width: "80px" },
  { key: "sent", header: "Sent", cell: (l) => l.despatched_quantity, align: "right", width: "70px" },
];

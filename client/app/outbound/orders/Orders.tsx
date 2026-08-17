import {
  Band,
  Code,
  Face,
  Fact,
  FaceWell,
  Faint,
  Field,
  Key,
  Lamp,
  Panel,
  Pill,
  Row,
  Soft,
  Record,
  Records,
  Spacer,
  Stack,
} from "@design/index";
import { Link } from "@design/index";
import { href } from "@app/routing/location";
import type { FulfilmentSummary, OrderMatch } from "@domain/types";
import type { OrdersDesk } from "./useOrders";

/**
 * Finding an order by the number a customer quotes.
 *
 * Stages 1 and 2 of the recorded process, on one screen, where they are four
 * screens across two systems today.
 *
 * # It opens on the latest, not on an empty field
 *
 * A search box is only usable by somebody who already knows the number. The
 * screen therefore arrives with the most recent orders at this site already on
 * it, and searching is what you do when that list is not enough. The two lists
 * are labelled differently on purpose: "the latest" and "what matched" look
 * identical and mean different things.
 *
 * # Why a search that answers with a list
 *
 * It feels like a lookup and is not. D44 says an externally-authoritative order
 * is amended by cancel-and-reraise, so **one reference can legitimately name a
 * cancelled order and the one that replaced it** — and showing only the newest
 * would hide exactly the thing somebody ringing up about a changed order needs
 * to see.
 *
 * # Progress is quantities, never a status
 *
 * S44: no table in the fulfilment set carries a stored progress column, because
 * any label it held would be a function of four coverage quantities that can
 * disagree with it. So this draws the quantities and computes the gate from
 * them, and a partly-picked commitment says *how far* rather than merely
 * failing.
 */
export function Orders({ desk }: { desk: OrdersDesk }) {
  return (
    <Stack gap={3}>
      <Panel elevation="raised" frame="bezel" as="section">
        <Face>
          <FaceWell>
            <Row gap={4} align="end" wrap>
              <Field
                label="Confirmation number or reference"
                numeric={false}
                value={desk.reference}
                onChange={desk.type}
                onSubmit={() => void desk.search()}
                disabled={desk.state.kind === "searching"}
              />
              <Key
                live
                disabled={!desk.reference.trim() || desk.state.kind === "searching"}
                onClick={() => void desk.search()}
              >
                Find
              </Key>
              {desk.state.kind !== "listed" && (
                <Key disabled={desk.state.kind === "searching"} onClick={() => void desk.list()}>
                  Latest
                </Key>
              )}
            </Row>
          </FaceWell>
        </Face>
      </Panel>

      {desk.state.kind === "searching" && (
        <Panel elevation="raised" frame="bezel">
          <Face>
            <Faint>Loading…</Faint>
          </Face>
        </Panel>
      )}

      {desk.state.kind === "failed" && (
        <Panel elevation="raised" frame="bezel">
          <Face>
            <Row gap={3} wrap>
              <Lamp kind="finding" />
              <span>{desk.state.message}</span>
            </Row>
          </Face>
        </Panel>
      )}

      {desk.state.kind === "found" && desk.state.orders.length === 0 && (
        <Panel elevation="raised" frame="bezel">
          <Face>
            <Stack gap={2}>
              <Row gap={3} wrap>
                <Lamp kind="finding" />
                <span>Nothing here answers to</span>
                <Code>{desk.state.reference}</Code>
              </Row>
              {/* The scanned or quoted string echoed verbatim, for the same
                  reason the locator echoes one: somebody is comparing it
                  against a screen or a voice, and needs the characters. */}
              <Faint>Searched both confirmation number and external reference.</Faint>
            </Stack>
          </Face>
        </Panel>
      )}

      {desk.state.kind === "listed" && (
        <Panel elevation="raised" frame="bezel">
          <Face pad={false}>
            <Band count={desk.state.orders.length}>Latest orders</Band>
            {desk.state.orders.length === 0 && (
              <FaceWell>
                <Faint>No orders have come in at this site.</Faint>
              </FaceWell>
            )}
          </Face>
        </Panel>
      )}

      {(desk.state.kind === "found" || desk.state.kind === "listed") &&
        desk.state.orders.map((order) => <Order key={order.order_id} order={order} />)}
    </Stack>
  );
}

function Order({ order }: { order: OrderMatch }) {
  const superseded = order.supersedes_order_id !== null;
  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <FaceWell>
            <Row gap={3} align="baseline" wrap>
              <Soft>{order.confirmation_number ?? order.external_ref ?? "no reference"}</Soft>
              <Faint>{order.customer_name ?? "no customer named"}</Faint>
              <Spacer />
              {superseded && <Pill tone="state">replaces an earlier one</Pill>}
              <Pill tone={order.state === "cancelled" ? "state" : "good"}>{order.state}</Pill>
            </Row>
          </FaceWell>
        </Face>

        {order.fulfilments.length === 0 ? (
          <Face>
            <Faint>Nothing is committed against this order yet.</Faint>
          </Face>
        ) : (
          <Face pad={false}>
            <Band>Committed</Band>
            <FaceWell>
              <Records>
                {order.fulfilments.map((f) => (
                  <Commitment key={f.fulfilment_id} fulfilment={f} />
                ))}
              </Records>
            </FaceWell>
          </Face>
        )}
      </Stack>
    </Panel>
  );
}

function Commitment({ fulfilment: f }: { fulfilment: FulfilmentSummary }) {
  return (
    <Record
      /* **The way through to the bench**, which is what the page this replaces
         existed for: stage 1 is finding the order, stage 2 is opening the
         commitment, and a screen that shows the second without offering it has
         lost what the first one bought. */
      name={<Link href={href(`/pack/${f.fulfilment_id}`)}>{f.site_code ?? "no site"}</Link>}
      tags={<Faint>{f.line_count === 1 ? "1 line" : `${f.line_count} lines`}</Faint>}
      facts={
        <>
          <Fact value={f.packed_quantity} label="packed" />
          <Fact value={f.despatched_quantity} label="despatched" />
        </>
      }
      meta={
        <>
          {f.fully_picked ? (
            <Pill tone="good">picked</Pill>
          ) : (
            <Pill tone="state">{`${f.picked_quantity} of ${f.committed_quantity} picked`}</Pill>
          )}
          {/* D95: a gate answered from a stale cache says so. Never measured is
              not the same as measured a moment ago and is not drawn as though
              it were. */}
          <Faint>{f.as_at === null ? "never projected" : "projected"}</Faint>
        </>
      }
    />
  );
}

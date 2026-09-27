import { PackageCheck } from "lucide-react";

import { Alert, Badge, Button, Card, EmptyState, List, ListItem, Page, PageHeader, ScanField, Skeleton, Spacer, TextField } from "@ui/index";
import { Faint } from "@app/common/cells";
import { Thumb } from "@app/common/Thumb";
import type { PutawayCell } from "@domain/types";

import type { PutawayBench } from "./usePutaway";
import s from "./putaway-page.module.css";

/**
 * Put away (D171): what is on the dock, and where it could go.
 *
 * The mirror of the pick walk. Picking has one destination and many sources,
 * so the destination is scanned once. Put-away has one source and many
 * destinations, so the goods are selected and then the bin is scanned, once
 * per trip.
 *
 * `homes` is information, not instruction: each row says which bins already
 * hold that item, nearest first — what a directed put-away would score from,
 * offered raw. A scored suggestion can be added later without changing what
 * the ledger records.
 */
export function PutawayPage({ bench }: { bench: PutawayBench }) {
  const st = bench.status;

  if (st.kind !== "ready") {
    return (
      <Page>
        <PageHeader title="Put away" />
        {st.kind === "failed" ? (
          <Alert tone="danger">{st.message}</Alert>
        ) : (
          <Card>
            <Skeleton width="50%" />
          </Card>
        )}
      </Page>
    );
  }

  const { cells } = st.screen;

  return (
    <Page>
      <PageHeader title="Put away" />

      <Card
        title={bench.holding ? "Putting away" : "On the dock"}
        actions={
          (bench.holding || bench.bin) && (
            <>
              {bench.holding && <span className={s.code}>{bench.holding.item_code}</span>}
              {bench.bin && <Badge tone="success">{bench.bin.code}</Badge>}
            </>
          )
        }
      >
        <ScanField
          label={bench.holding ? "Scan the bin" : "Scan the item"}
          value={bench.scan.typed}
          onChange={bench.typeScan}
          onScan={(v) => void bench.read(v)}
          busy={bench.busy}
          refocus={bench.scan.refocus}
          hint={bench.holding ? "The bin label on the shelf." : "Or choose an item from the dock list below."}
        />
      </Card>

      {bench.problem && (
        <Alert tone="danger" onDismiss={bench.dismiss}>
          {bench.problem}
        </Alert>
      )}
      {bench.stowed && (
        <Alert tone="success" onDismiss={bench.dismiss}>
          {`${bench.stowed.quantity} × ${bench.stowed.code} into ${bench.stowed.bin}.`}
        </Alert>
      )}

      <Card title="To put away" count={cells.length} padded={false}>
        {cells.length === 0 ? (
          <EmptyState icon={<PackageCheck />} title="Dock clear" description="Everything received has been put away." />
        ) : (
          <List label="To put away">
            {cells.map((cell) => (
              <Cell key={cell.stock_id} cell={cell} bench={bench} held={bench.holding?.stock_id === cell.stock_id} />
            ))}
          </List>
        )}
      </Card>
    </Page>
  );
}

function Cell({ cell, bench, held }: { cell: PutawayCell; bench: PutawayBench; held: boolean }) {
  const claimed = cell.available < cell.quantity;

  return (
    <ListItem
      current={held}
      lead={
        <span className={s.lead}>
          <Thumb picture={cell.picture} alt={cell.description ?? cell.item_code} />
          <span className={s.location}>{cell.location_code}</span>
        </span>
      }
      title={cell.item_code}
      badges={cell.lot_code ? <Badge>{cell.lot_code}</Badge> : undefined}
      description={cell.description ?? undefined}
      meta={
        <>
          <span>{cell.quantity} on the dock</span>
          {/* Some of it is already spoken for. It does not stop a put-away —
              the goods still need a shelf. */}
          {claimed && <span>{cell.quantity - cell.available} claimed</span>}
          {/* Where it already lives, offered as buttons: consolidating is the
              common answer, and walking to a bin you can already see named is
              not a decision worth two taps. */}
          {cell.homes.length === 0 ? (
            <span>No other bins</span>
          ) : (
            <span className={s.homes}>
              <span>Also in</span>
              {cell.homes.map((home) => (
                <Button
                  key={home.location_id}
                  size="sm"
                  variant="ghost"
                  disabled={bench.busy || !held}
                  onClick={() => bench.chooseBin({ id: home.location_id, code: home.location_code })}
                >
                  {`${home.location_code} · ${home.quantity}`}
                </Button>
              ))}
            </span>
          )}
        </>
      }
      action={
        <Button size="sm" disabled={bench.busy || held} onClick={() => bench.choose(cell)}>
          {held ? "Selected" : "Select"}
        </Button>
      }
    />
  );
}

/**
 * The dock: what is waiting, and the one thing there is to press. The quantity
 * and the commit live here rather than on the row — the row is up a scrolling
 * list and the hand holding the phone is down here.
 */
export function PutawayDockPage({ bench }: { bench: PutawayBench }) {
  const site = bench.status.kind === "ready" ? bench.status.screen.site : "—";
  const count = bench.status.kind === "ready" ? bench.status.screen.cells.length : 0;
  const cell = bench.holding;

  if (!cell) {
    return (
      <div className={s.row}>
        <Badge>{site}</Badge>
        {count > 0 && (
          <span>
            <strong>{count}</strong> <Faint>to put away</Faint>
          </span>
        )}
        <Spacer />
        <Button disabled={bench.busy} onClick={() => void bench.refresh()}>
          Refresh
        </Button>
      </div>
    );
  }

  return (
    <form
      className={s.dock}
      onSubmit={(e) => {
        e.preventDefault();
        void bench.away();
      }}
    >
      <div className={s.row}>
        <span className={s.code}>{cell.item_code}</span>
        <Spacer />
        {bench.bin ? (
          <Badge tone="success">{bench.bin.code}</Badge>
        ) : (
          // A prompt, not a refusal: Put away is disabled until the bin is named.
          <Faint>Scan a bin</Faint>
        )}
      </div>
      <div className={s.row}>
        <div className={s.quantity}>
          <TextField
            label="Putting away"
            inputMode="numeric"
            autoComplete="off"
            value={bench.quantity}
            onChange={(e) => bench.typeQuantity(e.target.value)}
            disabled={bench.busy}
            trailing={`of ${cell.quantity}`}
          />
        </div>
        <Spacer />
        <Button disabled={bench.busy} onClick={bench.release}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" disabled={bench.busy || !bench.bin}>
          Put away
        </Button>
      </div>
    </form>
  );
}

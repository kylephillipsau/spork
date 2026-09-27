import { PackageCheck } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  EmptyState,
  List,
  ListItem,
  Page,
  PageHeader,
  ScanField,
  Skeleton,
  Spacer,
  TextField,
} from "@ui/index";
import { Faint } from "@app/common/cells";
import { Thumb } from "@app/common/Thumb";
import type { PickLine } from "@domain/types";

import type { PickBench } from "./usePicking";
import s from "./picking-page.module.css";

/**
 * Picking (D171): what to pick, where it is, and what it looks like.
 *
 * **The picture is the point of this screen.** A code and a bin are enough for
 * somebody who knows the catalogue; a new hire needs to see what they are
 * looking for. D141 lets the picture come from the item's style when the code
 * has never been photographed, and the borrowed picture says so.
 *
 * The rows are sorted by `location.pick_sequence`, so the list is a route
 * rather than one trip per order.
 *
 * **One scanner, and it answers whichever question is open** (D166): the first
 * scan of a walk names the pallet or packing station, and every scan after it
 * confirms the item on a row. The one primary action, Pick, is in the dock.
 */
export function PickingPage({ bench }: { bench: PickBench }) {
  const st = bench.status;

  if (st.kind !== "ready") {
    return (
      <Page>
        <PageHeader title="Picking" />
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

  const { lines } = st.screen;

  return (
    <Page>
      <PageHeader title="Picking" />

      <Where bench={bench} />

      {bench.problem && (
        <Alert tone="danger" onDismiss={bench.dismiss}>
          {bench.problem}
        </Alert>
      )}
      {bench.took && (
        <Alert tone="success" onDismiss={bench.dismiss}>
          {`${bench.took.quantity} × ${bench.took.code} onto ${bench.destination?.code ?? "it"}.`}
          {bench.took.warnings.map((w) => (
            <div key={w}>{w}</div>
          ))}
        </Alert>
      )}

      <Card title="To pick" count={lines.length} padded={false}>
        {lines.length === 0 ? (
          <EmptyState icon={<PackageCheck />} title="Nothing to pick" description="All picked." />
        ) : (
          <List label="To pick">
            {lines.map((line) => (
              <Line
                key={line.fulfilment_line_id}
                line={line}
                bench={bench}
                confirmed={bench.confirmed?.fulfilment_line_id === line.fulfilment_line_id}
              />
            ))}
          </List>
        )}
      </Card>
    </Page>
  );
}

/**
 * Where the goods are going, and the scanner that asks. Above the walk because
 * it gates it: until it is answered there is nothing to do with an item.
 */
function Where({ bench }: { bench: PickBench }) {
  const to = bench.destination;
  return (
    <Card
      title="Picking onto"
      actions={
        to ? (
          <>
            <span className={s.code}>{to.code}</span>
            <Badge>{to.kind}</Badge>
            <Button size="sm" disabled={bench.busy} onClick={bench.clearDestination}>
              Change
            </Button>
          </>
        ) : (
          <Faint>Not set</Faint>
        )
      }
    >
      <ScanField
        label={to ? "Scan the item" : "Scan the pallet or station"}
        value={bench.scan.typed}
        onChange={bench.typeScan}
        onScan={(v) => void bench.read(v)}
        busy={bench.busy}
        refocus={bench.scan.refocus}
        hint={to ? "Scan an item on the list." : "Pallet, cage or packing station label."}
      />
    </Card>
  );
}

function Line({ line, bench, confirmed }: { line: PickLine; bench: PickBench; confirmed: boolean }) {
  // **A short pick, before it happens.** The cell holds less than the line
  // wants, and saying so on the walk lets the picker find the rest while they
  // are already in the aisle.
  const short = line.available !== null && line.available < line.remaining;

  return (
    <ListItem
      current={confirmed}
      lead={
        <div className={s.lead}>
          <Thumb picture={line.picture} alt={line.description ?? line.item_code} />
          {/* The bin first: it is what the picker walks to. */}
          {line.location_code ? <span className={s.bin}>{line.location_code}</span> : <Badge tone="warning">no stock</Badge>}
        </div>
      }
      title={line.item_code}
      badges={
        <>
          {line.lot_code && <Badge>{line.lot_code}</Badge>}
          {line.allocated && <Badge tone="success">claimed</Badge>}
          {confirmed && <Badge tone="success">picked</Badge>}
        </>
      }
      description={line.description}
      meta={
        <>
          <span>
            <strong className={s.figure}>{line.remaining}</strong> to pick
          </span>
          {short && (
            <span className={s.short}>
              <strong className={s.figure}>{line.available ?? 0}</strong> free in that bin
            </span>
          )}
          {line.reference && <span>{line.reference}</span>}
        </>
      }
      action={
        // Scanning is how a row is confirmed; this is for the label that will
        // not scan.
        line.stock_id ? (
          <Button size="sm" disabled={bench.busy || !bench.destination || confirmed} onClick={() => bench.choose(line)}>
            {confirmed ? "Confirmed" : "This one"}
          </Button>
        ) : undefined
      }
    />
  );
}

/**
 * The dock: what is left, and the one thing there is to press. The quantity
 * lives here rather than on the row because the row may be far up the list
 * and the hand holding the handheld is down here (D134).
 */
export function PickingDock({ bench }: { bench: PickBench }) {
  const site = bench.status.kind === "ready" ? bench.status.screen.site : "—";
  const count = bench.status.kind === "ready" ? bench.status.screen.lines.length : 0;
  const line = bench.confirmed;

  if (!line) {
    return (
      <div className={s.dockRow}>
        <Badge>{site}</Badge>
        {count > 0 && (
          <span>
            <strong className={s.figure}>{count}</strong> <Faint>to pick</Faint>
          </span>
        )}
        <Spacer />
        <Button size="sm" disabled={bench.busy} onClick={() => void bench.refresh()}>
          Refresh
        </Button>
      </div>
    );
  }

  const asked = Number.parseInt(bench.quantity.trim(), 10);
  const short = Number.isFinite(asked) && line.available !== null && asked > line.available;

  return (
    <form
      className={s.dockForm}
      onSubmit={(e) => {
        e.preventDefault();
        void bench.take();
      }}
    >
      <div className={s.dockRow}>
        <span className={s.code}>{line.item_code}</span>
        <Faint>{line.location_code ?? "no bin"}</Faint>
        <Spacer />
        {/* Said before it is pressed, while the picker is still at the shelf. */}
        {short && <Badge tone="warning">more than the bin holds</Badge>}
      </div>
      <div className={s.dockRow}>
        <div className={s.quantity}>
          <TextField
            label="Picked"
            inputMode="numeric"
            autoComplete="off"
            value={bench.quantity}
            onChange={(e) => bench.typeQuantity(e.target.value)}
            disabled={bench.busy}
            trailing={`of ${line.remaining}`}
          />
        </div>
        <Spacer />
        <Button disabled={bench.busy} onClick={bench.release}>
          Not this
        </Button>
        <Button type="submit" variant="primary" disabled={bench.busy}>
          Pick
        </Button>
      </div>
    </form>
  );
}

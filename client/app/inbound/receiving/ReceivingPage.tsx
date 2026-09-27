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
  Select,
  Skeleton,
  Spacer,
  TextField,
} from "@ui/index";
import type { ExpectedLine } from "@domain/types";
import { provenance } from "@app/measurement/baseline";
import { Faint } from "@app/common/cells";
import { Thumb } from "@app/common/Thumb";
import { grams } from "@app/common/format";

import type { ReceivingBench } from "./useReceiving";
import s from "./receiving-page.module.css";

/**
 * Receiving (D171): what is expected here, and checking it in.
 *
 * A delivery is a session (D43, Q172): one truck is one `goods_receipt`, and
 * lines join by carrying the same id, so the delivery stays open until it is
 * closed. A GS1-128 carton label carries the lot and expiry beside the GTIN,
 * so one scan names the line and fills both. Everything the write path refuses
 * on is said before the press.
 */
export function ReceivingPage({ bench }: { bench: ReceivingBench }) {
  const st = bench.status;

  return (
    <Page>
      <PageHeader
        title="Receiving"
        actions={
          bench.delivery ? (
            <>
              <Badge tone="success" dot>
                Delivery open
              </Badge>
              <Button size="sm" disabled={bench.busy} onClick={bench.closeDelivery}>
                Close delivery
              </Button>
            </>
          ) : undefined
        }
      />

      {st.kind === "failed" && <Alert tone="danger">{st.message}</Alert>}

      {st.kind !== "failed" && (
        <Card>
          <ScanField
            label={bench.bay ? `Scan the carton into ${bench.bay.code}` : "Scan the dock or bin"}
            value={bench.scan.typed}
            onChange={bench.typeScan}
            onScan={(v) => void bench.read(v)}
            busy={bench.busy}
            refocus={bench.scan.refocus}
            hint={
              bench.bay
                ? "A GS1 label fills in the item, lot and date."
                : "The dock or bin where this delivery is unloaded."
            }
          />
        </Card>
      )}

      {bench.problem && (
        <Alert tone="danger" onDismiss={bench.dismiss}>
          {bench.problem}
        </Alert>
      )}
      <Landed bench={bench} />
      {bench.counting && <Counting bench={bench} line={bench.counting} />}

      {st.kind === "loading" && (
        <Card>
          <Skeleton width="50%" />
        </Card>
      )}
      {st.kind === "ready" && (
        <Card title="Expected" count={st.screen.lines.length} padded={false}>
          {st.screen.lines.length === 0 ? (
            <EmptyState
              icon={<PackageCheck />}
              title="Nothing expected"
              description="All expected deliveries have been received."
            />
          ) : (
            <List label="Expected">
              {st.screen.lines.map((line) => (
                <Line
                  key={line.expected_supply_id}
                  line={line}
                  bench={bench}
                  counting={bench.counting?.expected_supply_id === line.expected_supply_id}
                />
              ))}
            </List>
          )}
        </Card>
      )}
    </Page>
  );
}

/** What the last line did: accepted, refused, or accepted with warnings. */
function Landed({ bench }: { bench: ReceivingBench }) {
  const it = bench.landed;
  if (!it) return null;
  return (
    <Alert tone={it.accepted ? "success" : "warning"} onDismiss={bench.dismiss}>
      {it.accepted
        ? `${it.entered} of ${it.code} — ${it.quantity} in.`
        : // A refusal is not an error: the goods are on the dock either way,
          // what did not happen is the record.
          `${it.code} not received: this item needs a lot.`}
      {it.warnings.map((w) => (
        <div key={w} className={s.warning}>
          {w}
        </div>
      ))}
    </Alert>
  );
}

/** The line being counted: what is on the label, and what it comes to. */
function Counting({ bench, line }: { bench: ReceivingBench; line: ExpectedLine }) {
  const over = bench.base !== null ? Math.max(0, bench.base - line.outstanding) : 0;
  // What one of whatever is being counted has weighed before. Usually null.
  const perUnit = line.levels.find((l) => l.level === bench.level)?.baseline ?? null;

  return (
    <Card
      title={<span className={s.code}>{line.item_code}</span>}
      description={line.order_number ?? undefined}
      actions={
        <Button size="sm" variant="ghost" disabled={bench.busy} onClick={bench.release}>
          Not this
        </Button>
      }
    >
      <form
        className={s.count}
        onSubmit={(e) => {
          e.preventDefault();
          if (!bench.busy && bench.missing === null) void bench.receive();
        }}
      >
        <div className={s.row}>
          <div className={s.qty}>
            <TextField
              label="Counted"
              inputMode="numeric"
              autoComplete="off"
              value={bench.entered}
              onChange={(e) => bench.typeEntered(e.target.value)}
              disabled={bench.busy}
            />
          </div>
          {/* Counted in what was counted (D92, Q173): only the levels this
              item has a config for are offered. */}
          {line.levels.length > 1 && (
            <div className={s.level}>
              <Select
                label="of"
                value={bench.level}
                onValueChange={bench.chooseLevel}
                options={line.levels.map((l) => ({ value: l.level, label: l.level }))}
                disabled={bench.busy}
              />
            </div>
          )}
          {bench.base !== null && (
            <span className={s.units}>
              <strong>{bench.base}</strong> units
            </span>
          )}
          {/* More than promised. Whether that is a finding is the server's
              tolerance to decide, so this states the fact only. */}
          {over > 0 && <Badge tone="warning">{`${over} more than expected`}</Badge>}
        </div>

        {/* The third witness: a scale belongs to neither the paperwork nor the
            count. Shown only when a baseline exists, and nothing here is
            written back to it (see @app/measurement/baseline). */}
        {perUnit && (
          <div className={s.row}>
            <div className={s.qty}>
              {/* Goods only: there is no tare here, so a pallet under the
                  cartons would read as cartons that are not there. */}
              <TextField
                label="Weighed (goods only)"
                inputMode="decimal"
                autoComplete="off"
                trailing="kg"
                value={bench.weighed}
                onChange={(e) => bench.typeWeighed(e.target.value)}
                disabled={bench.busy}
              />
            </div>
            {bench.scale && <strong>{bench.scale.sentence}</strong>}
            <Spacer />
            <Faint>
              {grams(perUnit.grams)} kg a {bench.level}, {provenance(perUnit)}
            </Faint>
          </div>
        )}

        {/* The lot, which one scan usually fills. Offered even when the policy
            does not require one. */}
        <div className={s.row}>
          <div className={s.lot}>
            <TextField
              label={line.requires_lot ? "Lot (required)" : "Lot"}
              autoComplete="off"
              value={bench.lotCode}
              onChange={(e) => bench.typeLot(e.target.value)}
              disabled={bench.busy}
            />
          </div>
          <div className={s.lot}>
            <TextField
              label="Expiry"
              autoComplete="off"
              value={bench.lotExpiry}
              onChange={(e) => bench.typeExpiry(e.target.value)}
              disabled={bench.busy}
            />
          </div>
        </div>

        {/* Whose the goods are, when the promise did not say: the parties
            already holding this item here, offered rather than defaulted. */}
        {!line.owner_id && line.owners.length > 0 && (
          <div className={s.row}>
            <span className={s.label}>Owner</span>
            {line.owners.map((o) => (
              <Button
                key={o.owner_id}
                size="sm"
                variant={bench.owner === o.owner_id ? "primary" : "secondary"}
                aria-pressed={bench.owner === o.owner_id}
                disabled={bench.busy}
                onClick={() => bench.chooseOwner(o.owner_id)}
              >
                {o.name}
              </Button>
            ))}
          </div>
        )}

        <div className={s.row}>
          <Faint>{bench.missing ?? "Ready."}</Faint>
          <Spacer />
          {/* Not a receipt of nothing: closing short says the rest is not
              coming, where a supplier conversation can find it. */}
          <Button disabled={bench.busy} onClick={() => void bench.closeShort()}>
            Close short
          </Button>
          <Button type="submit" variant="primary" disabled={bench.busy || bench.missing !== null}>
            Receive
          </Button>
        </div>
      </form>
    </Card>
  );
}

function Line({ line, bench, counting }: { line: ExpectedLine; bench: ReceivingBench; counting: boolean }) {
  return (
    <ListItem
      current={counting}
      lead={<Thumb picture={line.picture} alt={line.description ?? line.item_code} />}
      title={line.item_code}
      badges={
        <>
          {/* Said on the row, so it is known before the pallet is opened. */}
          {line.requires_lot && <Badge tone="warning">By lot</Badge>}
          {counting && <Badge tone="accent">Counting</Badge>}
        </>
      }
      description={line.description}
      meta={
        <>
          <span>
            <strong className={s.figure}>{line.outstanding}</strong> outstanding
          </span>
          {line.received > 0 && (
            <span>
              <strong className={s.figure}>{line.received}</strong> received
            </span>
          )}
          {line.supplier && <span>{line.supplier}</span>}
          {line.order_number && <span>{line.order_number}</span>}
        </>
      }
      action={
        <Button size="sm" disabled={bench.busy || !bench.bay || counting} onClick={() => bench.choose(line)}>
          {counting ? "Counting" : "This one"}
        </Button>
      }
    />
  );
}

/** The dock: what is expected, where it is going, and a refresh. */
export function ReceivingDockPage({ bench }: { bench: ReceivingBench }) {
  const site = bench.status.kind === "ready" ? bench.status.screen.site : "—";
  const count = bench.status.kind === "ready" ? bench.status.screen.lines.length : 0;
  return (
    <div className={s.dock}>
      <Badge>{site}</Badge>
      {count > 0 && (
        <span>
          <strong className={s.figure}>{count}</strong> expected
        </span>
      )}
      {bench.bay && <Faint>{`into ${bench.bay.code}`}</Faint>}
      <Spacer />
      <Button disabled={bench.busy} onClick={() => void bench.refresh()}>
        Refresh
      </Button>
    </div>
  );
}

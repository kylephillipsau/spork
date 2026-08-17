import {
  Band,
  Code,
  EmptySlot,
  Face,
  FaceWell,
  Fact,
  Faint,
  Field,
  Key,
  NoPhoto,
  Notice,
  Panel,
  Photo,
  Pill,
  Record,
  Records,
  Row,
  ScanInput,
  Soft,
  Spacer,
  Stack,
  Steel,
  Trailing,
} from "@design/index";
import { imageUrl } from "@domain/api";
import type { PutawayCell } from "@domain/types";
import type { PutawayBench } from "./usePutaway";

/**
 * What is on the dock, and where it could go.
 *
 * # The mirror of the pick walk
 *
 * Picking has one destination and many sources: the destination is scanned once
 * and every scan after it confirms a row. Put-away has one source and many
 * destinations: everything is on the dock and each thing has its own home, so
 * the goods are selected and then the bin is scanned, once per trip.
 *
 * The two screens are deliberately mirror images, and neither could be the other
 * with the arguments swapped.
 *
 * # `homes` is information, not instruction
 *
 * Each row says which bins already hold that item, nearest first. That is what a
 * directed put-away would compute a score from, offered raw instead — the
 * operator can see whether it fits and the system cannot. A scored suggestion
 * can be added later without changing what the ledger records, which is what
 * makes leaving it out now cost nothing.
 */
export function Putaway({ bench }: { bench: PutawayBench }) {
  if (bench.status.kind === "loading") {
    return (
      <Panel elevation="lifted" frame="bezel" as="section">
        <Face>
          <Soft>Loading…</Soft>
        </Face>
      </Panel>
    );
  }

  if (bench.status.kind === "failed") {
    return (
      <Panel elevation="lifted" frame="bezel" as="section">
        <Notice>{bench.status.message}</Notice>
      </Panel>
    );
  }

  const { cells } = bench.status.screen;

  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <Stack gap={3}>
            <Row gap={3} align="baseline" wrap>
              <Soft>{bench.holding ? "Putting away" : "On the dock"}</Soft>
              <Spacer />
              {bench.holding && <Code>{bench.holding.item_code}</Code>}
              {bench.bin && <Pill tone="good">{bench.bin.code}</Pill>}
            </Row>
            <ScanInput
              label={bench.holding ? "Scan the bin" : "Scan what you picked up"}
              value={bench.scan.typed}
              onChange={bench.typeScan}
              onScan={(v) => void bench.read(v)}
              busy={bench.busy}
              refocus={bench.scan.refocus}
              hint={
                bench.holding
                  ? "The label on the shelf you are standing at."
                  : "Or choose it from the dock below."
              }
            />
          </Stack>
        </Face>

        {bench.problem && <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>}
        {bench.stowed && (
          <Notice kind="recorded" onDismiss={bench.dismiss}>
            {`${bench.stowed.quantity} × ${bench.stowed.code} into ${bench.stowed.bin}.`}
          </Notice>
        )}

        <Face pad={false} as="section">
          <Band count={cells.length}>To put away</Band>
          <FaceWell>
            {cells.length === 0 ? (
              <EmptySlot
                label="Dock clear"
                note="Everything that arrived has a home."
              />
            ) : (
              <Records>
                {cells.map((cell) => (
                  <Cell
                    key={cell.stock_id}
                    cell={cell}
                    bench={bench}
                    held={bench.holding?.stock_id === cell.stock_id}
                  />
                ))}
              </Records>
            )}
          </FaceWell>
        </Face>
      </Stack>
    </Panel>
  );
}

function Cell({
  cell,
  bench,
  held,
}: {
  cell: PutawayCell;
  bench: PutawayBench;
  held: boolean;
}) {
  const claimed = cell.available < cell.quantity;

  return (
    <Record
      media={
        cell.picture ? (
          <Photo
            src={imageUrl(cell.picture.digest)}
            source={cell.picture.source}
            alt={cell.description ?? cell.item_code}
          />
        ) : (
          <NoPhoto />
        )
      }
      lead={<Code>{cell.location_code}</Code>}
      name={<Code>{cell.item_code}</Code>}
      tags={cell.lot_code ? <Pill tone="quiet">{cell.lot_code}</Pill> : undefined}
      facts={
        <>
          <Fact value={cell.quantity} label="on the dock" />
          {/* Some of it is already spoken for. It does not stop a put-away —
              the goods still need a shelf — and it is worth knowing before
              somebody wonders where it went. */}
          {claimed && <Fact value={cell.quantity - cell.available} label="claimed" />}
        </>
      }
      note={cell.description && <Faint>{cell.description}</Faint>}
      meta={
        <>
          {/* **Where it already lives.** The raw fact a directed put-away would
              score from, offered instead of scored — and offered as keys,
              because consolidating is the common answer and walking to a bin
              you can already see named is not a decision worth two taps. */}
          {cell.homes.length === 0 ? (
            <Faint>nowhere yet</Faint>
          ) : (
            <>
              <Faint>already in</Faint>
              {cell.homes.map((home) => (
                <Key
                  key={home.location_id}
                  size="small"
                  disabled={bench.busy || !held}
                  onClick={() =>
                    bench.chooseBin({ id: home.location_id, code: home.location_code })
                  }
                >
                  {`${home.location_code} · ${home.quantity}`}
                </Key>
              ))}
            </>
          )}
        </>
      }
      action={
        <Key
          size="small"
          disabled={bench.busy || held}
          onClick={() => bench.choose(cell)}
        >
          {held ? "In your hands" : "This one"}
        </Key>
      }
    />
  );
}

/**
 * The dock: what is waiting, and the one thing there is to press.
 *
 * The quantity and the commit live here rather than on the row, the same
 * argument `PickDock` and `CaptureDock` make — the row is up a scrolling list
 * and the hand holding the phone is down here.
 */
export function PutawayDock({ bench }: { bench: PutawayBench }) {
  const site = bench.status.kind === "ready" ? bench.status.screen.site : "—";
  const count = bench.status.kind === "ready" ? bench.status.screen.cells.length : 0;
  const cell = bench.holding;

  if (!cell) {
    return (
      <Row gap={3} align="center">
        <Pill tone="quiet">{site}</Pill>
        {count > 0 && (
          <>
            <Steel>{count}</Steel>
            <Faint>to put away</Faint>
          </>
        )}
        <Spacer />
        <Key size="small" disabled={bench.busy} onClick={() => void bench.refresh()}>
          Refresh
        </Key>
      </Row>
    );
  }

  return (
    <Stack gap={2}>
      <Row gap={3} align="baseline" wrap>
        <Code>{cell.item_code}</Code>
        <Spacer />
        {bench.bin ? (
          <Pill tone="good">{bench.bin.code}</Pill>
        ) : (
          /* Not a refusal, a prompt: the key below is disabled until the bin is
             named, and saying which bin is the whole act. */
          <Faint>scan a bin</Faint>
        )}
      </Row>
      <Row gap={3} align="end" wrap>
        <Field
          label="Putting away"
          width="inline"
          value={bench.quantity}
          onChange={bench.typeQuantity}
          disabled={bench.busy}
          onSubmit={() => void bench.away()}
        />
        <Faint>{`of ${cell.quantity}`}</Faint>
        <Trailing>
          <Key size="small" disabled={bench.busy} onClick={bench.release}>
            Put it back
          </Key>
          <Key live disabled={bench.busy || !bench.bin} onClick={() => void bench.away()}>
            Put away
          </Key>
        </Trailing>
      </Row>
    </Stack>
  );
}

import {
  Band,
  Code,
  EmptySlot,
  Face,
  FaceWell,
  Fact,
  Field,
  Faint,
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
import type { PickLine } from "@domain/types";
import type { PickBench } from "./usePicking";

/**
 * What to pick, where it is, and what it looks like.
 *
 * **The picture is the point of this screen.** A code and a bin are enough for
 * somebody who knows the catalogue; the person who does not is the person a
 * warehouse hires this week, and *what am I looking for* is the question the
 * codes cannot answer. D141 lets the picture come from the item's style when
 * the code itself has never been photographed, which is what makes it useful
 * across thirteen sizes of one boot — and the borrowed picture says so, because
 * an unlabelled one claims to be a photograph of the code in front of you.
 *
 * # In walking order, not in order order
 *
 * The rows are sorted by `location.pick_sequence`, so the list is a route. A
 * list grouped by order is a list that sends somebody up and down the same
 * aisle once per customer — which is the whole reason that column exists and
 * why J71 reports two bins claiming one position.
 *
 * # One scanner, and it answers whichever question is open
 *
 * This screen had no primary action, because recording a pick needs somewhere
 * for the goods to land and which somewhere was a question the model declined
 * to answer for another warehouse's floor. D166 answered it and the picker
 * answers it here: the first scan of a walk names the pallet or the packing
 * station, and every scan after it is *is this the thing on the row*.
 *
 * **Two questions, one field.** Two scan inputs on one screen is two places to
 * aim a reader and one of them always wrong, and a handheld has no room for the
 * mistake. What the field is asking is written under it and changes with the
 * answer already given.
 *
 * The lit key is Pick, in the dock, and it is the only one — D116, and the
 * reachable third, so a long walk cannot scroll it out of the hand holding it.
 */
export function PickList({ bench }: { bench: PickBench }) {
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

  const { lines } = bench.status.screen;

  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Where bench={bench} />

        {bench.problem && <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>}
        {bench.took && (
          <Notice
            kind="recorded"
            onDismiss={bench.dismiss}
            notes={bench.took.warnings.map((w) => (
              <Faint key={w}>{w}</Faint>
            ))}
          >
            {`${bench.took.quantity} × ${bench.took.code} onto ${bench.destination?.code ?? "it"}.`}
          </Notice>
        )}

        <Face pad={false} as="section">
          <Band count={lines.length}>To pick</Band>
          <FaceWell>
            {lines.length === 0 ? (
              <EmptySlot
                label="Nothing to pick"
                note="All picked."
              />
            ) : (
              <Records>
                {lines.map((line) => (
                  <Line
                    key={line.fulfilment_line_id}
                    line={line}
                    bench={bench}
                    confirmed={bench.confirmed?.fulfilment_line_id === line.fulfilment_line_id}
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

/**
 * Where the goods are going, and the scanner that asks.
 *
 * **Above the walk, because it gates it.** Until this is answered there is
 * nothing to do with an item, and a screen that let somebody scan twelve rows
 * and then asked where they had put them would be asking a question it had
 * watched them answer.
 */
function Where({ bench }: { bench: PickBench }) {
  const to = bench.destination;
  return (
    <Face>
      <Stack gap={3}>
        <Row gap={3} align="baseline" wrap>
          <Soft>Picking onto</Soft>
          <Spacer />
          {to ? (
            <>
              <Code>{to.code}</Code>
              <Pill tone="quiet">{to.kind}</Pill>
              <Key size="small" disabled={bench.busy} onClick={bench.clearDestination}>
                Change
              </Key>
            </>
          ) : (
            <Faint>Not set</Faint>
          )}
        </Row>
        <ScanInput
          label={to ? "Scan the item" : "Scan the pallet or station"}
          value={bench.scan.typed}
          onChange={bench.typeScan}
          onScan={(v) => void bench.read(v)}
          busy={bench.busy}
          refocus={bench.scan.refocus}
          hint={
            to
              ? "Scan an item on the list."
              : "Pallet, cage or packing station label."
          }
        />
      </Stack>
    </Face>
  );
}

function Line({
  line,
  bench,
  confirmed,
}: {
  line: PickLine;
  bench: PickBench;
  confirmed: boolean;
}) {
  // **A short pick, before it happens.** The cell holds less than the line
  // wants, and saying so on the walk is worth more than saying so afterwards:
  // the picker can go and find the rest while they are already in the aisle.
  const short = line.available !== null && line.available < line.remaining;

  return (
    <Record
      media={
        line.picture ? (
          <Photo
            src={imageUrl(line.picture.digest)}
            source={line.picture.source}
            alt={line.description ?? line.item_code}
          />
        ) : (
          <NoPhoto />
        )
      }
      /* The bin first: it is what the picker walks to, and the code is what
         they check when they get there. */
      lead={
        line.location_code ? (
          <Code>{line.location_code}</Code>
        ) : (
          <Pill tone="state">no stock</Pill>
        )
      }
      name={<Code>{line.item_code}</Code>}
      tags={
        <>
          {line.lot_code && <Pill tone="quiet">{line.lot_code}</Pill>}
          {line.allocated && <Pill tone="good">claimed</Pill>}
        </>
      }
      facts={
        <>
          {/* **A fact, though it is the number the picker acts on.** D167's
              rule holds even here: a Readout is one instrument on a bench, and
              a list of twelve lines cannot have twelve of them. It reads at
              arm's length because floor density sets the row's own size. */}
          <Fact value={line.remaining} label="to pick" />
          {/* **A short pick, before it happens.** The cell holds less than the
              line wants, and saying so on the walk is worth more than saying
              so afterwards: the picker can find the rest while in the aisle. */}
          {short && <Fact value={line.available ?? 0} label="free in that bin" />}
        </>
      }
      note={line.description && <Faint>{line.description}</Faint>}
      meta={
        <>
          {line.reference && <Faint>{line.reference}</Faint>}
          {confirmed && <Pill tone="good">picked</Pill>}
        </>
      }
      action={
        /* **Not lit, and not the way in.** Scanning is how a row is confirmed;
           this is for the reader that will not read a rubbed-off label, which
           is the case every barcode screen has to keep working without. */
        line.stock_id ? (
          <Key
            size="small"
            disabled={bench.busy || !bench.destination || confirmed}
            onClick={() => bench.choose(line)}
          >
            {confirmed ? "Confirmed" : "This one"}
          </Key>
        ) : undefined
      }
    />
  );
}

/** The dock: where it is, and the one thing there is to do. */
/**
 * The dock: what is left, and the one thing there is to press.
 *
 * The quantity lives here rather than on the row because the row is somewhere
 * up a scrolling list and the hand holding the phone is down here — the same
 * argument `CaptureDock` makes, and the reason `FloorShell` has a dock at all.
 */
export function PickDock({ bench }: { bench: PickBench }) {
  const site = bench.status.kind === "ready" ? bench.status.screen.site : "—";
  const count = bench.status.kind === "ready" ? bench.status.screen.lines.length : 0;
  const line = bench.confirmed;

  if (!line) {
    return (
      <Row gap={3} align="center">
        <Pill tone="quiet">{site}</Pill>
        {count > 0 && (
          <>
            <Steel>{count}</Steel>
            <Faint>to pick</Faint>
          </>
        )}
        <Spacer />
        <Key size="small" disabled={bench.busy} onClick={() => void bench.refresh()}>
          Refresh
        </Key>
      </Row>
    );
  }

  const asked = Number.parseInt(bench.quantity.trim(), 10);
  const short = Number.isFinite(asked) && line.available !== null && asked > line.available;

  return (
    <Stack gap={2}>
      <Row gap={3} align="baseline" wrap>
        <Code>{line.item_code}</Code>
        <Faint>{line.location_code ?? "no bin"}</Faint>
        <Spacer />
        {/* **Said before it is pressed.** The bin holds less than this asks
            for, and a picker who learns that from a refusal has already walked
            away from the shelf they would have to go back to. */}
        {short && <Pill tone="state">more than the bin holds</Pill>}
      </Row>
      <Row gap={3} align="end" wrap>
        <Field
          label="Picked"
          width="inline"
          value={bench.quantity}
          onChange={bench.typeQuantity}
          disabled={bench.busy}
          onSubmit={() => void bench.take()}
        />
        <Faint>{`of ${line.remaining}`}</Faint>
        <Trailing>
          <Key size="small" disabled={bench.busy} onClick={bench.release}>
            Not this
          </Key>
          <Key live disabled={bench.busy} onClick={() => void bench.take()}>
            Pick
          </Key>
        </Trailing>
      </Row>
    </Stack>
  );
}

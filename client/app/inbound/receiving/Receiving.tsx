import {
  Band,
  Chooser,
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
  grams,
} from "@design/index";
import { imageUrl } from "@domain/api";
import type { ExpectedLine } from "@domain/types";
import { provenance } from "@app/measurement/baseline";
import type { ReceivingBench } from "./useReceiving";

/**
 * What is expected here, and checking it in.
 *
 * # A delivery is a session
 *
 * D43 and Q172: one truck is one `goods_receipt`, and lines join by carrying the
 * same id. So the header at the top is not decoration — it is the delivery being
 * held open, and closing it is what makes the next line a new one.
 *
 * # The scan is worth more here than anywhere
 *
 * A GS1-128 carton label carries the lot and the expiry beside the GTIN, and
 * `Resolution` has parsed all three since the locator was built without a single
 * write path using them. One scan names the line, fills the lot and fills the
 * expiry — which is the difference between food traceability being a discipline
 * somebody keeps up and it being a consequence of scanning.
 *
 * # Everything the write path refuses on is said before the press
 *
 * A required lot, an owner nobody named, a count that is not a count. The
 * disposition **refuses** a line with no lot where the policy wants one, and
 * finding that out from a 400 means the pallet is already broken down.
 */
export function Receiving({ bench }: { bench: ReceivingBench }) {
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
        <Face>
          <Stack gap={3}>
            <Row gap={3} align="baseline" wrap>
              <Soft>{bench.bay ? "Receiving at" : "Unloading at"}</Soft>
              {bench.bay && <Code>{bench.bay.code}</Code>}
              <Trailing>
                {bench.delivery && (
                  <>
                    <Pill tone="good">delivery open</Pill>
                    <Key size="small" disabled={bench.busy} onClick={bench.closeDelivery}>
                      Close delivery
                    </Key>
                  </>
                )}
              </Trailing>
            </Row>
            <ScanInput
              label={bench.bay ? "Scan the carton" : "Scan the dock or bin"}
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
          </Stack>
        </Face>

        {bench.problem && <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>}
        {bench.landed && <Landed bench={bench} />}
        {bench.counting && <Counting bench={bench} line={bench.counting} />}

        <Face pad={false} as="section">
          <Band count={lines.length}>Expected</Band>
          <FaceWell>
            {lines.length === 0 ? (
              <EmptySlot
                label="Nothing expected"
                note="All expected deliveries have been received."
              />
            ) : (
              <Records>
                {lines.map((line) => (
                  <Line
                    key={line.expected_supply_id}
                    line={line}
                    bench={bench}
                    counting={bench.counting?.expected_supply_id === line.expected_supply_id}
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

/** What the last line did — accepted, refused, or accepted with something said. */
function Landed({ bench }: { bench: ReceivingBench }) {
  const it = bench.landed;
  if (!it) return null;
  return (
    <Notice
      kind={it.accepted ? "recorded" : "finding"}
      onDismiss={bench.dismiss}
      notes={it.warnings.map((w) => (
        <Faint key={w}>{w}</Faint>
      ))}
    >
      {it.accepted
        ? `${it.entered} of ${it.code} — ${it.quantity} in.`
        : /* **A refusal is not an error, and the difference matters here.** The
             goods are on the dock either way; what did not happen is the
             record. Saying "failed" would send somebody looking for a fault in
             the system rather than for the lot code on the carton. */
          `${it.code} not received: this item needs a lot.`}
    </Notice>
  );
}

/** The line being counted: what is on the label, and what it comes to. */
function Counting({ bench, line }: { bench: ReceivingBench; line: ExpectedLine }) {
  const over = bench.base !== null ? Math.max(0, bench.base - line.outstanding) : 0;
  // What one of whatever is being counted has weighed before. Null is the
  // ordinary case and the row below simply does not appear.
  const perUnit =
    line.levels.find((l) => l.level === bench.level)?.baseline ?? null;

  return (
    <Face>
      <Stack gap={3}>
        <Row gap={3} align="baseline" wrap>
          <Code>{line.item_code}</Code>
          {line.order_number && <Faint>{line.order_number}</Faint>}
          <Trailing>
            <Key size="small" disabled={bench.busy} onClick={bench.release}>
              Not this
            </Key>
          </Trailing>
        </Row>

        <Row gap={3} align="end" wrap>
          <Field
            label="Counted"
            width="inline"
            value={bench.entered}
            onChange={bench.typeEntered}
            disabled={bench.busy}
          />
          {/* **Counted in what was counted.** D92 and Q173: the receiver counts
              cartons, and only the levels this item has a config for are
              offered — a level with no factor behind it would be a question the
              server cannot answer. */}
          {line.levels.length > 1 && (
            <Chooser
              label="of"
              value={bench.level}
              onChange={bench.chooseLevel}
              options={line.levels.map((l) => ({ value: l.level, label: l.level }))}
            />
          )}
          {bench.base !== null && (
            <>
              <Steel>{bench.base}</Steel>
              <Faint>units</Faint>
            </>
          )}
          <Spacer />
          {/* More than promised. Whether that is a finding is the tolerance's
              decision and the tolerance is the server's, so this says the fact
              and not the judgement. */}
          {over > 0 && <Pill tone="state">{`${over} more than expected`}</Pill>}
        </Row>

        {/* **THE THIRD WITNESS.** The paperwork advised, the receiver counted,
            and those two can only disagree — neither is evidence about the
            other. A scale is an observation belonging to neither, and it is
            what makes a count of 46 defensible weeks later against a supplier's
            48.

            **Shown only when a baseline exists**, which today is seldom: this
            divides by what somebody actually put on an instrument, and
            `revalidation` records that 115 of 116 weights on file were
            transcribed off a sheet. A field offering to divide by nothing is a
            field that teaches people to ignore it.

            **Nothing here is written.** Dividing a dock weight by the count and
            storing the result as what a carton weighs would put the count's own
            error into the baseline, and the baseline would then agree with the
            next count for the same reason it was wrong — see
            `@app/measurement/baseline`. The figure that grows this baseline
            comes off the weighing bench, where one carton goes on the scale by
            itself and the divisor is 1. */}
        {perUnit && (
          <Row gap={3} align="end" wrap>
            <Field
              /* **What has to be on the scale.** The arithmetic divides by what
                 one unit weighs, so a supplier's pallet under the goods is a
                 board the baseline knows nothing about — 25 kg over 400 g
                 cartons reads as sixty-two cartons that are not there. There is
                 no tare here to subtract, so the label has to carry it. */
              label="Weighed (goods only)"
              suffix="kg"
              width="inline"
              value={bench.weighed}
              onChange={bench.typeWeighed}
              disabled={bench.busy}
            />
            {bench.scale && <Steel>{bench.scale.sentence}</Steel>}
            <Spacer />
            <Faint>
              {grams(perUnit.grams)} kg a {bench.level}, {provenance(perUnit)}
            </Faint>
          </Row>
        )}

        {/* The lot, which one scan usually fills. Shown whenever the policy
            wants one, and offered even when it does not — a receiver holding a
            dated carton should not have to be asked twice. */}
        <Row gap={3} align="end" wrap>
          <Field
            label={line.requires_lot ? "Lot (required)" : "Lot"}
            value={bench.lotCode}
            onChange={bench.typeLot}
            numeric={false}
            disabled={bench.busy}
          />
          <Field
            label="Expiry"
            value={bench.lotExpiry}
            onChange={bench.typeExpiry}
            numeric={false}
            disabled={bench.busy}
          />
        </Row>

        {/* Whose the goods are, when the promise did not say. The parties
            already holding this item here, offered rather than defaulted. */}
        {!line.owner_id && line.owners.length > 0 && (
          <Row gap={3} align="baseline" wrap>
            <Faint>Owner</Faint>
            {line.owners.map((o) => (
              <Key
                key={o.owner_id}
                size="small"
                live={bench.owner === o.owner_id}
                disabled={bench.busy}
                onClick={() => bench.chooseOwner(o.owner_id)}
              >
                {o.name}
              </Key>
            ))}
          </Row>
        )}

        <Row gap={3} align="center" wrap>
          {bench.missing ? <Faint>{bench.missing}</Faint> : <Faint>Ready.</Faint>}
          <Trailing>
            {/* **Not a receipt of nothing.** A zero-quantity movement would be a
                lie; closing the promise short says the rest is not coming and
                puts the shortfall where a supplier conversation can find it. */}
            <Key size="small" disabled={bench.busy} onClick={() => void bench.closeShort()}>
              Close short
            </Key>
            <Key
              live
              disabled={bench.busy || bench.missing !== null}
              onClick={() => void bench.receive()}
            >
              Receive
            </Key>
          </Trailing>
        </Row>
      </Stack>
    </Face>
  );
}

function Line({
  line,
  bench,
  counting,
}: {
  line: ExpectedLine;
  bench: ReceivingBench;
  counting: boolean;
}) {
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
      name={<Code>{line.item_code}</Code>}
      tags={
        <>
          {line.supplier && <Faint>{line.supplier}</Faint>}
          {/* Said on the row, so it is known before the pallet is opened. */}
          {line.requires_lot && <Pill tone="state">by lot</Pill>}
        </>
      }
      facts={
        <>
          <Fact value={line.outstanding} label="outstanding" />
          {line.received > 0 && <Fact value={line.received} label="received" />}
        </>
      }
      note={line.description && <Faint>{line.description}</Faint>}
      meta={
        <>
          {line.order_number && <Faint>{line.order_number}</Faint>}
          {counting && <Pill tone="good">counting</Pill>}
        </>
      }
      action={
        <Key
          size="small"
          disabled={bench.busy || !bench.bay || counting}
          onClick={() => bench.choose(line)}
        >
          {counting ? "Counting" : "This one"}
        </Key>
      }
    />
  );
}

/** The dock: what is expected, and whether a delivery is open. */
export function ReceivingDock({ bench }: { bench: ReceivingBench }) {
  const site = bench.status.kind === "ready" ? bench.status.screen.site : "—";
  const count = bench.status.kind === "ready" ? bench.status.screen.lines.length : 0;
  return (
    <Row gap={3} align="center">
      <Pill tone="quiet">{site}</Pill>
      {count > 0 && (
        <>
          <Steel>{count}</Steel>
          <Faint>expected</Faint>
        </>
      )}
      {bench.bay && <Faint>{`into ${bench.bay.code}`}</Faint>}
      <Trailing>
        <Key size="small" disabled={bench.busy} onClick={() => void bench.refresh()}>
          Refresh
        </Key>
      </Trailing>
    </Row>
  );
}

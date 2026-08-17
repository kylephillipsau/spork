import { useState } from "react";
import {
  Action,
  Band,
  Chooser,
  Code,
  EmptySlot,
  Face,
  FaceWell,
  Faint,
  Field,
  Key,
  Lamp,
  Link,
  Notice,
  Num,
  NumHead,
  Panel,
  Pill,
  Readout,
  Row,
  Soft,
  Spacer,
  Stack,
  Steel,
  Table,
  Tag,
  grams,
} from "@design/index";
import type { BenchScreen, CartonSummary, ExpectedWeight } from "@domain/types";
import { agreement, provenance } from "@app/measurement/baseline";
import type { PackBench as PackBenchState } from "./usePackBench";

/**
 * Stages 3 to 8 of the recorded process, on one screen.
 *
 * **Stage 3 disappears and stage 4 is the whole of the work.** The
 * walkthrough's stage 3 is three fields — Picked By, Packed By, Status — which
 * it calls *"keystrokes, not judgement"*; every one is now a consequence of who
 * signed on and what they did, so there is nothing to type. Stage 4 is *"the
 * only stage with genuinely variable, physically-measured input"*, and it is
 * two boxes.
 *
 * Takes state, emits primitives. No fetching here, so the whole screen renders
 * from a fixture.
 */
export function PackBench({ bench }: { bench: PackBenchState }) {
  if (bench.status.kind === "loading") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Faint>Loading…</Faint>
        </Face>
      </Panel>
    );
  }

  if (bench.status.kind === "failed") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Row gap={3}>
            <Lamp kind="finding" />
            <span>{bench.status.message}</span>
          </Row>
        </Face>
      </Panel>
    );
  }

  const screen = bench.status.screen;

  /**
   * **One panel, several windows.** The bench is one instrument, and a fascia
   * with a row of readouts seated into it is what one looks like. Every section
   * used to carry its own metal, its own bezel and its own cast shadow, so five
   * slabs floated at five heights over a ground none of them touched — which
   * reads as five instruments that happen to be stacked, not as a bench.
   *
   * The metal is continuous now and the gaps between faces are that metal
   * showing through, which is also the only honest way to draw it: a chassis is
   * one piece of material.
   */
  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <Stack gap={4}>
            <Gate screen={screen} />
            <Lines screen={screen} bench={bench} />
          </Stack>
        </Face>

        <NewCarton screen={screen} bench={bench} />

        {screen.cartons.map((carton) => (
          <Carton key={carton.id} carton={carton} bench={bench} />
        ))}

        {bench.problem && (
          <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>
        )}
      </Stack>
    </Panel>
  );
}

/**
 * The stage-2 check, as a header rather than as a screen.
 *
 * The walkthrough opens a record to read a status and a location before it may
 * proceed. Both are here, above the work, and neither is a navigation.
 */
function Gate({ screen }: { screen: BenchScreen }) {
  const left = screen.lines.reduce(
    (total, line) => total + Math.max(0, line.remaining),
    0,
  );
  return (
    <FaceWell>
      <Row gap={4} align="baseline" wrap>
        <Code>{screen.reference}</Code>
        <Soft>{screen.customer}</Soft>
        <Code>{screen.order_reference}</Code>
        <Spacer />
        <Tag>{screen.site}</Tag>
        <Pill tone={left > 0 ? "state" : "good"}>
          {left > 0 ? `${left} to pack` : "Nothing left to pack"}
        </Pill>
      </Row>
    </FaceWell>
  );
}

/**
 * What is left, and where it is.
 *
 * **The cell is a choice because nothing else can make it yet.** Question 26 —
 * who allocates, and when — is deferred against building the allocator, so
 * `/allocations` is directed only. When 26 is answered this becomes a default.
 */
function Lines({
  screen,
  bench,
}: {
  screen: BenchScreen;
  bench: PackBenchState;
}) {
  const [cells, setCells] = useState<Record<string, string>>({});
  const [quantities, setQuantities] = useState<Record<string, string>>({});

  return (
    <Stack gap={3}>
      <Band count={screen.lines.length}>Remaining</Band>
      <Table
        min="wide"
        head={
          <tr>
            <th>Item</th>
            <th>From</th>
            <NumHead>Left</NumHead>
            <NumHead>Add</NumHead>
          </tr>
        }
      >
        {screen.lines.map((line) => {
          const chosenCell =
            cells[line.line_id] ?? line.cells[0]?.stock_id ?? "";
          const quantity = quantities[line.line_id] ?? String(line.remaining);
          // **The control is on every row that has anywhere to pick from.**
          // It used to appear only where something was still outstanding, so a
          // job half packed showed the affordance on one line and bare space on
          // the others — which reads as an unfinished interface rather than as
          // finished work. It is present and disabled instead, which says the
          // same thing about the line without saying anything about the screen.
          const hasCells = line.cells.length > 0;
          const canAdd = line.remaining > 0 && hasCells && !!bench.openCarton;
          const add = () =>
            void bench.addToCarton({
              line: line.line_id,
              stock: chosenCell,
              quantity: Number.parseInt(quantity, 10),
            });

          return (
            <tr key={line.line_id}>
              <td>
                <Code>{line.item_code}</Code>
                {line.description && (
                  <div>
                    <Faint>{line.description}</Faint>
                  </div>
                )}
              </td>
              <td>
                {line.cells.length === 0 ? (
                  <Faint>no stock at this site</Faint>
                ) : (
                  <Chooser
                    label="Pick from"
                    quiet
                    value={chosenCell}
                    onChange={(next) =>
                      setCells((prev) => ({ ...prev, [line.line_id]: next }))
                    }
                    options={line.cells.map((cell) => ({
                      value: cell.stock_id,
                      label:
                        `${cell.location} · ${cell.available} free` +
                        (cell.lot ? ` · ${cell.lot}` : ""),
                    }))}
                  />
                )}
              </td>
              <Num>
                {line.remaining > 0 ? (
                  <Steel>{line.remaining}</Steel>
                ) : (
                  <Faint>0</Faint>
                )}
              </Num>
              <Action>
                {hasCells && (
                  <Row gap={2} align="center">
                    <Spacer />
                    <Field
                      label="Quantity"
                      quiet
                      width="inline"
                      value={quantity}
                      disabled={!canAdd || bench.busy}
                      onSubmit={add}
                      onChange={(next) =>
                        setQuantities((prev) => ({
                          ...prev,
                          [line.line_id]: next,
                        }))
                      }
                    />
                    <Key
                      size="small"
                      disabled={!canAdd || bench.busy}
                      onClick={add}
                    >
                      Add
                    </Key>
                  </Row>
                )}
              </Action>
            </tr>
          );
        })}
      </Table>
    </Stack>
  );
}

function NewCarton({
  screen,
  bench,
}: {
  screen: BenchScreen;
  bench: PackBenchState;
}) {
  const [preset, setPreset] = useState(screen.presets[0]?.id ?? "");
  return (
    <Face>
      <Row gap={4} align="end" wrap>
        <Chooser
          label="Preset"
          value={preset}
          onChange={setPreset}
          options={screen.presets.map((option) => ({
            value: option.id,
            label: option.name,
          }))}
        />
        {/* **Lit only when it is the thing to do next.** With a carton open,
            the act that advances the bench is sealing that one; starting a
            second is available and is not the primary. `live` was
            unconditional here and on every carton's Seal, which put three lit
            keys on the screen — the exact fault the despatch bench recorded
            fixing and this one kept. */}
        <Key
          live={!bench.openCarton}
          disabled={bench.busy || !preset}
          onClick={() => void bench.startCarton(preset)}
        >
          Start a carton
        </Key>
        <Spacer />
        {!bench.openCarton && <Faint>No carton is open.</Faint>}
      </Row>
    </Face>
  );
}

/**
 * Stage 4.
 *
 * **The footprint is the preset's and the height is not.** A carton is scored
 * at the corners and folded down to suit what is in it — same length, same
 * width, whatever height the goods came to. So the two numbers that cannot
 * change are shown, and the one that can is a field seeded with what the preset
 * says.
 *
 * Seeding a field is not recording a measurement: nothing is written unless the
 * height differs from the preset's, because a height that equals it was never
 * observed.
 */
/**
 * What the carton should weigh, and what that claim rests on.
 *
 * **Steel, not amber.** A baseline is state — what this thing has weighed
 * before — and amber is reserved for findings (D115). Nothing here is a
 * finding: a gap between the scale and the expectation is something for the
 * packer to look at, not a defect the system has decided upon.
 *
 * The caption is never optional. `grams` on its own reads as settled fact, and
 * the whole reason the server sends `n` and `borrowed` is that it frequently is
 * not one — see `expectation.ts`.
 */
function Expectation({ expected }: { expected: ExpectedWeight }) {
  const gap = agreement(expected);
  return (
    <Stack gap={1}>
      <Readout
        label="Expected"
        value={grams(expected.grams)}
        unit="kg"
        size="small"
      />
      <Steel>{provenance(expected)}</Steel>
      {gap && <Steel>{gap}</Steel>}
    </Stack>
  );
}

function Carton({
  carton,
  bench,
}: {
  carton: CartonSummary;
  bench: PackBenchState;
}) {
  const statedHeight = carton.stated_size?.height_mm ?? null;
  const [weight, setWeight] = useState(
    carton.gross_weight_g === null ? "" : grams(carton.gross_weight_g),
  );
  const [height, setHeight] = useState(
    String(carton.height_mm ?? statedHeight ?? ""),
  );

  const cut =
    height.trim() !== "" && height.trim() !== String(statedHeight ?? "");

  return (
    <Face pad={false} as="section">
      <Band count={carton.contents.length}>
        Carton {carton.sequence}
        {carton.package_type ? ` · ${carton.package_type}` : ""}
        {carton.sealed ? " · sealed" : ""}
      </Band>

      <FaceWell>
        <Stack gap={3}>
          {carton.contents.length === 0 ? (
            <EmptySlot
              label={carton.package_type ?? "Container"}
              note="Empty."
              {...(carton.stated_size ? { size: carton.stated_size } : {})}
            />
          ) : (
            <Table
              min="wide"
              head={
                <tr>
                  <th>Item</th>
                  <th>Lot</th>
                  <NumHead>Units</NumHead>
                  {!carton.sealed && <NumHead>{""}</NumHead>}
                </tr>
              }
            >
              {carton.contents.map((row) => (
                <tr key={`${row.item_code}-${row.lot_code ?? ""}`}>
                  <td>
                    <Code>{row.item_code}</Code>
                  </td>
                  <td>
                    {row.lot_code ? (
                      <Code>{row.lot_code}</Code>
                    ) : (
                      <Faint>—</Faint>
                    )}
                  </td>
                  <Num>{row.quantity}</Num>
                  {!carton.sealed && (
                    <Action>
                      <Key
                        size="small"
                        disabled={bench.busy}
                        onClick={() =>
                          void bench.takeOut({
                            picks: row.picks,
                            quantity: row.quantity,
                          })
                        }
                      >
                        Take out
                      </Key>
                    </Action>
                  )}
                </tr>
              ))}
            </Table>
          )}

          {carton.sealed ? (
            <Row gap={5} wrap align="end">
              {/* **Stage 5, and the one thing this screen was missing.** A
                  sealed carton gets an A4 sheet folded onto the pallet, which
                  D113 keeps server-rendered because it is genuinely a document
                  rather than a screen. The maud bench offered this and the
                  React one did not, which is the parity gap `handover.md` warns
                  a port quietly opens.

                  Not a `Link`: `/print` belongs to the server, so the router's
                  interceptor lets the browser have it — and this is one of the
                  few places a full page load is the right answer, because what
                  comes back is a document to print. */}
              <Link href={`/print/packing-list/${carton.id}`}>Packing list</Link>
              <Readout
                label="Gross"
                value={
                  carton.gross_weight_g === null
                    ? "—"
                    : grams(carton.gross_weight_g)
                }
                {...(carton.gross_weight_g === null ? {} : { unit: "kg" })}
                size="large"
              />
              {carton.expected && <Expectation expected={carton.expected} />}
              {carton.height_mm !== null && (
                <Readout
                  label="Height"
                  value={String(carton.height_mm)}
                  unit="mm"
                  size="small"
                />
              )}
              {carton.stated_size && (
                <Readout
                  label="Footprint"
                  value={`${carton.stated_size.length_mm} × ${carton.stated_size.width_mm}`}
                  unit="mm"
                  size="small"
                />
              )}
            </Row>
          ) : (
            <Stack gap={5}>
              <Row gap={5} wrap align="end">
                <Field
                  label="Weight"
                  suffix="kg"
                  width="measure"
                  value={weight}
                  onChange={setWeight}
                />
                <Field
                  label="Height"
                  suffix="mm"
                  width="measure"
                  value={height}
                  onChange={setHeight}
                />
                <Spacer />
                {/* **Before the figure is typed, not after it is judged.** The
                    packer is standing at the scale; what the carton has weighed
                    before is worth more to them now than a verdict on what they
                    have already entered would be. */}
                {carton.expected && <Expectation expected={carton.expected} />}
                {carton.stated_size && (
                  <Readout
                    label="Footprint"
                    value={`${carton.stated_size.length_mm} × ${carton.stated_size.width_mm}`}
                    unit="mm"
                    size="small"
                  />
                )}
              </Row>

              <Row gap={3} wrap>
                <Key
                  disabled={bench.busy}
                  onClick={() =>
                    void bench.measure({
                      carton: carton.id,
                      ...(weight.trim() ? { weightKg: weight.trim() } : {}),
                      ...(cut ? { heightMm: height.trim() } : {}),
                    })
                  }
                >
                  Record
                </Key>
                {/* One carton is open at a time, and only its Seal is lit:
                    two unsealed cartons on the bench would otherwise claim two
                    primary actions. */}
                <Key
                  live={carton.id === bench.openCarton}
                  disabled={bench.busy}
                  onClick={() => void bench.seal(carton.id)}
                >
                  Seal
                </Key>
                <Spacer />
                {carton.contents.length === 0 && (
                  <Key
                    size="small"
                    disabled={bench.busy}
                    onClick={() => void bench.discard(carton.id)}
                  >
                    Discard
                  </Key>
                )}
              </Row>
            </Stack>
          )}
        </Stack>
      </FaceWell>
    </Face>
  );
}

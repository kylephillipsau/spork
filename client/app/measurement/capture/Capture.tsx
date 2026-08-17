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
  Lamp,
  Notice,
  Panel,
  Pill,
  Readout,
  Record,
  Records,
  Row,
  ScanInput,
  Soft,
  Spacer,
  Stack,
  Steel,
  Tabs,
  Tag,
  centimetres,
  grams,
} from "@design/index";
import type { CaptureScreen, CaptureSubject } from "@domain/types";
import {
  FACES,
  PRESENTATIONS,
  presentationNeeded,
  presentationOffered,
  type CaptureBench,
  type Face as FaceName,
} from "./useCapture";
import { measurementsOf } from "./figures";
import styles from "./capture.module.css";

/**
 * Scan → weigh → length, width, height → seven faces.
 *
 * The first screen on the handheld, and the first thing to drive the
 * photograph model D132 built. Three stages, and the operator is in exactly
 * one of them: pick a subject, key the figures, take the pictures.
 *
 * **The stages are not tabs.** Photographs are unavailable until the figures
 * have landed, because D133 makes the session one act and an image hangs off
 * the event that act produced — there is no event to attach to until then. The
 * screen shows that as a sequence rather than as a disabled control with a
 * tooltip, because the sequence is the truth about the model.
 */
export function Capture({ bench }: { bench: CaptureBench }) {
  if (bench.stage.kind === "figures") {
    return <Figures bench={bench} subject={bench.stage.subject} />;
  }
  if (bench.stage.kind === "photographs") {
    return <Photographs bench={bench} subject={bench.stage.subject} />;
  }

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

  return <Worklist bench={bench} screen={bench.status.screen} />;
}

/**
 * `260101` as `26-01-01`.
 *
 * **Punctuation only, and deliberately not a date.** AI 17 carries a two-digit
 * year, and turning it into one requires a century rule — GS1's is a
 * fifty-year window around today — which the server declined to apply for the
 * same reason: inventing one silently is worse than handing back the digits.
 * Re-spacing them claims nothing and is the difference between a readable code
 * and a wall of numerals.
 */
function yymmdd(raw: string): string {
  return /^\d{6}$/.test(raw) ? `${raw.slice(0, 2)}-${raw.slice(2, 4)}-${raw.slice(4)}` : raw;
}

/**
 * What the last scan turned out to be.
 *
 * **Four outcomes, and four different things to say.** The temptation is two —
 * found and not found — and it is wrong in the place that costs most: telling
 * an operator "no such item" when the truth is "that is two items" is how a
 * scan ends up recorded against the wrong one. So an ambiguous scan draws its
 * candidates and says it will not choose between them, a well-formed identifier
 * nobody holds says so plainly, and a smudge says something different again.
 */
function Scanned({ bench }: { bench: CaptureBench }) {
  const found = bench.scan.found;
  if (!found) return null;

  if (found.outcome === "resolved" && found.subjects.length === 1) {
    const subject = found.subjects[0];
    if (!subject) return null;
    return (
      <Face pad={false}>
        <Band>Scanned</Band>
        <FaceWell>
          <Stack gap={3}>
            <Row gap={3} align="baseline" wrap>
              <Code>{subject.code}</Code>
              {subject.description && <Faint>{subject.description}</Faint>}
              <Spacer />
              <Pill tone="good">{subject.via.replace(/_/g, " ")}</Pill>
            </Row>
            {/* One scan can answer three questions: a GS1-128 carton label
                carries the lot and the expiry beside the item. Shown because
                the operator can see whether the label matches the box.

                **`Code`, not `Readout`.** A `Readout` runs `groupDigits`,
                which is right for a quantity and wrong for an identifier — it
                rendered the expiry `260101` as "260 101", a date code
                punctuated as though it were a number. `Code` is the primitive
                for anything read off a label, which is exactly what these
                are. */}
            {(found.lot || found.expiry) && (
              <Row gap={4} wrap align="baseline">
                {found.lot && (
                  <>
                    <Faint>Lot</Faint>
                    <Code>{found.lot}</Code>
                  </>
                )}
                {found.expiry && (
                  <>
                    <Faint>Expiry</Faint>
                    <Code>{yymmdd(found.expiry)}</Code>
                  </>
                )}
              </Row>
            )}
            {/* **The worklist's own subjects**, not a second enumeration. A
                scan cannot open a session the worklist would not list, which
                is what keeps a styled variant from being captured twice. */}
            <Records>
              {subject.capture.map((target) => (
                <Record
                  key={`${target.item_id ?? target.item_style_id ?? target.item_part_id}/${target.packaging_level ?? "part"}`}
                  name={<Level subject={target} />}
                  tags={target.item_style_id && <Faint>as {target.code}</Faint>}
                  meta={
                    target.wants.length === 0 ? (
                      <Pill tone="good">complete</Pill>
                    ) : (
                      target.wants.map((w) => (
                        <Pill key={w} tone="state">
                          {w}
                        </Pill>
                      ))
                    )
                  }
                  action={
                    <Key size="small" disabled={bench.busy} onClick={() => bench.choose(target)}>
                      Capture
                    </Key>
                  }
                />
              ))}
            </Records>
          </Stack>
        </FaceWell>
      </Face>
    );
  }

  const said =
    found.outcome === "identifier_ambiguous"
      ? // **Describes, and does not instruct.** It said "Pick which" and then
        // drew rows with no key on them: the server fills `capture` only when
        // exactly one subject survives, so there is nothing here to pick with.
        // Telling an operator to do something the screen does not afford is
        // the class of lie this repository keeps writing tests against.
        // Resolving an ambiguity per row is a real design question — whose
        // answer is probably `issuer_party_id` and a supplier on the screen —
        // and it is not answered by quietly adding a key.
        "That code matches more than one thing."
      : found.outcome === "identifier_unknown"
        ? "Nothing here holds that code."
        : "That is not a code.";

  return (
    <Face>
      <Stack gap={3}>
        <Row gap={3} wrap>
          <Lamp kind="finding" />
          <span>{said}</span>
          <Spacer />
          <Code>{found.scanned}</Code>
          <Key size="small" onClick={bench.clearScan}>
            Clear
          </Key>
        </Row>
        {/* An ambiguous scan draws what it could be rather than choosing.
            Both candidates are on the worklist below, which is where either
            one gets captured until a scan can say which supplier's code it
            was reading. */}
        {found.subjects.length > 1 && (
          <Stack gap={2}>
            <Faint>Both are on the worklist below.</Faint>
            <Records>
              {found.subjects.map((s) => (
                <Record
                  key={`${s.kind}/${s.id}`}
                  lead={<Pill tone="quiet">{s.kind}</Pill>}
                  name={<Code>{s.code}</Code>}
                  note={s.description && <Faint>{s.description}</Faint>}
                  meta={<Faint>{s.via.replace(/_/g, " ")}</Faint>}
                />
              ))}
            </Records>
          </Stack>
        )}
      </Stack>
    </Face>
  );
}

/**
 * The action in the reachable third, which changes with the stage.
 *
 * Rendered into `FloorShell`'s dock rather than at the foot of the screen, so
 * a long worklist cannot scroll it out of reach.
 */
export function CaptureDock({ bench }: { bench: CaptureBench }) {
  if (bench.stage.kind === "figures") {
    const ready = measurementsOf(bench.figures).length > 0;
    return (
      <Stack gap={2}>
        {/* Only when there is nothing to record. With figures keyed, the
            `Record` key is the affordance and a sentence restating it is one
            more thing to read every time. */}
        {!ready && <Faint>Enter at least one measurement.</Faint>}
        <Row gap={3}>
          <Key onClick={bench.leave}>Back</Key>
          <Spacer />
          <Key live block disabled={bench.busy || !ready} onClick={() => void bench.record()}>
            Record
          </Key>
        </Row>
      </Stack>
    );
  }

  if (bench.stage.kind === "photographs") {
    return (
      <Stack gap={2}>
        {/* Stacked rather than set beside the key. At handheld width the count
            and a full-width key share a line by overlapping, which is what the
            gate drew the first time this was a Row. */}
        <Faint>
          {bench.taken.length} of {FACES.length} faces photographed. The figures
          are already recorded.
        </Faint>
        <Key live block disabled={bench.busy} onClick={() => void bench.finish()}>
          Done
        </Key>
      </Stack>
    );
  }

  return null;
}

/**
 * What wants capturing, in the order somebody would walk it.
 *
 * **One list, because a walk has one order.** This drew three — never recorded,
 * part recorded, never measured or overdue — which is the right shape for
 * triage at a desk and the wrong one for a lap of the warehouse: an operator
 * working three lists in bin order walks past every shelf three times. The
 * server sorts by bin and the classification moved onto the row, where `wants`
 * says what this trip is for and `because` says why the row is here.
 *
 * The server phrases the judgement and the client draws it (D114), so `because`
 * arrives as a word rather than being computed here from the figures beside it.
 */
function Worklist({ bench, screen }: { bench: CaptureBench; screen: CaptureScreen }) {
  const total = screen.walk.length;

  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <FaceWell>
            <Stack gap={3}>
              <Row gap={3} align="baseline" wrap>
                <Soft>Capture</Soft>
                <Spacer />
                <Tag>{screen.site}</Tag>
                <Pill tone={total > 0 ? "state" : "good"}>
                  {total > 0 ? `${total} to capture` : "Nothing waiting"}
                </Pill>
              </Row>
              {/* **The fastest path to a record is the barcode on the thing in
                  your hand** (D111). It sits above the worklist rather than in
                  the chrome because this screen claims scan focus and the
                  chrome never steals it (D117) — and because a worklist is what
                  you read when you have nothing in your hand. */}
              <ScanInput
                label="Scan"
                value={bench.scan.typed}
                onChange={bench.typeScan}
                onScan={(v) => void bench.lookUp(v)}
                busy={bench.busy}
                refocus={bench.scan.refocus}
                hint="Barcode, GTIN, or type a code"
              />
            </Stack>
          </FaceWell>
        </Face>

        <Scanned bench={bench} />

        <List
          bench={bench}
          title="The walk"
          subjects={screen.walk}
          empty="Nothing at this site wants measuring."
        />
      </Stack>
    </Panel>
  );
}

function List({
  bench,
  title,
  subjects,
  empty,
}: {
  bench: CaptureBench;
  title: string;
  subjects: CaptureSubject[];
  empty: string;
}) {
  return (
    <Face pad={false} as="section">
      <Band count={subjects.length}>{title}</Band>
      <FaceWell>
        {subjects.length === 0 ? (
          <EmptySlot label="Nothing here" note={empty} />
        ) : (
          <Records>
            {subjects.map((subject) => (
              <Subject
                key={`${subject.item_id ?? subject.item_style_id ?? subject.item_part_id}/${subject.packaging_level ?? "part"}`}
                bench={bench}
                subject={subject}
              />
            ))}
          </Records>
        )}
      </FaceWell>
    </Face>
  );
}

/**
 * Which subject of this item, said in one pill.
 *
 * **A part has no packaging level and gets its own name instead.** The read
 * sends `packaging_level: null` for a part precisely so that nothing can post
 * one, and a screen that drew the null as an empty pill would be showing the
 * absence rather than the thing — the part's label is what tells two rows
 * under one code apart. D139.
 */
function Level({ subject }: { subject: CaptureSubject }) {
  if (subject.item_part_id) {
    return <Pill tone="state">{subject.part_label ?? "part"}</Pill>;
  }
  return <Pill tone="quiet">{subject.packaging_level}</Pill>;
}

function Subject({ bench, subject }: { bench: CaptureBench; subject: CaptureSubject }) {
  return (
    <Record
      /* **The bin leads, because it is what you are looking for.** On a walk
         the code identifies the box once you are at the shelf; the bin is what
         gets you there, and it is the sort key, so a row whose first word is
         the code makes the operator read every row to find the order they are
         already walking in. */
      lead={<Tag>{subject.location_code ?? "no bin"}</Tag>}
      name={<Code>{subject.code}</Code>}
      tags={
        <>
          <Level subject={subject} />
          {/* **Whose figure this is.** D108: a screen that cannot tell an
              inherited number from an own one reports a number nobody took
              against this code as though somebody had. */}
          {subject.source === "style" && subject.style_code && (
            <Faint>from {subject.style_code}</Faint>
          )}
        </>
      }
      facts={
        <>
          {/* How much is on that shelf. The printed sheet carries it and it is
              worth carrying: a bin the operator expects to be full and is not
              is a finding rather than a measurement. */}
          <Fact value={subject.soh} label="on hand" />
          {/* **Still drawn, though it no longer sorts.** Demand decided the
              order when this was three lists read at a desk, and bin order
              replaced it — but why a row is urgent is a different question
              from where it is. */}
          {subject.demand > 0 && <Fact value={subject.demand} label="on order" />}
        </>
      }
      note={subject.description && <Faint>{subject.description}</Faint>}
      meta={
        <>
          {subject.wants.map((want) => (
            <Pill key={want} tone="state">
              {want}
            </Pill>
          ))}
          {/* Why this row is on the walk at all, in the server's own word.
              Finer than the list it replaces: `overdue` and `never-measured`
              were one list and are not one reason. */}
          <Faint>{subject.because}</Faint>
        </>
      }
      action={
        <Key size="small" disabled={bench.busy} onClick={() => bench.choose(subject)}>
          Capture
        </Key>
      }
    />
  );
}

/** Stage two: a box in hand, and four numbers. */
function Figures({ bench, subject }: { bench: CaptureBench; subject: CaptureSubject }) {
  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <SubjectHead subject={subject} />

        <Face pad={false}>
          <Band>Measurements</Band>
          <FaceWell>
            <Stack gap={4}>
              <Field
                label="Gross weight"
                suffix="kg"
                width="measure"
                value={bench.figures.weight}
                onChange={(next) => bench.type("weight", next)}
              />
              {/* Three fields rather than one "LxWxH": the writer stores three
                  metrics and a combined box would have to be split on the way
                  in and could not report which of the three was mistyped. */}
              {bench.figures.noDimensions ? (
                <Row gap={3} wrap align="end">
                  <Soft>No dimensions.</Soft>
                  <Spacer />
                  <Key size="small" onClick={bench.toggleNoDimensions}>
                    Measure after all
                  </Key>
                </Row>
              ) : (
                <Row gap={3} wrap align="end">
                  {/* Centimetres, matching the tape and the sheet this
                      replaces. The value and its unit both travel to the
                      writer, which converts — nothing is scaled here. */}
                  <Field
                    label="Length"
                    suffix="cm"
                    width="inline"
                    value={bench.figures.length}
                    onChange={(next) => bench.type("length", next)}
                  />
                  <Field
                    label="Width"
                    suffix="cm"
                    width="inline"
                    value={bench.figures.width}
                    onChange={(next) => bench.type("width", next)}
                  />
                  <Field
                    label="Height"
                    suffix="cm"
                    width="inline"
                    value={bench.figures.height}
                    onChange={(next) => bench.type("height", next)}
                  />
                </Row>
              )}

              {/* **The answer that is not a number**, offered exactly where it
                  can be true. A carton has a box — that is what a carton is —
                  so the question belongs to a single loose thing and to a part.
                  D138. */}
              {presentationOffered(subject) && !bench.figures.noDimensions && (
                <Row gap={3} wrap align="end">
                  <Spacer />
                  <Key size="small" onClick={bench.toggleNoDimensions}>
                    It has no dimensions
                  </Key>
                </Row>
              )}
            </Stack>
          </FaceWell>
        </Face>

        {presentationOffered(subject) && !bench.figures.noDimensions && (
          <Arrangement bench={bench} subject={subject} />
        )}

        {/* **Measure the parts, not this.** A set whose parts go into a box
            independently has no box of its own, and the count is what tells the
            operator that before they try to find one. D139. */}
        {subject.parts > 0 && (
          <Face>
            <Row gap={3} wrap align="baseline">
              <Steel>{subject.parts}</Steel>
              <Soft>parts are listed separately. Measure each one.</Soft>
            </Row>
          </Face>
        )}

        <Barcodes bench={bench} subject={subject} />

        <Held subject={subject} />
        <Problem bench={bench} />
      </Stack>
    </Panel>
  );
}

/**
 * What this box answers to, and saying that a label is on it (D164).
 *
 * **Here rather than on a screen of its own**, because the moment somebody can
 * bind a barcode correctly is the moment they are holding the box and know
 * which one it is. A reference screen away from the shelf is where a carton's
 * label gets bound to the box inside it.
 *
 * **The level is not asked for.** It comes from the subject: the operator is
 * capturing a carton or an each, and the label in their hand is on the thing
 * they are capturing. Asking again is a question whose answer is already on the
 * screen — and one they could get wrong.
 *
 * A part is offered nothing at all: a part has no packaging level and the
 * writer refuses one, so a control here would be a control that always fails.
 */
function Barcodes({ bench, subject }: { bench: CaptureBench; subject: CaptureSubject }) {
  const level = subject.packaging_level;
  if (!level || !subject.item_id) return null;

  return (
    <Face pad={false} as="section">
      <Band count={bench.barcodes.length}>Barcodes</Band>
      <FaceWell>
        <Stack gap={3}>
          <Records>
            {bench.barcodes.map((b) => (
              <Record
                key={b.id}
                name={<Code>{b.barcode}</Code>}
                tags={
                  <Pill tone={b.packaging_level === level ? "state" : "quiet"}>
                    {b.packaging_level}
                  </Pill>
                }
                facts={
                  b.quantity !== null ? <Fact value={b.quantity} label="per scan" /> : undefined
                }
                meta={
                  /* D11 made visible. Null is a feed or an importer, which is a
                     real and different answer from an unnamed person — and the
                     rows that say it are the ones written before anybody had to
                     put their name on a binding. */
                  <Faint>{b.bound_by_name ? `by ${b.bound_by_name}` : "from a feed"}</Faint>
                }
              />
            ))}
          </Records>

          <Row gap={3} wrap align="end">
            <ScanInput
              label={`Bind to this ${level}`}
              value={bench.binding}
              onChange={bench.typeBinding}
              onScan={() => void bench.bind()}
              busy={bench.busy}
              hint="Scan the label on the box in your hand"
            />
            {/* Blank on purpose, and it stays blank unless somebody knows.
                Only a GTIN may carry no count, and the server says so rather
                than this screen guessing a case pack. */}
            <Field
              label="Per scan"
              width="measure"
              value={bench.count}
              onChange={bench.typeCount}
            />
            <Spacer />
            <Key
              size="small"
              disabled={bench.busy || bench.binding.trim() === ""}
              onClick={() => void bench.bind()}
            >
              Bind
            </Key>
          </Row>
        </Stack>
      </FaceWell>
    </Face>
  );
}

/**
 * How it was arranged, in the operator's words before it is a figure.
 *
 * **Tabs rather than keys**, on the rule `Tabs` states about itself: one of a
 * set, exactly one chosen, marked by ground rather than by the lamp — which
 * stays free for Record, the one thing on this screen anybody is about to do.
 *
 * Required at `each` and offered to a part, which is the writer's rule exactly:
 * a carton is rigid and has one arrangement, and asking for the word there
 * would be ceremony. D138.
 */
function Arrangement({
  bench,
  subject,
}: {
  bench: CaptureBench;
  subject: CaptureSubject;
}) {
  const required = presentationNeeded(subject);
  return (
    <Face pad={false}>
      <Band>Arrangement</Band>
      <FaceWell>
        <Stack gap={3}>
          <Tabs
            label="How it was arranged"
            value={bench.figures.presentation}
            options={PRESENTATIONS.map((p) => ({ value: p.value, label: p.label }))}
            onChange={bench.choosePresentation}
          />
          {/* One word, and it goes away once answered. The chooser's options
              are the explanation; a paragraph about why the question exists is
              read on every capture by somebody who knows. */}
          {required && !bench.figures.presentation && <Soft>Required.</Soft>}
        </Stack>
      </FaceWell>
    </Face>
  );
}

/** Stage three: the event exists, so the pictures have something to hang off. */
function Photographs({ bench, subject }: { bench: CaptureBench; subject: CaptureSubject }) {
  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <SubjectHead subject={subject} />

        {bench.recorded && (
          <Face>
            <Row gap={3} wrap>
              <Lamp kind="recorded" />
              <span>
                {bench.recorded.measurements} figure
                {bench.recorded.measurements === 1 ? "" : "s"} recorded.
              </span>
              {bench.recorded.warnings.map((w) => (
                <Faint key={w}>{w}</Faint>
              ))}
            </Row>
          </Face>
        )}

        <Face pad={false}>
          <Band count={bench.taken.length}>Photographs</Band>
          <FaceWell>
            <Stack gap={3}>
              {FACES.map((face) => (
                <FaceSlot key={face} bench={bench} face={face} />
              ))}
            </Stack>
          </FaceWell>
        </Face>

        <Problem bench={bench} />
      </Stack>
    </Panel>
  );
}

/**
 * One face, and the camera.
 *
 * **A file input with `capture="environment"`, not `getUserMedia`.** It opens
 * the rear camera on the handheld, works inside the WebView with no permission
 * dance and no video element to tear down, and degrades to a file picker on a
 * bench. `getUserMedia` buys a live preview and a shutter button we would then
 * own; it is the second move, if a preview turns out to be worth it.
 */
function FaceSlot({ bench, face }: { bench: CaptureBench; face: FaceName }) {
  const taken = bench.taken.includes(face);

  return (
    <Row gap={3} align="center">
      <Soft>{face}</Soft>
      <Spacer />
      {taken && <Pill tone="good">taken</Pill>}
      {/* The label is the control. A `Key` renders a button and a button
          cannot open a camera; a file input renders chrome no stylesheet may
          replace. So the input is taken out of the visual layer and the label
          around it is dressed as a key, which keeps it operable by keyboard
          and findable by a screen reader. */}
      <label className={styles.camera}>
        <span className={styles.hidden}>Photograph the {face}</span>
        <span aria-hidden="true">{taken ? "Retake" : "Photograph"}</span>
        <input
          type="file"
          accept="image/*"
          capture="environment"
          disabled={bench.busy}
          onChange={(e) => {
            const file = e.currentTarget.files?.[0];
            // Cleared so choosing the same file twice still fires a change: a
            // retake is a new row (D132) and must be possible.
            e.currentTarget.value = "";
            if (file) void bench.attach(face, file);
          }}
        />
      </label>
    </Row>
  );
}

function SubjectHead({ subject }: { subject: CaptureSubject }) {
  return (
    <Face>
      <FaceWell>
        <Row gap={3} align="baseline" wrap>
          <Code>{subject.code}</Code>
          <Level subject={subject} />
          <Spacer />
          {subject.description && <Faint>{subject.description}</Faint>}
        </Row>
      </FaceWell>
    </Face>
  );
}

/**
 * What is already on file for this subject.
 *
 * Shown while the operator keys, because a figure that disagrees with the held
 * one by a factor of ten is a mistyped unit, and the moment to notice is with
 * the box still in hand.
 */
function Held({ subject }: { subject: CaptureSubject }) {
  // **A declared absence is on file.** Drawing "nothing on file" over a subject
  // somebody has looked at and answered would put the operator back at the
  // start of the question they already settled. D138.
  const nothing =
    subject.gross_weight_g === null &&
    subject.length_mm === null &&
    subject.width_mm === null &&
    subject.height_mm === null &&
    !subject.weight_absent &&
    !subject.dimensions_absent;

  // `none` rather than an em dash: the dash is this screen's word for *not
  // recorded*, and the whole point of D138 is that those are different answers.
  const dimension = (value: number | null) =>
    value !== null ? centimetres(value) : subject.dimensions_absent ? "none" : "—";

  return (
    <Face pad={false}>
      <Band>Recorded</Band>
      <FaceWell>
        {nothing ? (
          <EmptySlot
            label="Nothing on file"
            note="Not measured yet."
          />
        ) : (
          <Row gap={4} wrap align="end">
            <Readout
              label="Gross"
              value={
                subject.gross_weight_g !== null
                  ? grams(subject.gross_weight_g)
                  : subject.weight_absent
                    ? "none"
                    : "—"
              }
              {...(subject.gross_weight_g === null ? {} : { unit: "kg" })}
            />
            <Readout
              label="L"
              size="small"
              value={dimension(subject.length_mm)}
              {...(subject.length_mm === null ? {} : { unit: "cm" })}
            />
            <Readout
              label="W"
              size="small"
              value={dimension(subject.width_mm)}
              {...(subject.width_mm === null ? {} : { unit: "cm" })}
            />
            <Readout
              label="H"
              size="small"
              value={dimension(subject.height_mm)}
              {...(subject.height_mm === null ? {} : { unit: "cm" })}
            />
            <Spacer />
            {subject.method && <Faint>{subject.method}</Faint>}
          </Row>
        )}
      </FaceWell>
    </Face>
  );
}

function Problem({ bench }: { bench: CaptureBench }) {
  if (!bench.problem) return null;
  return (
    <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>
  );
}

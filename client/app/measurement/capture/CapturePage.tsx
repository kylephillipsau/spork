import { Camera, Check, PackageSearch } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  EmptyState,
  Fact,
  Facts,
  List,
  ListItem,
  Page,
  PageHeader,
  ScanField,
  Skeleton,
  Stat,
  StatGrid,
  Tabs,
  TextField,
} from "@ui/index";
import { centimetres, grams } from "@app/common/format";
import { Faint } from "@app/common/cells";
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
import s from "./capture-page.module.css";

/**
 * Capture (D171): scan, weigh, measure, photograph.
 *
 * Three stages, and the operator is in exactly one of them. Photographs come
 * after the figures because D133 makes the session one act and an image hangs
 * off the event that act produced: there is nothing to attach to until then.
 */
export function CapturePage({ bench }: { bench: CaptureBench }) {
  if (bench.stage.kind === "figures") {
    return <Figures bench={bench} subject={bench.stage.subject} />;
  }
  if (bench.stage.kind === "photographs") {
    return <Photographs bench={bench} subject={bench.stage.subject} />;
  }

  return (
    <Page>
      <PageHeader
        title="Capture"
        actions={
          bench.status.kind === "ready" ? (
            <>
              <Badge>{bench.status.screen.site}</Badge>
              <Badge tone={bench.status.screen.walk.length > 0 ? "accent" : "success"}>
                {bench.status.screen.walk.length > 0 ? `${bench.status.screen.walk.length} to capture` : "Nothing waiting"}
              </Badge>
            </>
          ) : undefined
        }
      />
      {bench.status.kind === "loading" ? (
        <Card>
          <Skeleton width="50%" />
        </Card>
      ) : bench.status.kind === "failed" ? (
        <Alert tone="danger">{bench.status.message}</Alert>
      ) : (
        <Worklist bench={bench} screen={bench.status.screen} />
      )}
    </Page>
  );
}

/**
 * `260101` as `26-01-01`. Punctuation only, deliberately not a date: AI 17
 * carries a two-digit year, and choosing its century is a rule the server also
 * declines to apply.
 */
function yymmdd(raw: string): string {
  return /^\d{6}$/.test(raw) ? `${raw.slice(0, 2)}-${raw.slice(2, 4)}-${raw.slice(4)}` : raw;
}

function subjectKey(subject: CaptureSubject): string {
  return `${subject.item_id ?? subject.item_style_id ?? subject.item_part_id}/${subject.packaging_level ?? "part"}`;
}

/**
 * What the last scan turned out to be. Four outcomes, four things to say:
 * "no such item" when the truth is "that is two items" is how a scan ends up
 * recorded against the wrong one.
 */
function Scanned({ bench }: { bench: CaptureBench }) {
  const found = bench.scan.found;
  if (!found) return null;

  if (found.outcome === "resolved" && found.subjects.length === 1) {
    const subject = found.subjects[0];
    if (!subject) return null;
    return (
      <Card
        title="Scanned"
        description={subject.description ?? undefined}
        actions={<Badge tone="success">{subject.via.replace(/_/g, " ")}</Badge>}
        padded={false}
      >
        <div className={s.scannedHead}>
          <span className={s.code}>{subject.code}</span>
          {/* A GS1-128 carton label carries the lot and expiry beside the item. */}
          {(found.lot || found.expiry) && (
            <Facts>
              <Fact label="Lot" mono>
                {found.lot}
              </Fact>
              <Fact label="Expiry" mono>
                {found.expiry && yymmdd(found.expiry)}
              </Fact>
            </Facts>
          )}
        </div>
        {/* The worklist's own subjects, not a second enumeration: a scan cannot
            open a session the worklist would not list. */}
        <List label="Scanned subjects">
          {subject.capture.map((target) => (
            <ListItem
              key={subjectKey(target)}
              title={target.code}
              badges={
                <>
                  <Level subject={target} />
                  {target.item_style_id && <Faint>as {target.code}</Faint>}
                </>
              }
              meta={
                target.wants.length === 0 ? (
                  <Badge tone="success">complete</Badge>
                ) : (
                  target.wants.map((w) => (
                    <Badge key={w} tone="accent">
                      {w}
                    </Badge>
                  ))
                )
              }
              action={
                <Button size="sm" disabled={bench.busy} onClick={() => bench.choose(target)}>
                  Capture
                </Button>
              }
            />
          ))}
        </List>
      </Card>
    );
  }

  // Describes, and does not instruct: the server fills `capture` only when
  // exactly one subject survives, so there is nothing here to pick with.
  const said =
    found.outcome === "identifier_ambiguous"
      ? "This code matches more than one item."
      : found.outcome === "identifier_unknown"
        ? "No match for this code."
        : "Not a recognised code.";

  return (
    <>
      <Alert tone="warning" onDismiss={bench.clearScan}>
        {said} <span className={s.code}>{found.scanned}</span>
      </Alert>
      {found.subjects.length > 1 && (
        <Card title="Matches" description="Both are on the worklist below." padded={false}>
          <List label="Matches">
            {found.subjects.map((m) => (
              <ListItem
                key={`${m.kind}/${m.id}`}
                lead={<Badge>{m.kind}</Badge>}
                title={m.code}
                description={m.description ?? undefined}
                meta={m.via.replace(/_/g, " ")}
              />
            ))}
          </List>
        </Card>
      )}
    </>
  );
}

/**
 * The action in the reachable third, which changes with the stage. Rendered
 * into the frame's dock so a long worklist cannot scroll it out of reach.
 */
export function CaptureDockPage({ bench }: { bench: CaptureBench }) {
  if (bench.stage.kind === "figures") {
    const ready = measurementsOf(bench.figures).length > 0;
    return (
      <div className={s.dock}>
        {!ready && <p className={s.dockNote}>Enter at least one measurement.</p>}
        <div className={s.dockRow}>
          <Button size="lg" onClick={bench.leave}>
            Back
          </Button>
          <Button size="lg" variant="primary" block disabled={bench.busy || !ready} onClick={() => void bench.record()}>
            Record
          </Button>
        </div>
      </div>
    );
  }

  if (bench.stage.kind === "photographs") {
    return (
      <div className={s.dock}>
        <p className={s.dockNote}>
          {bench.taken.length} of {FACES.length} faces photographed. Measurements saved.
        </p>
        <Button size="lg" variant="primary" block disabled={bench.busy} onClick={() => void bench.finish()}>
          Done
        </Button>
      </div>
    );
  }

  return null;
}

/**
 * What needs capturing, in the order somebody would walk it: one list sorted
 * by bin, because a walk has one order. `wants` says what the trip is for and
 * `because` says why the row is here, in the server's words (D114).
 */
function Worklist({ bench, screen }: { bench: CaptureBench; screen: CaptureScreen }) {
  return (
    <>
      {/* The barcode on the thing in your hand is the fastest path (D111).
          This screen claims scan focus, so the header's search is not shown (D117). */}
      <ScanField
        label="Scan"
        value={bench.scan.typed}
        onChange={bench.typeScan}
        onScan={(v) => void bench.lookUp(v)}
        busy={bench.busy}
        refocus={bench.scan.refocus}
        placeholder="Barcode, GTIN, or type a code"
      />

      <Scanned bench={bench} />

      <Card title="The walk" count={screen.walk.length} padded={false}>
        {screen.walk.length === 0 ? (
          <EmptyState icon={<PackageSearch />} title="Nothing here" description="Nothing at this site needs to be measured." />
        ) : (
          <List label="The walk">
            {screen.walk.map((subject) => (
              <Subject key={subjectKey(subject)} bench={bench} subject={subject} />
            ))}
          </List>
        )}
      </Card>
    </>
  );
}

/**
 * Which subject of this item. A part has no packaging level and gets its own
 * label instead, which is what tells two rows under one code apart (D139).
 */
function Level({ subject }: { subject: CaptureSubject }) {
  if (subject.item_part_id) {
    return <Badge tone="accent">{subject.part_label ?? "part"}</Badge>;
  }
  return <Badge>{subject.packaging_level}</Badge>;
}

function Subject({ bench, subject }: { bench: CaptureBench; subject: CaptureSubject }) {
  return (
    <ListItem
      // The bin leads: it is the sort key and what gets you to the shelf.
      lead={<Badge>{subject.location_code ?? "no bin"}</Badge>}
      title={subject.code}
      badges={
        <>
          <Level subject={subject} />
          {/* Whose figure this is (D108): an inherited number is not an own one. */}
          {subject.source === "style" && subject.style_code && <Faint>from {subject.style_code}</Faint>}
          {subject.wants.map((want) => (
            <Badge key={want} tone="accent">
              {want}
            </Badge>
          ))}
        </>
      }
      description={subject.description ?? undefined}
      meta={
        <>
          <span>{subject.soh} on hand</span>
          {subject.demand > 0 && <span>{subject.demand} on order</span>}
          <span>{subject.because}</span>
        </>
      }
      action={
        <Button size="sm" disabled={bench.busy} onClick={() => bench.choose(subject)}>
          Capture
        </Button>
      }
    />
  );
}

/** Stage two: a box in hand, and four numbers. */
function Figures({ bench, subject }: { bench: CaptureBench; subject: CaptureSubject }) {
  const offered = presentationOffered(subject) && !bench.figures.noDimensions;
  return (
    <Page>
      <SubjectHead subject={subject} />

      <Card
        title="Measurements"
        actions={
          bench.figures.noDimensions ? (
            <Button size="sm" onClick={bench.toggleNoDimensions}>
              Add dimensions
            </Button>
          ) : (
            // The answer that is not a number, offered where it can be true:
            // a single loose thing or a part. A carton always has a box (D138).
            presentationOffered(subject) && (
              <Button size="sm" onClick={bench.toggleNoDimensions}>
                No dimensions
              </Button>
            )
          )
        }
      >
        <div className={s.figures}>
          <div className={s.weight}>
            <TextField
              label="Gross weight"
              inputMode="decimal"
              autoComplete="off"
              trailing="kg"
              value={bench.figures.weight}
              onChange={(e) => bench.type("weight", e.target.value)}
            />
          </div>
          {/* Three fields rather than one "LxWxH", so the writer can say which
              one was mistyped. Centimetres, as on the tape; the writer converts. */}
          {bench.figures.noDimensions ? (
            <p className={s.muted}>No dimensions.</p>
          ) : (
            <div className={s.dimensions}>
              <TextField
                label="Length"
                inputMode="decimal"
                autoComplete="off"
                trailing="cm"
                value={bench.figures.length}
                onChange={(e) => bench.type("length", e.target.value)}
              />
              <TextField
                label="Width"
                inputMode="decimal"
                autoComplete="off"
                trailing="cm"
                value={bench.figures.width}
                onChange={(e) => bench.type("width", e.target.value)}
              />
              <TextField
                label="Height"
                inputMode="decimal"
                autoComplete="off"
                trailing="cm"
                value={bench.figures.height}
                onChange={(e) => bench.type("height", e.target.value)}
              />
            </div>
          )}
        </div>
      </Card>

      {offered && <Arrangement bench={bench} subject={subject} />}

      {/* Measure the parts, not this: a set whose parts ship separately has no
          box of its own (D139). */}
      {subject.parts > 0 && (
        <Alert tone="info">
          {subject.parts} parts are listed separately. Measure each one.
        </Alert>
      )}

      <Barcodes bench={bench} subject={subject} />

      <Held subject={subject} />
      <Problem bench={bench} />
    </Page>
  );
}

/**
 * What this box answers to, and binding a label to it (D164). Here because the
 * moment somebody can bind correctly is when they hold the box. The level comes
 * from the subject, and a part is offered nothing: the writer refuses one.
 */
function Barcodes({ bench, subject }: { bench: CaptureBench; subject: CaptureSubject }) {
  const level = subject.packaging_level;
  if (!level || !subject.item_id) return null;

  return (
    <Card title="Barcodes" count={bench.barcodes.length} padded={false}>
      {bench.barcodes.length > 0 && (
        <List label="Barcodes">
          {bench.barcodes.map((b) => (
            <ListItem
              key={b.id}
              title={b.barcode}
              badges={<Badge tone={b.packaging_level === level ? "accent" : "neutral"}>{b.packaging_level}</Badge>}
              meta={
                <>
                  {b.quantity !== null && <span>{b.quantity} per scan</span>}
                  {/* D11: null is a feed or an importer, not an unnamed person. */}
                  <span>{b.bound_by_name ? `by ${b.bound_by_name}` : "from a feed"}</span>
                </>
              }
            />
          ))}
        </List>
      )}
      <div className={s.bind}>
        <div className={s.bindScan}>
          <ScanField
            label={`Bind to this ${level}`}
            value={bench.binding}
            onChange={bench.typeBinding}
            onScan={() => void bench.bind()}
            busy={bench.busy}
            claim={false}
            placeholder="Scan the box label"
          />
        </div>
        {/* Blank unless somebody knows. Only a GTIN may carry no count, and the
            server says so rather than this screen guessing a case pack. */}
        <div className={s.perScan}>
          <TextField
            label="Per scan"
            inputMode="numeric"
            autoComplete="off"
            value={bench.count}
            onChange={(e) => bench.typeCount(e.target.value)}
          />
        </div>
        <Button disabled={bench.busy || bench.binding.trim() === ""} onClick={() => void bench.bind()}>
          Bind
        </Button>
      </div>
    </Card>
  );
}

/**
 * How it was arranged, in the operator's words before it is a figure. Required
 * at `each` and offered to a part; a carton is rigid and has one (D138).
 */
function Arrangement({ bench, subject }: { bench: CaptureBench; subject: CaptureSubject }) {
  const required = presentationNeeded(subject);
  return (
    <Card title="Arrangement" description={required && !bench.figures.presentation ? "Required." : undefined}>
      <div className={s.arrangement}>
        <Tabs
          aria-label="Arrangement"
          value={bench.figures.presentation}
          onValueChange={bench.choosePresentation}
          items={PRESENTATIONS.map((p) => ({ value: p.value, label: p.label }))}
        />
      </div>
    </Card>
  );
}

/** Stage three: the event exists, so the pictures have something to hang off. */
function Photographs({ bench, subject }: { bench: CaptureBench; subject: CaptureSubject }) {
  return (
    <Page>
      <SubjectHead subject={subject} />

      {bench.recorded && (
        <Alert tone="success">
          {bench.recorded.measurements} figure{bench.recorded.measurements === 1 ? "" : "s"} recorded.
          {bench.recorded.warnings.map((w) => (
            <span key={w} className={s.warning}>
              {w}
            </span>
          ))}
        </Alert>
      )}

      <Card title="Photographs" count={bench.taken.length} padded={false}>
        <List label="Photographs">
          {FACES.map((face) => (
            <FaceSlot key={face} bench={bench} face={face} />
          ))}
        </List>
      </Card>

      <Problem bench={bench} />
    </Page>
  );
}

/**
 * One face, and the camera. A file input with `capture="environment"` opens
 * the rear camera in the WebView with no permission dance, and degrades to a
 * file picker on a bench. A button cannot open a camera, so the label around
 * the hidden input is dressed as one; that keeps it keyboard-operable.
 */
function FaceSlot({ bench, face }: { bench: CaptureBench; face: FaceName }) {
  const taken = bench.taken.includes(face);

  return (
    <ListItem
      title={<span className={s.face}>{face}</span>}
      badges={taken && <Badge tone="success">taken</Badge>}
      action={
        <label className={s.camera} aria-disabled={bench.busy || undefined}>
          <span className={s.hidden}>Photograph the {face}</span>
          {taken ? <Check aria-hidden /> : <Camera aria-hidden />}
          <span aria-hidden="true">{taken ? "Retake" : "Photograph"}</span>
          <input
            type="file"
            accept="image/*"
            capture="environment"
            disabled={bench.busy}
            onChange={(e) => {
              const file = e.currentTarget.files?.[0];
              // Cleared so the same file twice still fires: a retake is a new row (D132).
              e.currentTarget.value = "";
              if (file) void bench.attach(face, file);
            }}
          />
        </label>
      }
    />
  );
}

function SubjectHead({ subject }: { subject: CaptureSubject }) {
  return (
    <PageHeader
      title={
        <span className={s.subject}>
          <span className={s.code}>{subject.code}</span>
          <Level subject={subject} />
        </span>
      }
      description={subject.description ?? undefined}
    />
  );
}

/**
 * What is already on file, shown while the operator keys: a figure ten times
 * the held one is a mistyped unit, and the moment to notice is with the box
 * in hand.
 */
function Held({ subject }: { subject: CaptureSubject }) {
  // A declared absence is on file (D138).
  const nothing =
    subject.gross_weight_g === null &&
    subject.length_mm === null &&
    subject.width_mm === null &&
    subject.height_mm === null &&
    !subject.weight_absent &&
    !subject.dimensions_absent;

  // `none` rather than a dash: the dash means not recorded, and D138 is that
  // those are different answers.
  const dimension = (value: number | null) =>
    value !== null ? `${centimetres(value)} cm` : subject.dimensions_absent ? "none" : "—";

  return (
    <Card title="Recorded" actions={subject.method ? <Faint>{subject.method}</Faint> : undefined} padded={nothing}>
      {nothing ? (
        <EmptyState title="Nothing on file" description="Not measured yet." />
      ) : (
        <StatGrid>
          <Stat
            label="Gross"
            size="md"
            value={
              subject.gross_weight_g !== null ? `${grams(subject.gross_weight_g)} kg` : subject.weight_absent ? "none" : "—"
            }
          />
          <Stat label="L" size="md" value={dimension(subject.length_mm)} />
          <Stat label="W" size="md" value={dimension(subject.width_mm)} />
          <Stat label="H" size="md" value={dimension(subject.height_mm)} />
        </StatGrid>
      )}
    </Card>
  );
}

function Problem({ bench }: { bench: CaptureBench }) {
  if (!bench.problem) return null;
  return (
    <Alert tone="danger" onDismiss={bench.dismiss}>
      {bench.problem}
    </Alert>
  );
}


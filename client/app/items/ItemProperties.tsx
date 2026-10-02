import { Suspense, lazy, useState, type ReactNode } from "react";
import { Barcode, Camera, ChevronLeft, ChevronRight, Crop, ImageOff, Ruler, Scale } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  Drawer,
  EmptyState,
  Fact,
  Facts,
  Link,
  Select,
  Skeleton,
  Spacer,
  Stack,
  Tabs,
  TextField,
  Toolbar,
} from "@ui/index";
import { imageUrl } from "@domain/api";
import type { CaptureSubject, ItemView, SubjectPhoto } from "@domain/types";
import { Thumb } from "@app/common/Thumb";
import { Faint, dateTime, sentence } from "@app/common/cells";
import { centimetres, kg } from "@app/common/format";

import { BOX_FACES, boxSize, faceName, facesToAsk, isBox, measuredAspect, type BoxFace } from "./box";
import { handheld } from "./crop";
import { FaceCrop } from "./FaceCrop";
import {
  PRESENTATIONS,
  bindable,
  holdsInWords,
  isOwnCarton,
  nameOf,
  photosOf,
  presentationNeeded,
  presentationOffered,
  subjectKey,
  weighable,
  type Face,
  shown,
} from "./subjects";

// three.js is its own chunk, fetched the first time a box is shown.
const BoxView = lazy(() => import("./BoxView"));
import { useItemProperties, type Action, type PropertiesDesk } from "./useItemProperties";
import s from "./items.module.css";

/**
 * An item's properties: what gets measured for it, what is known of each, and
 * weighing, measuring, photographing and labelling each (D174).
 *
 * **One implementation, wherever an item is.** The item's page draws it full
 * size; anything else that shows an item (the item list, the packing bench)
 * opens it in an [`ItemDrawer`] beside what it was doing.
 */
export function ItemProperties({ item, desk }: { item: ItemView; desk: PropertiesDesk }) {
  return (
    <Stack gap={4}>
      {desk.said && (
        <Alert tone={desk.said.tone} onDismiss={desk.dismiss}>
          {desk.said.text}
          {desk.said.finding && (
            <>
              {" "}
              <Link href={`/findings/${desk.said.finding}`}>Open the finding</Link>
            </>
          )}
        </Alert>
      )}
      {desk.problem && (
        <Alert tone="danger" onDismiss={desk.dismiss}>
          {desk.problem}
        </Alert>
      )}
      {item.subjects.length === 0 ? (
        <Card>
          <EmptyState
            icon={<Ruler />}
            title="Nothing to measure for this item yet"
            description="Its each is measured once it is on file."
          />
        </Card>
      ) : (
        item.subjects.map((subject) => <Subject key={subjectKey(subject)} item={item} subject={subject} desk={desk} />)
      )}
      {desk.cropping && (
        <FaceCrop
          key={desk.cropping.image_id}
          cropping={desk.cropping}
          name={faceName(desk.cropping.face, desk.cropping.subject)}
          aspect={measuredAspect(desk.cropping.subject, desk.cropping.face)}
          desk={desk}
        />
      )}
    </Stack>
  );
}

/** What it is: its photo and its family. What a carton of it holds is on the carton. */
export function ItemSummary({ item }: { item: ItemView }) {
  return (
    <div className={s.summary}>
      <Photo key={item.item_id} item={item} />
      <Facts columns={1}>
        <Fact label="Family" always>
          {item.style ? (
            <>
              <span className={s.code}>{item.style.code}</span>
              <Faint> · {item.style.variants === 1 ? "1 code" : `${item.style.variants} codes`}</Faint>
            </>
          ) : (
            <Faint>Not part of a family</Faint>
          )}
        </Fact>
      </Facts>
    </div>
  );
}

/**
 * An item's properties beside whatever screen it was found on. It reads the
 * item itself, so opening it is one line: `<ItemDrawer itemId={…} onClose={…} />`.
 * `desk` is for fixtures, which draw it with no network.
 *
 * Previous and Next, where given, walk the list it was opened from, so a list
 * of things to measure is worked down without closing it.
 */
export function ItemDrawer({
  itemId,
  onClose,
  previous,
  next,
  desk,
}: {
  itemId: string | null;
  onClose: () => void;
  previous?: (() => void) | null | undefined;
  next?: (() => void) | null | undefined;
  desk?: PropertiesDesk | undefined;
}) {
  const own = useItemProperties(desk ? null : itemId);
  const d = desk ?? own;
  const item = d.read.kind === "ready" ? d.read.item : null;
  const walks = previous !== undefined || next !== undefined;

  return (
    <Drawer
      open={itemId !== null}
      onOpenChange={(open) => !open && onClose()}
      width={600}
      title={item ? <span className={s.code}>{item.code}</span> : "Item"}
      description={item?.description}
      footer={
        <>
          {walks && (
            <>
              <Button icon={<ChevronLeft />} disabled={!previous} onClick={previous ?? undefined}>
                Previous
              </Button>
              <Button iconAfter={<ChevronRight />} disabled={!next} onClick={next ?? undefined}>
                Next
              </Button>
            </>
          )}
          <Spacer />
          {item && <Link href={`/items/${item.item_id}`}>Open the item’s page</Link>}
        </>
      }
    >
      {d.read.kind === "failed" ? (
        <Alert tone="danger">{d.read.message}</Alert>
      ) : !item ? (
        <Stack gap={3}>
          <Skeleton width="40%" />
          <Skeleton width="70%" />
        </Stack>
      ) : (
        <Stack gap={5}>
          <ItemSummary item={item} />
          <ItemProperties item={item} desk={d} />
        </Stack>
      )}
    </Drawer>
  );
}

/**
 * An item code that opens the item's properties. A button, not a link: it
 * opens beside the work rather than leaving it.
 */
export function ItemCode({ code, onOpen }: { code: string; onOpen: () => void }) {
  return (
    <button type="button" className={s.itemCode} onClick={onOpen} title={`${code}: size, weight and photos`}>
      {code}
    </button>
  );
}

const ACTIONS: { action: Action; label: string; icon: ReactNode; when: (s: CaptureSubject) => boolean }[] = [
  { action: "weigh", label: "Weigh", icon: <Scale />, when: weighable },
  { action: "measure", label: "Measure", icon: <Ruler />, when: () => true },
  { action: "photos", label: "Photograph", icon: <Camera />, when: () => true },
  { action: "barcodes", label: "Barcodes", icon: <Barcode />, when: bindable },
];

/** One subject: what is known of it, its photographs, and what can be done to it. */
function Subject({ item, subject, desk }: { item: ItemView; subject: CaptureSubject; desk: PropertiesDesk }) {
  const open = desk.open?.key === subjectKey(subject) ? desk.open.action : null;
  const photos = photosOf(item, subject);
  // An item's own carton is a box of so many of it (D178).
  const carton = isOwnCarton(subject);
  const needs = [
    subject.wants.includes("weight") && "weight",
    subject.wants.includes("dimensions") && "size",
    subject.wants.includes("photographs") && "photos",
  ].filter(Boolean) as string[];

  return (
    <Card
      title={nameOf(subject)}
      description={carton && !item.packing ? "Say how many it holds when you weigh or measure it" : provenance(subject)}
      actions={needs.length > 0 && <Badge tone="warning">Needs {needs.join(", ")}</Badge>}
      padded={false}
    >
      <div className={s.known}>
        <Facts columns={2}>
          <Fact label="Weight" always>
            {weightOf(subject)}
          </Fact>
          <Fact label="Size (L × W × H)" always>
            {sizeOf(subject)}
          </Fact>
          {carton && (
            <Fact label="Holds" always>
              {item.packing?.inners_per_carton != null ? holdsInWords(item.packing) : <Faint>{holdsInWords(item.packing)}</Faint>}
            </Fact>
          )}
        </Facts>
        {/* With the camera open its photos are shown there, once. */}
        {open !== "photos" && <Photos subject={subject} photos={photos} desk={null} />}
        {subject.parts > 0 && (
          <p className={s.note}>
            It comes as {subject.parts} parts, each listed below. Measure each part; this has no box of its own.
          </p>
        )}
      </div>
      <Toolbar>
        {ACTIONS.filter((a) => a.when(subject)).map((a) => (
          <Button
            key={a.action}
            size="sm"
            icon={a.icon}
            aria-pressed={open === a.action}
            disabled={desk.busy}
            onClick={() => (open === a.action ? desk.close() : desk.show(subject, a.action))}
          >
            {a.label}
          </Button>
        ))}
      </Toolbar>
      {open === "weigh" && <WeighForm subject={subject} desk={desk} />}
      {open === "measure" && <MeasureForm subject={subject} desk={desk} />}
      {open === "photos" && (
        <div className={s.form}>
          <p className={s.note}>
            {isBox(subject)
              ? "Photograph each side you can, and its label. Each photo is saved as it is taken."
              : "Take its photo, and its label. Each is saved as it is taken."}
            {handheld() && " They are cut to their faces at a computer, under Photos to crop."}
          </p>
          <Photos subject={subject} photos={photos} desk={desk} />
          <div className={s.formActions}>
            <Button onClick={desk.close}>Done</Button>
          </div>
        </div>
      )}
      {open === "barcodes" && <BarcodesForm subject={subject} desk={desk} />}
    </Card>
  );
}

/** A scale reading, in the unit the scale shows. */
function WeighForm({ subject, desk }: { subject: CaptureSubject; desk: PropertiesDesk }) {
  return (
    <form
      className={s.form}
      onSubmit={(e) => {
        e.preventDefault();
        void desk.weigh(subject);
      }}
    >
      <div className={s.fields}>
        <div className={s.reading}>
          <TextField
            label="Scale reading"
            inputMode="decimal"
            autoComplete="off"
            autoFocus
            value={desk.reading}
            onChange={(e) => desk.typeReading(e.target.value)}
          />
        </div>
        <div className={s.unit}>
          <Select
            label="Unit"
            value={desk.unit}
            onValueChange={desk.setUnit}
            options={[
              { value: "kg", label: "kg" },
              { value: "g", label: "g" },
            ]}
          />
        </div>
        {isOwnCarton(subject) && <HoldsField desk={desk} />}
      </div>
      <div className={s.formActions}>
        <Button onClick={desk.close}>Cancel</Button>
        <Button type="submit" variant="primary" loading={desk.busy} disabled={!desk.reading.trim()}>
          Record weight
        </Button>
      </div>
    </form>
  );
}

/**
 * Weight and size as one act (D133), in kilograms and centimetres as the
 * instruments read. A thing with no box says so rather than leaving it blank
 * (D138).
 */
function MeasureForm({ subject, desk }: { subject: CaptureSubject; desk: PropertiesDesk }) {
  const f = desk.figures;
  const offered = presentationOffered(subject);
  return (
    <form
      className={s.form}
      onSubmit={(e) => {
        e.preventDefault();
        void desk.measure(subject);
      }}
    >
      <div className={s.fields}>
        <div className={s.reading}>
          <TextField
            label="Gross weight"
            inputMode="decimal"
            autoComplete="off"
            autoFocus
            trailing="kg"
            value={f.weight}
            onChange={(e) => desk.type("weight", e.target.value)}
          />
        </div>
        {isOwnCarton(subject) && <HoldsField desk={desk} />}
      </div>
      {!f.noDimensions && (
        <div className={s.dimensions}>
          {(["length", "width", "height"] as const).map((field) => (
            <TextField
              key={field}
              label={sentence(field)}
              inputMode="decimal"
              autoComplete="off"
              trailing="cm"
              value={f[field]}
              onChange={(e) => desk.type(field, e.target.value)}
            />
          ))}
        </div>
      )}
      {offered && (
        <div>
          <Button size="sm" aria-pressed={f.noDimensions} onClick={desk.toggleNoDimensions}>
            {f.noDimensions ? "It has a size after all" : "It has no box shape to measure"}
          </Button>
        </div>
      )}
      {offered && !f.noDimensions && (
        <div className={s.arrangement}>
          <span className={s.fieldLabel}>
            How it was arranged{presentationNeeded(subject) ? " (needed with a size)" : ""}
          </span>
          <Tabs
            aria-label="How it was arranged"
            value={f.presentation}
            onValueChange={desk.choosePresentation}
            items={PRESENTATIONS.map((p) => ({ value: p.value, label: p.label }))}
          />
        </div>
      )}
      <div className={s.formActions}>
        <Button onClick={desk.close}>Cancel</Button>
        <Button type="submit" variant="primary" loading={desk.busy}>
          Record
        </Button>
      </div>
    </form>
  );
}

/**
 * How many of the item one carton holds (D178): the carton and the item in it,
 * said together. Blank says nothing of the count.
 */
function HoldsField({ desk }: { desk: PropertiesDesk }) {
  return (
    <div className={s.holds}>
      <TextField
        label="How many in it"
        inputMode="numeric"
        autoComplete="off"
        trailing="× each"
        value={desk.holds}
        onChange={(e) => desk.typeHolds(e.target.value)}
      />
    </div>
  );
}

/** The labels it answers to, and binding another to it (D164). */
function BarcodesForm({ subject, desk }: { subject: CaptureSubject; desk: PropertiesDesk }) {
  const level = subject.packaging_level ?? "";
  const bound = desk.barcodes.filter((b) => b.packaging_level === level);
  return (
    <form
      className={s.form}
      onSubmit={(e) => {
        e.preventDefault();
        void desk.bind(subject);
      }}
    >
      {bound.length > 0 ? (
        <ul className={s.barcodes}>
          {bound.map((b) => (
            <li key={b.id}>
              <span className={s.code}>{b.barcode}</span>
              <Faint>
                {b.quantity !== null ? ` · ${b.quantity} per scan` : ""} · {b.bound_by_name ? `by ${b.bound_by_name}` : "from a feed"}
              </Faint>
            </li>
          ))}
        </ul>
      ) : (
        <p className={s.note}>No labels bound to its {level} yet.</p>
      )}
      <div className={s.fields}>
        <div className={s.grow}>
          <TextField
            label={`Bind a label to its ${level}`}
            autoComplete="off"
            autoFocus
            placeholder="Scan or type the label"
            value={desk.binding}
            onChange={(e) => desk.typeBinding(e.target.value)}
          />
        </div>
        <div className={s.unit}>
          <TextField label="Per scan" inputMode="numeric" autoComplete="off" value={desk.count} onChange={(e) => desk.typeCount(e.target.value)} />
        </div>
      </div>
      <div className={s.formActions}>
        <Button onClick={desk.close}>Cancel</Button>
        <Button type="submit" variant="primary" loading={desk.busy} disabled={!desk.binding.trim()}>
          Bind
        </Button>
      </div>
    </form>
  );
}

/**
 * Its photographs. **A box is shown as a box**: its six sides on a box of its
 * measured size, turned by dragging, with its label beside it. A thing with no
 * box shape has one photo and its label.
 *
 * At rest only what has been taken is shown, and a box of one photo is just
 * that photo: the box appears from the second side. With the camera open every
 * face to ask for has a shutter, and the box turns to the side just taken: a
 * file input with `capture="environment"` opens the rear camera on a handheld
 * and a file picker at a desk.
 */
function Photos({
  subject,
  photos,
  desk,
}: {
  subject: CaptureSubject;
  photos: Map<string, SubjectPhoto>;
  desk: PropertiesDesk | null;
}) {
  const box = isBox(subject);
  const asked = facesToAsk(subject);
  const sides: Partial<Record<BoxFace, string>> = {};
  for (const face of BOX_FACES) {
    const photo = photos.get(face);
    if (photo) sides[face] = shown(photo);
  }
  const showBox = box && (desk !== null || Object.keys(sides).length > 1);
  // On the box, the sides need no tiles of their own until there is a camera.
  const tiles = asked.filter((face) => (desk ? true : photos.has(face) && !(showBox && face !== "label")));
  const last = desk?.taken[desk.taken.length - 1];
  const facing = last && last !== "label" ? last : null;

  if (!showBox && tiles.length === 0) return <Faint>No photos yet</Faint>;
  return (
    <div className={s.photos}>
      {showBox && (
        <Suspense fallback={<div className={s.box} />}>
          <BoxView faces={sides} size={boxSize(subject)} facing={facing} label={subject.code} />
        </Suspense>
      )}
      {tiles.length > 0 && (
        <ul className={s.faces} aria-label="Photographs">
          {tiles.map((face) => {
            const photo = photos.get(face);
            const name = faceName(face, subject);
            return (
              <li key={face} className={s.faceTile}>
                <Thumb picture={photo ? { digest: shown(photo), source: "own" } : null} alt={`${subject.code}, ${name}`} />
                <span className={s.faceName}>{name}</span>
                {desk && <Shutter face={face} name={name} subject={subject} desk={desk} taken={desk.taken.includes(face)} />}
                {desk && photo && (
                  <Button
                    size="sm"
                    icon={<Crop />}
                    disabled={desk.busy}
                    aria-label={`Crop the ${name.toLowerCase()}`}
                    onClick={() =>
                      desk.crop({ subject, face, image_id: photo.image_id, digest: photo.digest, corners: photo.cut?.corners ?? null })
                    }
                  >
                    Crop
                  </Button>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

/** A label dressed as a button around a hidden file input: a button cannot open a camera. */
function Shutter({ face, name, subject, desk, taken }: { face: Face; name: string; subject: CaptureSubject; desk: PropertiesDesk; taken: boolean }) {
  return (
    <label className={s.shutter} aria-disabled={desk.busy || undefined}>
      <span className={s.hidden}>Take the {name.toLowerCase()}</span>
      <Camera aria-hidden />
      <span aria-hidden="true">{taken ? "Again" : "Take"}</span>
      <input
        type="file"
        accept="image/*"
        capture="environment"
        disabled={desk.busy}
        onChange={(e) => {
          const file = e.currentTarget.files?.[0];
          // Cleared so the same file twice still fires: a retake is a new row (D132).
          e.currentTarget.value = "";
          if (file) void desk.attach(subject, face, file);
        }}
      />
    </label>
  );
}

/** The front, or a tile saying there is none. A family's photo says so (D141). */
function Photo({ item }: { item: ItemView }) {
  const [missing, setMissing] = useState(false);
  const picture = item.picture;
  if (!picture || missing) {
    return (
      <div className={s.photo} role="img" aria-label="No photo yet">
        <ImageOff aria-hidden />
        <span>No photo yet</span>
      </div>
    );
  }
  return (
    <figure className={s.figure}>
      <img className={s.photo} src={imageUrl(picture.digest)} alt={`${item.code}, front`} onError={() => setMissing(true)} />
      {picture.source !== "own" && (
        <figcaption className={s.caption}>{item.style ? `Photo of the ${item.style.code} family` : "Photo of its family"}</figcaption>
      )}
    </figure>
  );
}

const METHOD: Record<string, string> = {
  instrument: "Measured",
  scan: "Scanned",
  keyed: "Typed in",
  derived: "Worked out",
  estimated: "Estimated",
  transcribed: "Copied from a list",
  asserted: "Stated",
  photographed: "From a photo",
};

/** Whose figures these are and how they were come by: "Measured 3 Oct", "Copied from a list · the STY-7720 family's". */
function provenance(s: CaptureSubject): string {
  const parts: string[] = [];
  if (s.method) parts.push(`${METHOD[s.method] ?? sentence(s.method)}${s.observed_at ? ` ${dateTime(s.observed_at)}` : ""}`);
  if (s.source === "style") parts.push(`the ${s.style_code ?? "family"}’s figures`);
  if (s.source === "mixed") parts.push(`partly the ${s.style_code ?? "family"}’s figures`);
  return parts.length ? parts.join(" · ") : "Nothing recorded yet";
}

function weightOf(s: CaptureSubject): ReactNode {
  if (s.gross_weight_g !== null) return kg(s.gross_weight_g);
  return <Faint>{s.weight_absent ? "None, it was said" : "Not weighed"}</Faint>;
}

function sizeOf(s: CaptureSubject): ReactNode {
  const d = [s.length_mm, s.width_mm, s.height_mm];
  if (d.some((v) => v !== null)) return `${d.map((v) => (v === null ? "?" : centimetres(v))).join(" × ")} cm`;
  return <Faint>{s.dimensions_absent ? "No box shape" : "Not measured"}</Faint>;
}

import { Suspense, lazy, useState, type ReactNode } from "react";
import { Barcode, Camera, ChevronLeft, ChevronRight, Crop, ImageOff, Ruler, Scale } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  Dialog,
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
  cx,
} from "@ui/index";
import { imageUrl } from "@domain/api";
import type { CaptureSubject, ItemView, PackagingType, Picture, SubjectPhoto } from "@domain/types";
import { Thumb } from "@app/common/Thumb";
import { Faint, dateTime, sentence } from "@app/common/cells";
import { centimetres, kg } from "@app/common/format";

import { BOX_FACES, boxSize, faceName, facesToAsk, isBox, isRound, measuredAspect, type BoxFace } from "./box";
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
  readHolds,
  readHoldsFor,
  subjectKey,
  unitWord,
  levelName,
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
  // **What is only offered stays out of the way** (D218): a carton nobody has
  // said, a single product inside the box it is sold as. Each is a quiet line
  // until somebody asks to measure it, or a form is already open on it.
  const [asked, setAsked] = useState<Set<string>>(() => new Set());
  const shownNow = (subject: CaptureSubject) =>
    !subject.offered || asked.has(subjectKey(subject)) || desk.open?.key === subjectKey(subject);
  const visible = item.subjects.filter(shownNow);
  const offers = item.subjects.filter((subject) => !shownNow(subject));
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
        visible.map((subject) => (
          <Subject key={subjectKey(subject)} item={item} subject={subject} desk={desk} alone={visible.length === 1} />
        ))
      )}
      {offers.length > 0 && (
        <div className={s.offers}>
          {offers.map((subject) => (
            <Button
              key={subjectKey(subject)}
              size="sm"
              variant="ghost"
              onClick={() => setAsked((was) => new Set(was).add(subjectKey(subject)))}
            >
              {subject.packaging_level === "carton"
                ? "Comes in a carton?"
                : `Measure a single one${unitWord(item) === "Each" ? "" : ` from the ${unitWord(item).toLowerCase()}`}`}
            </Button>
          ))}
        </div>
      )}
      <AddVariant desk={desk} />
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

/** What it is: its photo, what it is sold as, and its family. What a carton of it holds is on the carton. */
export function ItemSummary({ item, desk }: { item: ItemView; desk?: PropertiesDesk | undefined }) {
  const pictures = item.style?.picture_item_id === item.item_id;
  const [changing, setChanging] = useState(false);
  return (
    <div className={s.summary}>
      <Photo key={item.item_id} item={item} />
      <Facts columns={1}>
        <Fact label="Sold as" always>
          {soldAs(item)}
          {desk && (
            <div>
              <Button size="sm" disabled={desk.busy} onClick={() => setChanging(true)}>
                Change
              </Button>
            </div>
          )}
          {desk && changing && <SoldAsDialog item={item} desk={desk} onClose={() => setChanging(false)} />}
        </Fact>
        <Fact label="Family" always>
          {item.style ? (
            <>
              <span className={s.code}>{item.style.code}</span>
              <Faint> · {item.style.variants === 1 ? "1 code" : `${item.style.variants} codes`}</Faint>
              {desk && item.style.variants > 1 && (
                <div>
                  <Button size="sm" disabled={desk.busy} onClick={() => void desk.pictureFamily(!pictures)}>
                    {pictures ? "Pictures the family · undo" : "Use as the family’s picture"}
                  </Button>
                </div>
              )}
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
          <ItemSummary item={item} desk={d} />
          <ItemProperties item={item} desk={d} />
        </Stack>
      )}
    </Drawer>
  );
}

/**
 * Whether a card's records can be moved (D219): one of the item's own levels
 * with something recorded on it. Another of its cards, or another item's
 * (D222), is where they go.
 */
function movable(item: ItemView, s: CaptureSubject): boolean {
  return ownLevel(item, s) && recorded(s);
}

type Level = "each" | "inner" | "carton";

/**
 * The levels of an item a card's records could go to: those it has a card
 * for, shown or offered, other than the card itself. A pack or carton is only
 * a definite thing under a case pack (D23), so with none said there is only
 * the single product.
 */
function destinations(item: ItemView, s: CaptureSubject): Level[] {
  const there = new Set(item.subjects.filter((x) => ownLevel(item, x)).map((x) => x.packaging_level));
  return (["each", "inner", "carton"] as const).filter(
    (level) =>
      !(item.item_id === s.item_id && level === s.packaging_level) &&
      there.has(level) &&
      (level === "each" || item.packing !== null),
  );
}

function ownLevel(item: ItemView, s: CaptureSubject): boolean {
  const level = s.packaging_level;
  return (
    s.item_id === item.item_id &&
    !s.item_style_id &&
    !s.lot_id &&
    !s.item_part_id &&
    (level === "each" || level === "inner" || level === "carton")
  );
}

/** Something recorded against the card itself: a family's figures or a variant's shown on it are not its own. */
function recorded(s: CaptureSubject): boolean {
  if (s.source === "style" || s.source === "variant") return false;
  return s.gross_weight_g !== null || s.length_mm !== null || s.weight_absent || s.dimensions_absent || s.faces.length > 0;
}

/**
 * Move a card's figures and photos to the card they belong on (D219): a box
 * of ten weighed on the carton card, put on the box's; or a kit's part
 * measured on the kit's card, put on the part's own item (D222). Kept with
 * when and how they were taken; nothing is rewritten.
 */
export function MoveDialog({
  item,
  subject,
  desk,
  onClose,
}: {
  item: ItemView;
  subject: CaptureSubject;
  desk: PropertiesDesk;
  onClose: () => void;
}) {
  const here = nameOf(subject, item);
  const ownCards = destinations(item, subject).length > 0;
  const [elsewhere, setElsewhere] = useState(!ownCards);
  // The other item, once found by its code.
  const [other, setOther] = useState<ItemView | null>(null);
  const [code, setCode] = useState("");
  const into = elsewhere ? other : item;
  const choices = into ? destinations(into, subject).map((level) => ({ value: level, label: levelName(level, into) })) : [];
  // A card with records of its own was measured as itself, and is not filled over.
  const taken = new Set(into ? into.subjects.filter((x) => ownLevel(into, x) && recorded(x)).map((x) => x.packaging_level) : []);
  // What it is sold as, when that card is empty: where a misfiled box most often belongs.
  const open = choices.filter((c) => !taken.has(c.value));
  const [picked, setPicked] = useState<Level | null>(null);
  const to = choices.find((c) => c.value === picked)?.value ?? (open.find((c) => c.value === into?.unit.level) ?? open[0] ?? choices[0])?.value;
  const level = choices.find((c) => c.value === to)?.label ?? to;
  const name = elsewhere && into ? `${into.code} · ${level}` : level;
  const find = () => {
    if (code.trim()) void desk.findItem(code).then(setOther);
  };
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && !desk.busy && onClose()}
      width={460}
      title="Move to another card"
      description={`Everything recorded on ${here} moves to the card it belongs on, with when and how it was taken.`}
      footer={
        <>
          <Button onClick={onClose} disabled={desk.busy}>
            Cancel
          </Button>
          <Button
            variant="primary"
            loading={desk.busy}
            disabled={!into || !to || taken.has(to)}
            onClick={() => into && to && void desk.refile(subject, { item: into.item_id, level: to }, name ?? to).then((ok) => ok && onClose())}
          >
            {into && to ? `Move to ${name}` : "Move"}
          </Button>
        </>
      }
    >
      <Stack gap={3}>
        {desk.problem && (
          <Alert tone="danger" onDismiss={desk.dismiss}>
            {desk.problem}
          </Alert>
        )}
        {ownCards && (
          <Tabs
            aria-label="Which item"
            value={elsewhere ? "other" : "this"}
            onValueChange={(v) => setElsewhere(v === "other")}
            items={[
              { value: "this", label: "This item" },
              { value: "other", label: "Another item" },
            ]}
          />
        )}
        {elsewhere && (
          <form
            className={s.findItem}
            onSubmit={(e) => {
              e.preventDefault();
              find();
            }}
          >
            <TextField
              label="The item it belongs to"
              placeholder="Its code"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              autoFocus
            />
            <Button type="submit" disabled={desk.busy || !code.trim()}>
              Find
            </Button>
          </form>
        )}
        {elsewhere && other && <ItemLine code={other.code} description={other.description} picture={other.picture} />}
        {into && choices.length > 0 && (
          <Tabs aria-label="Move to" value={to ?? ""} onValueChange={(v) => setPicked(v as Level)} items={choices} />
        )}
        {into && choices.length === 0 && <p className={s.note}>{into.code} has no card to move these to.</p>}
        {to && taken.has(to) && (
          <p className={s.note}>{name} has figures or photos of its own. Move those away first, or record over them.</p>
        )}
      </Stack>
    </Dialog>
  );
}

/**
 * What one of it is, and what it comes in (D218): "Box of 100 · in cartons of
 * 10 boxes". Whose word it is, when Spork said it rather than NetSuite.
 */
function soldAs(item: ItemView): ReactNode {
  const unit = levelName(item.unit.level, item);
  const carton =
    item.unit.level !== "carton" && item.packing?.inners_per_carton
      ? levelName("carton", item).replace(/^Carton of/, "in cartons of")
      : null;
  return (
    <>
      {unit}
      {carton && <Faint> · {carton}</Faint>}
      {item.unit.said ? <Faint> · said in Spork</Faint> : !item.unit.netsuite_unit && <Faint> · NetSuite doesn’t say</Faint>}
    </>
  );
}

const SOLD_AS: { level: "each" | "inner" | "carton"; label: string; hint: string }[] = [
  { level: "each", label: "Single item", hint: "One is one thing: a catalogue, a pair of boots, a roll" },
  { level: "inner", label: "Pack or box", hint: "One is a box or pack of several: a box of 100 earplugs" },
  { level: "carton", label: "Carton", hint: "One is a whole carton: gloves by the carton of 1,000" },
];

/** Say which level is one in NetSuite (D218), over its Pack Unit. */
function SoldAsDialog({ item, desk, onClose }: { item: ItemView; desk: PropertiesDesk; onClose: () => void }) {
  const [level, setLevel] = useState(item.unit.level);
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && !desk.busy && onClose()}
      width={440}
      title="What is one of it?"
      description={
        item.unit.netsuite_unit
          ? `What one of ${item.code} is when NetSuite counts it. NetSuite’s Pack Unit says “${item.unit.netsuite_unit}”.`
          : `What one of ${item.code} is when NetSuite counts it. NetSuite’s Pack Unit is blank.`
      }
      footer={
        <>
          <Button onClick={onClose} disabled={desk.busy}>
            Cancel
          </Button>
          <Button
            variant="primary"
            loading={desk.busy}
            disabled={level === item.unit.level}
            onClick={() => void desk.sayUnit(level).then((ok) => ok && onClose())}
          >
            Save
          </Button>
        </>
      }
    >
      <Tabs
        aria-label="What one of it is"
        value={level}
        onValueChange={(v) => setLevel(v as typeof level)}
        items={SOLD_AS.map((o) => ({ value: o.level, label: o.label }))}
      />
      <p className={s.note}>{SOLD_AS.find((o) => o.level === level)?.hint}</p>
    </Dialog>
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

/**
 * An item on a row of work: its photograph, its code, which opens its
 * properties when there is somewhere to open them, and what it is. Anything
 * that lists items to find or handle draws them this way: the packing bench,
 * the bin map's card (D221), and the item a card's records are moving to (D222).
 */
export function ItemLine({
  code,
  description,
  picture,
  onOpen,
  done = false,
}: {
  code: string;
  /** Said under the code; left out where the row has no room for it. */
  description?: string | undefined;
  picture: Picture | null;
  onOpen?: (() => void) | undefined;
  /** Dealt with, so drawn quieter. */
  done?: boolean | undefined;
}) {
  return (
    <span className={s.itemLine}>
      <Thumb picture={picture} alt={description ?? code} />
      <span className={cx(s.itemText, done && s.itemDone)}>
        {onOpen ? <ItemCode code={code} onOpen={onOpen} /> : <span className={s.code}>{code}</span>}
        {/* An item made from a code alone has the code as its description;
            saying it twice is noise. */}
        {description && description !== code && <span className={s.itemDesc}>{description}</span>}
      </span>
    </span>
  );
}

const ACTIONS: { action: Action; label: string; icon: ReactNode; when: (s: CaptureSubject) => boolean }[] = [
  { action: "weigh", label: "Weigh", icon: <Scale />, when: weighable },
  { action: "measure", label: "Measure", icon: <Ruler />, when: () => true },
  { action: "photos", label: "Photograph", icon: <Camera />, when: () => true },
  { action: "barcodes", label: "Barcodes", icon: <Barcode />, when: bindable },
];

/** One subject: what is known of it, its photographs, and what can be done to it. */
function Subject({
  item,
  subject,
  desk,
  alone,
}: {
  item: ItemView;
  subject: CaptureSubject;
  desk: PropertiesDesk;
  /** The only card on the page: what it is sold as goes without saying. */
  alone: boolean;
}) {
  const open = desk.open?.key === subjectKey(subject) ? desk.open.action : null;
  const photos = photosOf(item, subject);
  // A thing that is not a box is asked for every side only when somebody wants them.
  const [sides, setSides] = useState(false);
  const [moving, setMoving] = useState(false);
  // An item's own carton is a box of so many of it (D178).
  const carton = isOwnCarton(subject);
  const needs = [
    subject.wants.includes("weight") && "weight",
    subject.wants.includes("dimensions") && "size",
    subject.wants.includes("photographs") && "photos",
  ].filter(Boolean) as string[];

  return (
    <Card
      title={nameOf(subject, item)}
      description={
        subject.source === "variant"
          ? `Shown from the variant ${subject.variant_code ?? ""}`
          : subject.offered && subject.packaging_level === "each"
            ? "Only if one is ever measured on its own: it is sold and packed as the whole"
            : carton && !item.packing
              ? "Say how many it holds when you weigh or measure it"
              : provenance(subject)
      }
      actions={
        <>
          {subject.is_unit && !alone && <Badge tone="info">One in NetSuite</Badge>}
          {needs.length > 0 && <Badge tone="warning">Needs {needs.join(", ")}</Badge>}
          {subject.lot_id && <VariantChoice item={item} subject={subject} desk={desk} />}
        </>
      }
      padded={false}
    >
      <div className={s.known}>
        <Facts columns={2}>
          <Fact label="Weight" always>
            {weightOf(subject)}
          </Fact>
          <Fact label={subject.diameter_mm !== null ? "Size" : "Size (L × W × H)"} always>
            {sizeOf(subject)}
          </Fact>
          <Fact label="Packed in" always>
            {packedInWords(subject, desk.packagingTypes)}
          </Fact>
          {subject.packaging_level && <Ships subject={subject} desk={desk} />}
          {subject.packaging_level && <WayUp subject={subject} desk={desk} />}
          {carton && (
            <Fact label="Holds" always>
              {(() => {
                // A box-sold item's carton holds boxes (D218).
                const said = holdsInWords(item.packing, item.unit.level === "inner" ? unitWord(item) : "pack");
                return item.packing?.inners_per_carton != null ? said : <Faint>{said}</Faint>;
              })()}
            </Fact>
          )}
        </Facts>
        {/* With the camera open its photos are shown there, once. */}
        {open !== "photos" && <Photos subject={subject} photos={photos} desk={null} everySide={sides} />}
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
        {movable(item, subject) && (
          <Button size="sm" variant="ghost" disabled={desk.busy} onClick={() => setMoving(true)}>
            Move…
          </Button>
        )}
      </Toolbar>
      {moving && <MoveDialog item={item} subject={subject} desk={desk} onClose={() => setMoving(false)} />}
      {open === "weigh" && <WeighForm item={item} subject={subject} desk={desk} />}
      {open === "measure" && <MeasureForm item={item} subject={subject} desk={desk} />}
      {open === "photos" && (
        <div className={s.form}>
          <PackedIn subject={subject} desk={desk} />
          <p className={s.note}>
            {isBox(subject)
              ? "Take each side in turn, then its label. Each photo sends while you take the next."
              : isRound(subject)
                ? "Take its side square on, then its lid from above, then its label or a close-up if they help. Each sends while you take the next."
                : "Take its photo, then its back, label or a close-up if they help. Each sends while you take the next."}
            {handheld() && isBox(subject) && " They are cut to their faces at a computer, under Photos to crop."}
          </p>
          <NextSide subject={subject} desk={desk} order={facesToAsk(subject, sides)} />
          {!isBox(subject) && !isRound(subject) && !sides && (
            <div>
              <Button size="sm" onClick={() => setSides(true)}>
                Take every side as well
              </Button>
            </div>
          )}
          <Photos subject={subject} photos={photos} desk={desk} everySide={sides} />
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
function WeighForm({ item, subject, desk }: { item: ItemView; subject: CaptureSubject; desk: PropertiesDesk }) {
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
        {isOwnCarton(subject) && <HoldsField desk={desk} item={item} />}
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
/** What a round thing is measured by (D213): its widths, its whole height, and
 *  how far down from the rim it stays straight before it tapers. */
const ROUND_FIELDS = [
  { field: "top", label: "Across the top", hint: "At the rim" },
  { field: "base", label: "Across the base", hint: "Blank if straight" },
  { field: "height", label: "Height", hint: "With the lid on" },
  { field: "topHeight", label: "Top part’s height", hint: "Straight, under the rim" },
] as const;

function MeasureForm({ item, subject, desk }: { item: ItemView; subject: CaptureSubject; desk: PropertiesDesk }) {
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
        {isOwnCarton(subject) && <HoldsField desk={desk} item={item} />}
      </div>
      {!f.noDimensions && isRound(subject) && (
        <div className={s.dimensions}>
          {ROUND_FIELDS.map(({ field, label, hint }) => (
            <TextField
              key={field}
              label={label}
              hint={hint}
              inputMode="decimal"
              autoComplete="off"
              trailing="cm"
              value={f[field]}
              onChange={(e) => desk.type(field, e.target.value)}
            />
          ))}
        </div>
      )}
      {!f.noDimensions && !isRound(subject) && (
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
            {f.noDimensions ? "It has a size after all" : "It has no size to measure"}
          </Button>
        </div>
      )}
      <PackedIn subject={subject} desk={desk} />
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
function HoldsField({ desk, item }: { desk: PropertiesDesk; item: ItemView }) {
  // Sold by the pack, its carton is so many packs, and a pack so many (D218).
  if (item.unit.level === "inner") {
    const word = unitWord(item).toLowerCase();
    const many = /(x|s|sh|ch)$/.test(word) ? `${word}es` : `${word}s`;
    const read = readHoldsFor("inner", desk.holds, desk.per, item.packing);
    return (
      <>
        <div className={s.holds}>
          <TextField
            label={`How many ${many} in it`}
            hint="The whole carton"
            error={"problem" in read && desk.holds.trim() !== "" ? read.problem : undefined}
            inputMode="numeric"
            autoComplete="off"
            trailing={many}
            value={desk.holds}
            onChange={(e) => desk.typeHolds(e.target.value)}
          />
        </div>
        <div className={s.holds}>
          <TextField
            label={`Each ${word} holds`}
            hint="Blank if not counted"
            inputMode="numeric"
            autoComplete="off"
            trailing="× each"
            value={desk.per}
            onChange={(e) => desk.typePer(e.target.value)}
          />
        </div>
      </>
    );
  }
  // The whole carton's count, and the packs worked out from it (D185).
  const read = readHolds(desk.holds, desk.per);
  const packs = "problem" in read || read.per === null || read.holds === null ? null : read;
  const wrong = "problem" in read && desk.per.trim() !== "" && desk.holds.trim() !== "" ? read.problem : undefined;
  return (
    <>
      <div className={s.holds}>
        <TextField
          label="How many in it"
          hint="The whole carton"
          inputMode="numeric"
          autoComplete="off"
          trailing="× each"
          value={desk.holds}
          onChange={(e) => desk.typeHolds(e.target.value)}
        />
      </div>
      <div className={s.holds}>
        <TextField
          label="In packs of"
          hint={packs ? `That’s ${packs.holds!.toLocaleString()} ${packs.holds === 1 ? "pack" : "packs"} of ${packs.per!.toLocaleString()}` : "Blank if loose"}
          error={wrong}
          inputMode="numeric"
          autoComplete="off"
          trailing="× each"
          value={desk.per}
          onChange={(e) => desk.typePer(e.target.value)}
        />
      </div>
    </>
  );
}

/** Whether this variant stands for the item's carton, and choosing it (D184). */
function VariantChoice({ item, subject, desk }: { item: ItemView; subject: CaptureSubject; desk: PropertiesDesk }) {
  const carton = item.subjects.find((s) => s.item_id === item.item_id && s.packaging_level === "carton");
  if (!carton) return null;
  const chosen = carton.variant_lot_id === subject.lot_id;
  return chosen ? (
    <Button size="sm" disabled={desk.busy} onClick={() => void desk.chooseVariant(null)}>
      The carton · undo
    </Button>
  ) : (
    <Button size="sm" disabled={desk.busy} onClick={() => void desk.chooseVariant(subject.lot_id)}>
      Use for the carton
    </Button>
  );
}

/**
 * Another variant: the same product in a carton that looks different, a
 * different printing or factory, named by what is on the carton (D182). It
 * gets a card of its own to photograph and measure.
 */
function AddVariant({ desk }: { desk: PropertiesDesk }) {
  const [code, setCode] = useState("");
  const [open, setOpen] = useState(false);
  if (!open) {
    return (
      <div>
        <Button size="sm" onClick={() => setOpen(true)}>
          Add a variant
        </Button>
      </div>
    );
  }
  return (
    <Card title="Add a variant" description="The same product in a carton that looks different. Name it by what is printed on it.">
      <form
        className={s.fields}
        onSubmit={(e) => {
          e.preventDefault();
          void desk.addVariant(code).then((named) => {
            if (named) {
              setCode("");
              setOpen(false);
            }
          });
        }}
      >
        <div className={s.grow}>
          <TextField label="Name" autoComplete="off" autoFocus placeholder="O/N 66081, made in India" value={code} onChange={(e) => setCode(e.target.value)} />
        </div>
        <div className={s.formActions}>
          <Button onClick={() => setOpen(false)}>Cancel</Button>
          <Button type="submit" variant="primary" loading={desk.busy} disabled={!code.trim()}>
            Add
          </Button>
        </div>
      </form>
    </Card>
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
  everySide,
}: {
  subject: CaptureSubject;
  photos: Map<string, SubjectPhoto>;
  desk: PropertiesDesk | null;
  /** A thing asked for every side as well (D191). */
  everySide: boolean;
}) {
  const box = isBox(subject);
  const asked = facesToAsk(subject, everySide || BOX_FACES.some((f) => f !== "front" && f !== "back" && photos.has(f)));
  const sides: Partial<Record<BoxFace, string>> = {};
  for (const face of BOX_FACES) {
    const photo = photos.get(face);
    if (photo) sides[face] = shown(photo);
  }
  const showBox = box && (desk !== null || Object.keys(sides).length > 1);
  // On the box, the sides need no tiles of their own until there is a camera.
  const tiles = asked.filter((face) => (desk ? true : photos.has(face) && !(showBox && face !== "label")));
  const last = desk?.taken[desk.taken.length - 1];
  const facing = last && last !== "label" && last !== "detail" ? last : null;

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
                {photo?.same_as && <Faint>Same as {photo.same_as}</Faint>}
                {desk?.sending[face] === "sending" && <Faint>Sending…</Faint>}
                {desk?.sending[face] === "failed" && (
                  <Button size="sm" onClick={() => desk.resend(face)}>
                    Send again
                  </Button>
                )}
                {desk && <Shutter face={face} name={name} subject={subject} desk={desk} taken={desk.taken.includes(face)} />}
                {desk && photo && !photo.same_as && box && (
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

/** What it is packed in, in GS1's words, and whose saying it is (D191). */
function packedInWords(subject: CaptureSubject, types: PackagingType[]): ReactNode {
  if (!subject.packed_in) return <Faint>Not said</Faint>;
  const name = types.find((t) => t.code === subject.packed_in)?.name ?? subject.packed_in;
  if (subject.packed_in_source === "style") return `${name} (the family's)`;
  if (subject.packed_in_source === "item") return `${name} (the item's carton)`;
  return name;
}

/**
 * Whether it goes to the carrier as it is or into a box (D196), whose saying
 * that is, and one press to say the other. Unsaid, a carton ships as it is and
 * an each or an inner pack goes into a box; a roll in its own box is the kind
 * of thing somebody says otherwise about.
 */
function Ships({ subject, desk }: { subject: CaptureSubject; desk: PropertiesDesk }) {
  const words = subject.ships_as_is ? "As it is" : "In a box";
  const whose =
    subject.ships_as_is_source === "style"
      ? " (the family's)"
      : subject.ships_as_is_source === "item"
        ? " (the item's carton)"
        : subject.ships_as_is_source === "default"
          ? " unless said"
          : "";
  return (
    <Fact label="Ships" always>
      <span className={s.ships}>
        {subject.ships_as_is_source === "own" ? words : <Faint>{`${words}${whose}`}</Faint>}
        <Button size="sm" variant="ghost" disabled={desk.busy} onClick={() => void desk.shipAsIs(subject, !subject.ships_as_is)}>
          {subject.ships_as_is ? "Goes in a box" : "Ships as it is"}
        </Button>
      </span>
    </Fact>
  );
}

/**
 * Whether it must stay the way up it stands (D200): a box of bottles, a carton
 * printed "this way up". Unsaid, any way up will do, and the bench's
 * suggestion lays it on its biggest side.
 */
function WayUp({ subject, desk }: { subject: CaptureSubject; desk: PropertiesDesk }) {
  const words = subject.upright ? "This way up" : "Any way up";
  const whose =
    subject.upright_source === "style"
      ? " (the family's)"
      : subject.upright_source === "item"
        ? " (the item's carton)"
        : subject.upright_source === "default"
          ? " unless said"
          : "";
  return (
    <Fact label="Way up" always>
      <span className={s.ships}>
        {subject.upright_source === "own" ? words : <Faint>{`${words}${whose}`}</Faint>}
        <Button size="sm" variant="ghost" disabled={desk.busy} onClick={() => void desk.keepUpright(subject, !subject.upright)}>
          {subject.upright ? "Any way up" : "Keep this way up"}
        </Button>
      </span>
    </Fact>
  );
}

/**
 * What it is packed in (D191): GS1's common types as buttons, the rest in a
 * list. A type without six sides is photographed as a thing, not a box.
 */
function PackedIn({ subject, desk }: { subject: CaptureSubject; desk: PropertiesDesk }) {
  const types = desk.packagingTypes;
  if (types.length === 0) return null;
  const own = subject.packed_in_source === "own" ? subject.packed_in : null;
  const rest = types.filter((t) => t.common === null);
  return (
    <div className={s.packedIn}>
      <span className={s.fieldLabel}>Packed in{subject.packed_in && !own ? `: ${packedInWords(subject, types) as string}` : ""}</span>
      <div className={s.packedInChoices} role="group" aria-label="Packed in">
        {types
          .filter((t) => t.common !== null)
          .map((t) => (
            <Button
              key={t.code}
              size="sm"
              aria-pressed={own === t.code}
              title={t.definition}
              disabled={desk.busy}
              onClick={() => void desk.packIn(subject, t.code)}
            >
              {t.name}
            </Button>
          ))}
      </div>
      <Select
        size="sm"
        aria-label="Packed in, other"
        placeholder="Something else…"
        value={own && rest.some((t) => t.code === own) ? own : undefined}
        disabled={desk.busy}
        onValueChange={(code) => void desk.packIn(subject, code)}
        options={rest.map((t) => ({ value: t.code, label: t.name }))}
      />
    </div>
  );
}

/** The side a side is often printed like, taken before it in the walk round. */
const OPPOSITE: Partial<Record<Face, Face>> = { back: "front", left: "right", bottom: "top" };

/**
 * The next side to take, as one big button, and Skip for a side there is no
 * getting at: a box photographed in one walk round it, a tap a side, the
 * photographs sending behind it (D181).
 */
function NextSide({ subject, desk, order }: { subject: CaptureSubject; desk: PropertiesDesk; order: readonly Face[] }) {
  const [skipped, setSkipped] = useState<Face[]>([]);
  const next = order.find((f) => !desk.taken.includes(f) && !skipped.includes(f));
  const done = order.filter((f) => desk.taken.includes(f)).length;
  const on = Object.values(desk.sending).filter((v) => v === "sending").length;
  // A side printed like its opposite, already taken, is said rather than shot (D183).
  const like = next ? OPPOSITE[next] : undefined;
  const copy = like && desk.taken.includes(like) ? like : undefined;
  return (
    <div className={s.nextSide}>
      {next ? (
        <>
          <Shutter face={next} name={faceName(next, subject)} subject={subject} desk={desk} taken={false} big />
          {copy && isBox(subject) && <Button onClick={() => desk.same(subject, next, copy)}>Same as {copy}</Button>}
          <Button onClick={() => setSkipped((k) => [...k, next])}>Skip</Button>
        </>
      ) : (
        <span>{isBox(subject) ? "Every side taken." : "All taken."}</span>
      )}
      <Faint>
        {done} of {order.length} taken{on > 0 ? ` · sending ${on}` : ""}
      </Faint>
    </div>
  );
}

/** A label dressed as a button around a hidden file input: a button cannot open a camera. */
function Shutter({
  face,
  name,
  subject,
  desk,
  taken,
  big = false,
}: {
  face: Face;
  name: string;
  subject: CaptureSubject;
  desk: PropertiesDesk;
  taken: boolean;
  /** The next side, as the one button to press. */
  big?: boolean;
}) {
  return (
    <label className={cx(s.shutter, big && s.shutterBig)} aria-disabled={desk.busy || undefined}>
      <span className={s.hidden}>Take the {name.toLowerCase()}</span>
      <Camera aria-hidden />
      <span aria-hidden="true">{big ? `Take the ${name.toLowerCase()}` : taken ? "Again" : "Take"}</span>
      <input
        type="file"
        accept="image/*"
        capture="environment"
        disabled={desk.busy}
        onChange={(e) => {
          const file = e.currentTarget.files?.[0];
          // Cleared so the same file twice still fires: a retake is a new row (D132).
          e.currentTarget.value = "";
          if (file) desk.attach(subject, face, file);
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
        <figcaption className={s.caption}>
          {picture.source === "variant"
            ? "Photo of a variant"
            : item.style
              ? `Photo of the ${item.style.code} family`
              : "Photo of its family"}
        </figcaption>
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
  if (s.diameter_mm !== null) return roundSize(s);
  const d = [s.length_mm, s.width_mm, s.height_mm];
  if (d.some((v) => v !== null)) return `${d.map((v) => (v === null ? "?" : centimetres(v))).join(" × ")} cm`;
  return <Faint>{s.dimensions_absent ? "No size" : "Not measured"}</Faint>;
}

/** A round thing's size: "30 cm across, 25 at the base, 40 tall, straight for 8". */
function roundSize(s: CaptureSubject): string {
  const parts = [`${centimetres(s.diameter_mm ?? 0)} cm across`];
  if (s.base_diameter_mm !== null && s.base_diameter_mm !== s.diameter_mm) parts.push(`${centimetres(s.base_diameter_mm)} at the base`);
  if (s.height_mm !== null) parts.push(`${centimetres(s.height_mm)} tall`);
  if (s.top_height_mm !== null) parts.push(`straight for ${centimetres(s.top_height_mm)}`);
  return parts.join(", ");
}

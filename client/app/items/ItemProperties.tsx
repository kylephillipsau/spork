import { Suspense, lazy, useState, type ReactNode } from "react";
import { Barcode, Camera, ChevronLeft, ChevronRight, Crop, ImageOff, Images, Ruler, Scale } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  Checkbox,
  Dialog,
  Drawer,
  EmptyState,
  Fact,
  Facts,
  Link,
  List,
  ListItem,
  Section,
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
import type { CaptureSubject, FamilyMember, ItemTags, ItemView, PackagingType, Picture, RecordedCard, SubjectPhoto } from "@domain/types";
import { Thumb } from "@app/common/Thumb";
import { Faint, dateTime, sentence, shortDate } from "@app/common/cells";
import { centimetres, kg } from "@app/common/format";

import { BOX_FACES, boxSize, faceName, facesToAsk, isBox, isRound, measuredAspect, type BoxFace } from "./box";
import { roundSize } from "./round";
import { handheld } from "./crop";
import { ArrangeSides } from "./ArrangeSides";
import { FaceCrop } from "./FaceCrop";
import {
  PRESENTATIONS,
  bindable,
  cartonHolds,
  isOwnCarton,
  nameOf,
  photosOf,
  presentationNeeded,
  presentationOffered,
  NO_HOLDS,
  holdsOf,
  isOwnPack,
  packHolds,
  plural,
  readHoldsTyped,
  singleOffer,
  singlesOf,
  subjectKey,
  type HoldsTyped,
  unitWord,
  levelName,
  weighable,
  type Face,
  shown,
} from "./subjects";

// three.js is its own chunk, fetched the first time a box is shown.
const BoxView = lazy(() => import("./BoxView"));
const RoundView = lazy(() => import("./RoundView"));
import { useItemProperties, type Action, type PropertiesDesk, type SoldAs } from "./useItemProperties";
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
                : subject.packaging_level === "inner"
                  ? "Comes in a pack or inner box?"
                  : singleOffer(item)}
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
  const [choosing, setChoosing] = useState(false);
  return (
    <div className={s.summary}>
      <div className={s.figure}>
        <Photo key={item.item_id} item={item} />
        {desk && choices(item).length > 0 && (
          <div>
            <Button size="sm" variant="ghost" disabled={desk.busy} onClick={() => setChoosing(true)}>
              Main picture…
            </Button>
          </div>
        )}
        {desk && choosing && <MainPictureDialog item={item} desk={desk} onClose={() => setChoosing(false)} />}
      </div>
      <Facts columns={1}>
        {artNo(item) && (
          <Fact label="Art No.">
            <span className={s.code}>{artNo(item)}</span>
          </Fact>
        )}
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
          <Family item={item} desk={d} />
          <NetSuiteSays item={item} desk={d} />
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
 * for, shown or offered, other than the card itself. A pack is only a
 * definite thing under a case pack (D23); a carton is said by the move when
 * nothing has said one (D232), so a carton measured as the product goes on
 * its carton whatever is on file.
 */
function destinations(item: ItemView, s: CaptureSubject): Level[] {
  const there = new Set(item.subjects.filter((x) => ownLevel(item, x)).map((x) => x.packaging_level));
  return (["each", "inner", "carton"] as const).filter(
    (level) =>
      !(item.item_id === s.item_id && level === s.packaging_level) &&
      there.has(level) &&
      (level !== "inner" || item.packing !== null),
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

/** Whether a card's sides can be arranged (D243): a box of the item's own, two sides or more photographed. */
export function arrangeable(item: ItemView, s: CaptureSubject): boolean {
  return ownLevel(item, s) && isBox(s) && photosOf(item, s).size >= 2;
}

/** Whether a card's figures can be put right a figure at a time (D236): its own, and recorded. */
function correctable(item: ItemView, s: CaptureSubject): boolean {
  return ownLevel(item, s) && s.source === "own" && (s.gross_weight_g !== null || s.length_mm !== null);
}

/** Something recorded against the card itself: a family's figures or a variant's shown on it are not its own. */
export function recorded(s: CaptureSubject): boolean {
  if (s.source === "style" || s.source === "variant") return false;
  return s.gross_weight_g !== null || s.length_mm !== null || s.weight_absent || s.dimensions_absent || s.faces.length > 0;
}

/**
 * Move a card's figures and photos to the card they belong on (D219): a box
 * of ten weighed on the carton card, put on the box's; or a kit's part
 * measured on the kit's card, put on the part's own item (D222). Kept with
 * when and how they were taken; nothing is rewritten.
 *
 * **A carton measured as the product** (D232) moves onto its carton with
 * what the carton holds, asked here when nobody has said: the carton of
 * sixteen rolls weighed on the roll's card is the roll's carton, of sixteen.
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
  // What a carton holds, asked where nobody has said: the move says it (D232).
  const unsaid = !!into && to !== undefined && to !== "each" && into.packing?.inners_per_carton == null;
  const [holds, setHolds] = useState<HoldsTyped>(NO_HOLDS);
  const counts = into ? readHoldsTyped(holds, into) : null;
  const carton = unsaid && counts && !("problem" in counts) ? counts : undefined;
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
            disabled={!into || !to || taken.has(to) || (unsaid && !carton)}
            onClick={() =>
              into && to && void desk.refile(subject, { item: into.item_id, level: to }, name ?? to, carton).then((ok) => ok && onClose())
            }
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
        {into && unsaid && !taken.has(to) && (
          <>
            <div className={s.fields}>
              <HoldsField
                item={into}
                desk={{ holds, typeHolds: (next) => setHolds((h) => ({ ...h, ...next })) }}
              />
            </div>
            <p className={s.note}>
              {counts && "problem" in counts
                ? counts.problem
                : "Nobody has said what its carton holds. Say it here, or leave it blank to say later."}
            </p>
          </>
        )}
      </Stack>
    </Dialog>
  );
}

/**
 * The other items of its family (D228), each with what is on its cards, and
 * Match… to copy those to this item's. The colours of one brush come in the
 * same carton, the same size and weight, with the same sides but a label.
 * `onOpen` opens one beside the page; without it, its code is only said.
 */
export function Family({ item, desk, onOpen }: { item: ItemView; desk: PropertiesDesk; onOpen?: ((id: string) => void) | undefined }) {
  const [matching, setMatching] = useState<FamilyMember | null>(null);
  if (item.family.length === 0) return null;
  return (
    <Section title="Its family" count={item.family.length}>
      <Card padded={false}>
        <List label="Its family">
          {item.family.map((m) => (
            <ListItem
              key={m.item_id}
              title={
                <ItemLine
                  code={m.code}
                  description={m.description}
                  picture={m.picture}
                  onOpen={onOpen && (() => onOpen(m.item_id))}
                  note={m.cards.length > 0 ? m.cards.map((c) => `${levelName(c.level, item)} ${onCard(c)}`).join(" · ") : "Nothing measured or photographed"}
                />
              }
              badges={!m.active && <Badge tone="warning">Inactive</Badge>}
              action={
                m.cards.length > 0 ? (
                  <Button size="sm" disabled={desk.busy} onClick={() => setMatching(m)}>
                    Match…
                  </Button>
                ) : undefined
              }
            />
          ))}
        </List>
      </Card>
      {matching && <MatchDialog item={item} from={matching} desk={desk} onClose={() => setMatching(null)} />}
    </Section>
  );
}

/** What is on a card: "weighed, measured, 7 photos". */
function onCard(c: RecordedCard): string {
  const photos = c.faces > 0 && `${c.faces} ${c.faces === 1 ? "photo" : "photos"}`;
  return [c.weighed && "weighed", c.measured && "measured", photos].filter(Boolean).join(", ");
}

/**
 * Copy what another of the family has on its cards to the same cards of this
 * item (D228), with when and how each was taken; the other keeps its own. A
 * card of this item's with records of its own is left unticked: ticked, the
 * newer of each stays. A side that differs is then photographed again.
 */
export function MatchDialog({
  item,
  from,
  desk,
  onClose,
}: {
  item: ItemView;
  from: FamilyMember;
  desk: PropertiesDesk;
  onClose: () => void;
}) {
  const own = new Set(item.subjects.filter((x) => ownLevel(item, x) && recorded(x)).map((x) => x.packaging_level));
  const [levels, setLevels] = useState<Set<Level>>(() => new Set(from.cards.map((c) => c.level).filter((l) => !own.has(l))));
  const picked = from.cards.map((c) => c.level).filter((l) => levels.has(l));
  const tick = (level: Level, on: boolean) =>
    setLevels((was) => {
      const next = new Set(was);
      if (on) next.add(level);
      else next.delete(level);
      return next;
    });
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && !desk.busy && onClose()}
      width={460}
      title={`Match from ${from.code}`}
      description={`What ${from.code} has on these cards is copied to the same cards of ${item.code}, with when and how it was taken. ${from.code} keeps its own.`}
      footer={
        <>
          <Button onClick={onClose} disabled={desk.busy}>
            Cancel
          </Button>
          <Button
            variant="primary"
            loading={desk.busy}
            disabled={picked.length === 0}
            onClick={() => void desk.matchFamily(from, picked).then((ok) => ok && onClose())}
          >
            Match
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
        <ItemLine code={from.code} description={from.description} picture={from.picture} />
        {from.cards.map((c) => (
          <Checkbox
            key={c.level}
            label={
              <>
                {levelName(c.level, item)} <Faint>· {onCard(c)}</Faint>
              </>
            }
            checked={levels.has(c.level)}
            onCheckedChange={(on) => tick(c.level, on)}
          />
        ))}
        {picked.some((l) => own.has(l)) && (
          <p className={s.note}>{item.code} has figures or photos of its own there: the newer of each stays.</p>
        )}
        <p className={s.note}>Then photograph again any side that differs, such as a label of another colour.</p>
      </Stack>
    </Dialog>
  );
}

/**
 * What one of it is, and what it comes in (D218): "Box of 100 · in cartons of
 * 10 boxes". Whose word it is, when Spork said it rather than NetSuite.
 */
export function soldAs(item: ItemView): ReactNode {
  // A pair not packed as one is two of the single one (D233).
  const k = singlesOf(item);
  const unit = item.unit.level === "each" && k > 1 ? `${unitWord(item)}, ${k} single` : levelName(item.unit.level, item);
  const carton =
    item.unit.level !== "carton" && item.packing?.inners_per_carton
      ? levelName("carton", item).replace(/^(\S+) of/, (_, word: string) => `in ${plural(word, 2)} of`)
      : null;
  return (
    <>
      {unit}
      {carton && <Faint> · {carton}</Faint>}
      {item.unit.said ? <Faint> · said in Spork</Faint> : !item.unit.netsuite_unit && <Faint> · NetSuite doesn’t say</Faint>}
    </>
  );
}

const SOLD_AS: (SoldAs & { value: string; label: string; hint: string })[] = [
  { value: "each", level: "each", quantity: 1, label: "Single item", hint: "One is one thing: a catalogue, a roll, a pair of safety glasses" },
  { value: "pair", level: "each", quantity: 2, label: "Pair", hint: "One is two single ones: a pair of boots or gloves" },
  { value: "inner", level: "inner", quantity: 1, label: "Pack or box", hint: "One is a box or pack of several: a box of 100 earplugs" },
  { value: "carton", level: "carton", quantity: 1, label: "Carton", hint: "One is a whole carton: gloves by the carton of 1,000" },
];

/** Which of `SOLD_AS` it is: a pair, packed as one or not, is a pair (D233). */
function soldAsValue(item: ItemView): string {
  return singlesOf(item) === 2 ? "pair" : item.unit.level;
}

/** One of `SOLD_AS` by its value, as the unit write takes it. */
function soldAsOf(value: string): SoldAs {
  const o = SOLD_AS.find((x) => x.value === value) ?? SOLD_AS[0]!;
  return { level: o.level, quantity: o.quantity };
}

/**
 * What one of it is, chosen (D218): in Sold as's own dialog, and in Correct…
 * beside the figures (D239). A carton that holds just one is the thing in
 * its box, sold as the carton.
 */
function SoldAsChoice({ item, value, onChange }: { item: ItemView; value: string; onChange: (value: string) => void }) {
  const chosen = SOLD_AS.find((o) => o.value === value) ?? SOLD_AS[0]!;
  const ofOne = chosen.value === "carton" && cartonHolds(item.packing) === 1;
  return (
    <>
      <Tabs
        aria-label="What one of it is"
        value={value}
        onValueChange={onChange}
        items={SOLD_AS.map((o) => ({ value: o.value, label: o.label }))}
      />
      <p className={s.note}>
        {ofOne ? "One is its carton, which holds just one: weighed, measured and packed as the carton" : chosen.hint}
      </p>
    </>
  );
}

/** Say which level is one in NetSuite (D218), over its Pack Unit. */
function SoldAsDialog({ item, desk, onClose }: { item: ItemView; desk: PropertiesDesk; onClose: () => void }) {
  const [value, setValue] = useState(soldAsValue(item));
  const chosen = soldAsOf(value);
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
            disabled={value === soldAsValue(item)}
            onClick={() => void desk.sayUnit(chosen.level, chosen.quantity).then((ok) => ok && onClose())}
          >
            Save
          </Button>
        </>
      }
    >
      <SoldAsChoice item={item} value={value} onChange={setValue} />
    </Dialog>
  );
}

/** Its article number, as NetSuite says it: the field read as one (D238). */
export function artNo(item: ItemView): string | null {
  return item.netsuite?.fields.find((f) => f.role === "art_no")?.value ?? null;
}

/** A picture an item could be shown by, and what it is. */
interface Choice {
  digest: string;
  label: string;
}

/**
 * The pictures an item could be shown by (D237): its own photographs, each
 * side as cut, its box drawing, and NetSuite's picture unless somebody said
 * it isn't this product.
 */
function choices(item: ItemView): Choice[] {
  const own = item.photos.flatMap((p) => {
    const subject = item.subjects.find((x) => subjectKey(x) === subjectKey(p));
    if (!subject || p.item_id !== item.item_id || p.same_as) return [];
    return [{ digest: shown(p), label: `${nameOf(subject, item)}, ${faceName(p.face as Face, subject).toLowerCase()}` }];
  });
  const drawn = item.box_picture ? [{ digest: item.box_picture.digest, label: "Box drawing" }] : [];
  const netsuite = item.netsuite?.picture && !item.netsuite.picture_not_it ? [{ digest: item.netsuite.picture, label: "NetSuite’s picture" }] : [];
  return [...own, ...drawn, ...netsuite];
}

/**
 * Choose the picture an item is shown by everywhere (D237): any photograph of
 * it, its box drawing, or NetSuite's picture; or none, to be shown as Spork
 * chooses, its own photograph before NetSuite's.
 */
function MainPictureDialog({ item, desk, onClose }: { item: ItemView; desk: PropertiesDesk; onClose: () => void }) {
  const [picked, setPicked] = useState<string | null>(item.main_picture);
  const options = choices(item);
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && !desk.busy && onClose()}
      width={560}
      title="Main picture"
      description="What it is shown by wherever it is listed: on the bench, the shelves and when picking."
      footer={
        <>
          <Button onClick={onClose} disabled={desk.busy}>
            Cancel
          </Button>
          <Button
            variant="primary"
            loading={desk.busy}
            disabled={picked === item.main_picture}
            onClick={() => void desk.sayPicture("main", picked).then((ok) => ok && onClose())}
          >
            Save
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
        <div className={s.pictures}>
          <button type="button" className={s.pictureChoice} aria-pressed={picked === null} onClick={() => setPicked(null)}>
            <span>As Spork chooses: its own photo, else NetSuite’s</span>
          </button>
          {options.map((o) => (
            <button
              key={o.digest}
              type="button"
              className={s.pictureChoice}
              aria-pressed={picked === o.digest}
              onClick={() => setPicked(o.digest)}
            >
              <img src={imageUrl(o.digest)} alt="" loading="lazy" />
              <span>{sentence(o.label)}</span>
            </button>
          ))}
        </div>
      </Stack>
    </Dialog>
  );
}

/** What each of NetSuite's fields is read as, in words (D238). */
export const NETSUITE_ROLES: { role: string; label: string }[] = [
  { role: "kept", label: "Kept" },
  { role: "shown", label: "Shown beside the code" },
  { role: "warning", label: "A warning" },
  { role: "note", label: "A note" },
  { role: "art_no", label: "Art No." },
  { role: "picture", label: "Its picture" },
  { role: "weight", label: "Weight" },
  { role: "length", label: "Length" },
  { role: "width", label: "Width" },
  { role: "height", label: "Height" },
  { role: "barcode", label: "A barcode" },
  { role: "per_carton", label: "How many in a carton" },
  { role: "per_inner", label: "How many in a pack" },
  { role: "inners_per_carton", label: "Packs in a carton" },
];

/** The order its fields are listed in: what tells it apart, what is compared, then the rest. */
export const ROLE_ORDER = ["art_no", "shown", "warning", "weight", "length", "width", "height", "barcode", "per_carton", "per_inner", "inners_per_carton", "note", "kept"];

/** Where a field read as this disagrees with Spork, said; none where it agrees or isn't compared. */
export function differs(ns: NonNullable<ItemView["netsuite"]>, role: string): string | null {
  if (role === "weight" && ns.weight_differs) return "Differs from what was weighed";
  if (["length", "width", "height"].includes(role) && ns.size_differs) return "Differs from what was measured";
  if (role === "barcode" && ns.barcode_differs) return "Not a barcode scanned here";
  if (["per_carton", "per_inner", "inners_per_carton"].includes(role) && ns.pack_differs) return "Not what its carton holds here";
  return null;
}

/**
 * What NetSuite says of an item (D237, D238), beside what Spork recorded and
 * never in its place: every field as NetSuite said it, with what it is read
 * as and since when, and where it disagrees with what was measured, scanned
 * and said here. A difference is put right in NetSuite, not here; what a
 * field means is set under Workspace › NetSuite fields.
 */
export function NetSuiteSays({ item, desk }: { item: ItemView; desk: PropertiesDesk }) {
  const ns = item.netsuite;
  if (!ns) return null;
  const fields = ns.fields
    .filter((f) => f.role !== "picture")
    .sort((a, b) => ROLE_ORDER.indexOf(a.role) - ROLE_ORDER.indexOf(b.role) || a.field.localeCompare(b.field));
  const said = (role: string) => NETSUITE_ROLES.find((r) => r.role === role)?.label ?? role;
  return (
    <Section title="What NetSuite says">
      <Card>
        <div className={s.summary}>
          {ns.picture && (
            <figure className={s.figure}>
              <img className={s.netsuitePicture} src={imageUrl(ns.picture)} alt={`${item.code}, as NetSuite pictures it`} />
              {ns.picture_not_it ? (
                <figcaption className={s.caption}>Said not to be this product, so shown to nobody</figcaption>
              ) : (
                item.main_picture !== ns.picture && (
                  <div>
                    <Button size="sm" variant="ghost" disabled={desk.busy} onClick={() => void desk.sayPicture("main", ns.picture)}>
                      Use as main picture
                    </Button>
                  </div>
                )
              )}
              <div>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={desk.busy}
                  onClick={() => void desk.sayPicture(ns.picture_not_it ? "is_it" : "not_it", ns.picture)}
                >
                  {ns.picture_not_it ? "It is this product" : "Not this product"}
                </Button>
              </div>
            </figure>
          )}
          <Facts columns={1}>
            {fields.map((f) => {
              const off = differs(ns, f.role);
              return (
                <Fact key={f.field} label={f.field}>
                  <span className={f.role === "art_no" || f.role === "barcode" ? s.code : undefined}>{f.value}</span>{" "}
                  {off && <Badge tone="warning">{off}</Badge>}
                  <Faint>
                    {" "}
                    · {said(f.role)}
                    {f.level ? `, the ${f.level === "inner" ? "pack" : f.level}’s` : ""} · since {shortDate(f.since)}
                  </Faint>
                </Fact>
              );
            })}
          </Facts>
        </div>
        <p className={s.note}>
          As NetSuite says it, for what it counts one of: its record of the product, not the product. Where it differs, put
          it right in NetSuite.
        </p>
      </Card>
    </Section>
  );
}

/**
 * What NetSuite says beside an item wherever it is listed (D237, D238): its
 * article number and the fields shown beside the code on one line, and its
 * warnings under it.
 */
export function ItemTagsLine({ tags }: { tags: ItemTags | null | undefined }) {
  if (!tags) return null;
  const line = [tags.art_no && `Art ${tags.art_no}`, ...tags.shown.map((f) => `${f.field} ${f.value}`)].filter(Boolean).join(" · ");
  return (
    <>
      {line && <span className={s.art}>{line}</span>}
      {tags.warnings.map((w) => (
        <span key={w} className={s.warning}>
          {w}
        </span>
      ))}
    </>
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
  note,
  tags,
  done = false,
}: {
  code: string;
  /** Said under the code; left out where the row has no room for it. */
  description?: string | undefined;
  picture: Picture | null;
  onOpen?: (() => void) | undefined;
  /** A word about it on this row, under what it is: which kit it is part of (D223). */
  note?: string | undefined;
  /** What NetSuite says beside it (D237, D238): its Art No., a colour or size, a warning. */
  tags?: ItemTags | null | undefined;
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
        <ItemTagsLine tags={tags} />
        {note && <span className={s.itemNote}>{note}</span>}
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
  const [arranging, setArranging] = useState(false);
  // An item's own carton is a box of so many of it (D178), and its pack too
  // (D234), but a pair packed as one, which holds its two (D233).
  const carton = isOwnCarton(subject);
  const pack = isOwnPack(subject) && !(item.unit.level === "inner" && singlesOf(item) > 1);
  const packSaid = (item.packing?.units_per_inner ?? 0) > 1;
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
              : pack && !packSaid && item.unit.level !== "inner"
                ? "Say how many are in a pack when you weigh or measure it"
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
          {pack && (
            <Fact label="Holds" always>
              {packHolds(item) ?? <Faint>Not said yet</Faint>}
              {packSaid && (
                <div>
                  <Button
                    size="sm"
                    aria-pressed={open === "holds"}
                    disabled={desk.busy}
                    onClick={() => (open === "holds" ? desk.close() : desk.show(subject, "holds"))}
                  >
                    Change
                  </Button>
                </div>
              )}
            </Fact>
          )}
          {carton && (
            <Fact label="Holds" always>
              {item.packing?.inners_per_carton != null ? holdsOf(item) : <Faint>{holdsOf(item)}</Faint>}
              {item.packing?.inners_per_carton != null && (
                <div>
                  <Button
                    size="sm"
                    aria-pressed={open === "holds"}
                    disabled={desk.busy}
                    onClick={() => (open === "holds" ? desk.close() : desk.show(subject, "holds"))}
                  >
                    Change
                  </Button>
                </div>
              )}
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
        {/* Sold one at a time in a carton of its own: the carton is what is measured (D239). */}
        {subject.is_unit && subject.packaging_level === "each" && cartonHolds(item.packing) === 1 && (
          <div className={s.ships}>
            <Faint>Its carton holds just one.</Faint>
            <Button size="sm" disabled={desk.busy} onClick={() => void desk.sayUnit("carton")}>
              Sold in its carton
            </Button>
          </div>
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
        {correctable(item, subject) && (
          <Button
            size="sm"
            variant="ghost"
            aria-pressed={open === "correct"}
            disabled={desk.busy}
            onClick={() => (open === "correct" ? desk.close() : desk.show(subject, "correct"))}
          >
            Correct…
          </Button>
        )}
        {movable(item, subject) && (
          <Button size="sm" variant="ghost" disabled={desk.busy} onClick={() => setMoving(true)}>
            Move…
          </Button>
        )}
        {arrangeable(item, subject) && (
          <Button size="sm" variant="ghost" disabled={desk.busy} onClick={() => setArranging(true)}>
            Arrange sides…
          </Button>
        )}
      </Toolbar>
      {moving && <MoveDialog item={item} subject={subject} desk={desk} onClose={() => setMoving(false)} />}
      {arranging && <ArrangeSides item={item} subject={subject} onClose={() => setArranging(false)} onSaved={desk.refresh} />}
      {open === "weigh" && <WeighForm item={item} subject={subject} desk={desk} />}
      {open === "measure" && <MeasureForm item={item} subject={subject} desk={desk} />}
      {open === "correct" && <MeasureForm item={item} subject={subject} desk={desk} correcting />}
      {open === "photos" && (
        <div className={s.form}>
          {/* A pack nobody has said is said with its first photo (D234). */}
          {asksPack(item, subject) && (
            <div className={s.fields}>
              <PackField desk={desk} item={item} />
            </div>
          )}
          <PackedIn subject={subject} desk={desk} />
          <p className={s.note}>
            {isBox(subject)
              ? "Take each side in turn, then its label. Each photo sends while you take the next."
              : isRound(subject)
                ? "Take its front square on, then turn it a quarter at a time for its right, back and left; then its lid from above and its base. Each sends while you take the next."
                : "Take its photo, then its back, label or a close-up if they help. Each sends while you take the next."}
            {handheld() && isBox(subject) && " They are cut to their faces at a computer, under Photos to crop."}
            {isRound(subject) && " They are wrapped round it at a computer, under Photos to crop."}
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
      {open === "holds" && <HoldsForm item={item} subject={subject} desk={desk} />}
    </Card>
  );
}

/**
 * What its carton or its pack holds, said again because it was said wrongly
 * (D229, D234): a thousand boxes typed for ten. The same carton, so what was
 * weighed, measured and photographed of it, and of what is in it, stays.
 */
function HoldsForm({ item, subject, desk }: { item: ItemView; subject: CaptureSubject; desk: PropertiesDesk }) {
  const pack = isOwnPack(subject);
  return (
    <form
      className={s.form}
      onSubmit={(e) => {
        e.preventDefault();
        void desk.correctHolds(subject);
      }}
    >
      <div className={s.fields}>{pack ? <PackField desk={desk} item={item} /> : <HoldsField desk={desk} item={item} />}</div>
      <p className={s.note}>
        Puts right what was said of this {pack ? "pack" : "carton"}. What was weighed, measured and photographed stays.
      </p>
      <div className={s.formActions}>
        <Button onClick={desk.close}>Cancel</Button>
        <Button type="submit" variant="primary" loading={desk.busy} disabled={!(pack ? desk.holds.per : desk.holds.count).trim()}>
          Save
        </Button>
      </div>
    </form>
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
        {asksPack(item, subject) && <PackField desk={desk} item={item} />}
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

/** What a round thing is measured by (D213): its widths, its whole height, and
 *  how far down from the rim it stays straight before it tapers. */
const ROUND_FIELDS = [
  { field: "top", label: "Across the top", hint: "At the rim" },
  { field: "base", label: "Across the base", hint: "Blank if straight" },
  { field: "height", label: "Height", hint: "With the lid on" },
  { field: "topHeight", label: "Top part’s height", hint: "Straight, under the rim" },
] as const;

/**
 * Weight and size as one act (D133), in kilograms and centimetres as the
 * instruments read. A thing with no box says so rather than leaving it blank
 * (D138).
 *
 * `correcting`, the same fields filled with what is on file, to put right a
 * figure at a time with what its carton or pack holds (D236). A correction
 * keeps the arrangement it was measured in.
 */
function MeasureForm({
  item,
  subject,
  desk,
  correcting = false,
}: {
  item: ItemView;
  subject: CaptureSubject;
  desk: PropertiesDesk;
  correcting?: boolean | undefined;
}) {
  const f = desk.figures;
  const offered = presentationOffered(subject) && !correcting;
  const pairPacked = item.unit.level === "inner" && singlesOf(item) > 1;
  // What one of it is, put right with the figures (D239).
  const [soldAs, setSoldAs] = useState(soldAsValue(item));
  return (
    <form
      className={s.form}
      onSubmit={(e) => {
        e.preventDefault();
        void (correcting
          ? desk.correct(subject, soldAs === soldAsValue(item) ? undefined : soldAsOf(soldAs))
          : desk.measure(subject));
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
        {(correcting ? isOwnPack(subject) && !pairPacked : asksPack(item, subject)) && <PackField desk={desk} item={item} />}
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
      {correcting && (
        <div className={s.arrangement}>
          <span className={s.fieldLabel}>Sold as: what one of {item.code} is in NetSuite</span>
          <SoldAsChoice item={item} value={soldAs} onChange={setSoldAs} />
        </div>
      )}
      <PackedIn subject={subject} desk={desk} />
      {correcting && (
        <p className={s.note}>
          Only what you change is put right, as of when it was measured. The rest stays as it was. What it is packed in
          is said as you press it.
        </p>
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
          {correcting ? "Save corrections" : "Record"}
        </Button>
      </div>
    </form>
  );
}

/**
 * How many of the item one carton holds (D178): the carton and the item in it,
 * said together. Blank says nothing of the count.
 *
 * **Altogether, or in packs** (D185, D233): 140 in it, or 10 packs each
 * holding 12; counted in single ones or, for an item sold by the pair, in
 * pairs. However it is typed, the carton is kept as packs of single ones, and
 * what that comes to is said under it.
 */
function HoldsField({
  desk,
  item,
}: {
  desk: Pick<PropertiesDesk, "holds" | "typeHolds">;
  item: Pick<ItemView, "unit" | "packing">;
}) {
  const h = desk.holds;
  const k = singlesOf(item);
  // Sold by the box, its carton is so many boxes (D218); anything else's, packs.
  const word = item.unit.level === "inner" && k <= 1 ? unitWord(item).toLowerCase() : "pack";
  const packs = plural(word, 2);
  const read = readHoldsTyped(h, item);
  const typed = h.count.trim() !== "" || h.per.trim() !== "";
  const said =
    "problem" in read || read.holds === null
      ? null
      : `That’s ${holdsOf({ unit: item.unit, packing: { units_per_inner: read.per ?? 1, inners_per_carton: read.holds, effective_from: "" } })}`;
  const problem = typed && "problem" in read ? read.problem : undefined;
  const each = k > 1 ? undefined : "× each";
  return (
    <>
      <div className={s.holdsBy}>
        <Tabs
          aria-label="What it holds, counted"
          value={h.by}
          onValueChange={(v) => desk.typeHolds({ by: v === "packs" ? "packs" : "all" })}
          items={[
            { value: "all", label: "How many in it" },
            { value: "packs", label: `In ${packs}` },
          ]}
        />
      </div>
      {h.by === "all" ? (
        <div className={s.holds}>
          <TextField
            label="How many in it"
            hint={said ?? "The whole carton"}
            error={problem}
            inputMode="numeric"
            autoComplete="off"
            trailing={each}
            value={h.count}
            onChange={(e) => desk.typeHolds({ count: e.target.value })}
          />
        </div>
      ) : (
        <>
          <div className={s.holds}>
            <TextField
              label={`How many ${packs} in it`}
              inputMode="numeric"
              autoComplete="off"
              trailing={packs}
              value={h.count}
              onChange={(e) => desk.typeHolds({ count: e.target.value })}
            />
          </div>
          <div className={s.holds}>
            <TextField
              label={`Each ${word} holds`}
              hint={said ?? "Blank if not counted"}
              error={problem}
              inputMode="numeric"
              autoComplete="off"
              trailing={each}
              value={h.per}
              onChange={(e) => desk.typeHolds({ per: e.target.value })}
            />
          </div>
        </>
      )}
      <CountedIn desk={desk} item={item} />
    </>
  );
}

/**
 * What a pack or inner box of it holds (D234), on the pack's own card: so
 * many single ones or pairs. A pair packed as one holds its two, and is
 * never asked.
 */
function PackField({ desk, item }: { desk: Pick<PropertiesDesk, "holds" | "typeHolds">; item: Pick<ItemView, "unit" | "packing"> }) {
  const p = desk.holds.per.trim();
  const ok = /^\d+$/.test(p) && Number(p) >= 1;
  return (
    <>
      <div className={s.holds}>
        <TextField
          label="How many in a pack"
          hint={ok && desk.holds.in > 1 ? `That’s ${(Number(p) * desk.holds.in).toLocaleString()} single` : "The whole pack or inner box"}
          error={p && !ok ? "A whole number, 1 or more." : undefined}
          inputMode="numeric"
          autoComplete="off"
          trailing={singlesOf(item) > 1 ? undefined : "× each"}
          value={desk.holds.per}
          onChange={(e) => desk.typeHolds({ per: e.target.value })}
        />
      </div>
      <CountedIn desk={desk} item={item} />
    </>
  );
}

/** Whether counts are typed in single ones or pairs: asked only of an item sold by the pair (D233). */
function CountedIn({ desk, item }: { desk: Pick<PropertiesDesk, "holds" | "typeHolds">; item: Pick<ItemView, "unit"> }) {
  const k = singlesOf(item);
  if (k <= 1) return null;
  return (
    <div className={s.unit}>
      <Select
        label="Counted in"
        value={String(desk.holds.in)}
        onValueChange={(v) => desk.typeHolds({ in: Number(v) })}
        options={[
          { value: String(k), label: plural(unitWord(item), 2) },
          { value: "1", label: "single" },
        ]}
      />
    </div>
  );
}

/**
 * Whether weighing or measuring its pack asks what a pack holds (D234):
 * nobody has said, and it is no box sold as one, which may be measured
 * first, nor a pair packed as one.
 */
function asksPack(item: ItemView, subject: CaptureSubject): boolean {
  return isOwnPack(subject) && item.unit.level !== "inner" && (item.packing?.units_per_inner ?? 0) <= 1;
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
 * and a file picker at a desk. Beside it, From photos chooses one already
 * taken, from the phone's photos.
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
  // A round thing wrapped in its photographs is drawn as its tub (D240).
  const size = isRound(subject) ? roundSize(subject) : null;
  const wrapped = size && subject.wrap ? { size, wrap: subject.wrap } : null;
  const showBox = box && (desk !== null || Object.keys(sides).length > 1);
  // On the box or the tub, its sides need no tiles of their own until there is a camera.
  const onModel = (face: Face) => (showBox && face !== "label") || (wrapped !== null && face !== "label" && face !== "detail");
  const tiles = asked.filter((face) => (desk ? true : photos.has(face) && !onModel(face)));
  const last = desk?.taken[desk.taken.length - 1];
  const facing = last && last !== "label" && last !== "detail" ? last : null;

  if (!showBox && !wrapped && tiles.length === 0) return <Faint>No photos yet</Faint>;
  return (
    <div className={s.photos}>
      {showBox && (
        <Suspense fallback={<div className={s.box} />}>
          <BoxView faces={sides} size={boxSize(subject)} facing={facing} label={subject.code} />
        </Suspense>
      )}
      {wrapped && (
        <Suspense fallback={<div className={s.box} />}>
          <RoundView size={wrapped.size} wrap={wrapped.wrap} label={subject.code} />
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
                {desk && <FromPhotos faces={[face]} name={name} subject={subject} desk={desk} />}
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
export function packedInWords(subject: CaptureSubject, types: PackagingType[]): ReactNode {
  if (!subject.packed_in) return <Faint>Not said</Faint>;
  const type = types.find((t) => t.code === subject.packed_in);
  const named = type?.name ?? subject.packed_in;
  const name = shapeless(type) && subject.box_shaped ? `${named}, box-shaped` : named;
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
/** Whose saying a fact of a card is, after the fact: its family's, its item's carton's, or nobody's yet. */
function whose(source: string): string {
  if (source === "style") return " (the family's)";
  if (source === "item") return " (the item's carton)";
  if (source === "default") return " unless said";
  return "";
}

/** Whether it ships as it is (D196), in words, faint where it is not its own saying. */
export function shipsWords(subject: CaptureSubject): ReactNode {
  const words = subject.ships_as_is ? "As it is" : "In a box";
  return subject.ships_as_is_source === "own" ? words : <Faint>{`${words}${whose(subject.ships_as_is_source)}`}</Faint>;
}

/** Whether it stays the way up it stands (D200), in words, faint where it is not its own saying. */
export function wayUpWords(subject: CaptureSubject): ReactNode {
  const words = subject.upright ? "This way up" : "Any way up";
  return subject.upright_source === "own" ? words : <Faint>{`${words}${whose(subject.upright_source)}`}</Faint>;
}

function Ships({ subject, desk }: { subject: CaptureSubject; desk: PropertiesDesk }) {
  return (
    <Fact label="Ships" always>
      <span className={s.ships}>
        {shipsWords(subject)}
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
  return (
    <Fact label="Way up" always>
      <span className={s.ships}>
        {wayUpWords(subject)}
        <Button size="sm" variant="ghost" disabled={desk.busy} onClick={() => void desk.keepUpright(subject, !subject.upright)}>
          {subject.upright ? "Any way up" : "Keep this way up"}
        </Button>
      </span>
    </Fact>
  );
}

/** A type with no shape of its own (D239): a wrapper, shrink-wrap, a band; not a box, not round. */
function shapeless(type: PackagingType | undefined): boolean {
  return type !== undefined && !type.six_sided && !type.round;
}

/**
 * What it is packed in (D191): GS1's common types as buttons, the rest in a
 * list. A type without six sides is photographed as a thing, not a box,
 * unless a wrapping is said to be round a block with six flat sides (D239).
 */
function PackedIn({ subject, desk }: { subject: CaptureSubject; desk: PropertiesDesk }) {
  const types = desk.packagingTypes;
  if (types.length === 0) return null;
  const own = subject.packed_in_source === "own" ? subject.packed_in : null;
  const rest = types.filter((t) => t.common === null);
  const wrapped = shapeless(types.find((t) => t.code === subject.packed_in));
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
      {wrapped && (
        <div className={s.ships}>
          <Button
            size="sm"
            aria-pressed={subject.box_shaped}
            disabled={desk.busy}
            onClick={() => void desk.sayShape(subject, !subject.box_shaped)}
          >
            Box-shaped
          </Button>
          <Faint>
            {subject.box_shaped
              ? "Six flat sides: each photo is cut to its face, and it is drawn as a box."
              : "Wrapped round six flat sides? Then its photos are cut to their faces and it is drawn."}
          </Faint>
        </div>
      )}
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
  const left = order.filter((f) => !desk.taken.includes(f) && !skipped.includes(f));
  const next = left[0];
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
          <FromPhotos faces={left} name={faceName(next, subject)} subject={subject} desk={desk} />
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

/** The camera, for one side. */
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
    <FilePress
      label={`Take the ${name.toLowerCase()}`}
      text={big ? `Take the ${name.toLowerCase()}` : taken ? "Again" : "Take"}
      icon={<Camera aria-hidden />}
      camera
      big={big}
      disabled={desk.busy}
      chosen={([file]) => desk.attach(subject, face, file!)}
    />
  );
}

/**
 * Photos already taken, chosen from the phone's photos: one for one side, or,
 * for the sides still to take, several at once, each the next side in the walk
 * round in the order they were chosen.
 */
function FromPhotos({ faces, name, subject, desk }: { faces: readonly Face[]; name: string; subject: CaptureSubject; desk: PropertiesDesk }) {
  const one = faces.length === 1;
  return (
    <FilePress
      label={one ? `Choose a photo of the ${name.toLowerCase()}` : `Choose photos of the sides still to take, from the ${name.toLowerCase()} on`}
      text={one ? "Choose" : "From photos"}
      icon={<Images aria-hidden />}
      multiple={!one}
      disabled={desk.busy}
      chosen={(files) => files.slice(0, faces.length).forEach((file, i) => desk.attach(subject, faces[i]!, file))}
    />
  );
}

/**
 * A label dressed as a button around a hidden file input: a button can open
 * neither a camera nor the phone's photos. `camera` opens the rear camera on a
 * handheld; without it, the photos to choose from.
 */
function FilePress({
  label,
  text,
  icon,
  camera = false,
  multiple = false,
  big = false,
  disabled,
  chosen,
}: {
  label: string;
  text: string;
  icon: ReactNode;
  camera?: boolean;
  multiple?: boolean;
  big?: boolean;
  disabled: boolean;
  chosen: (files: File[]) => void;
}) {
  return (
    <label className={cx(s.shutter, big && s.shutterBig)} aria-disabled={disabled || undefined}>
      <span className={s.hidden}>{label}</span>
      {icon}
      <span aria-hidden="true">{text}</span>
      <input
        type="file"
        accept="image/*"
        {...(camera ? { capture: "environment" as const } : {})}
        multiple={multiple}
        disabled={disabled}
        onChange={(e) => {
          const files = Array.from(e.currentTarget.files ?? []);
          // Cleared so the same file twice still fires: a retake is a new row (D132).
          e.currentTarget.value = "";
          if (files.length > 0) chosen(files);
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
          {picture.source === "netsuite"
            ? "NetSuite’s picture"
            : picture.source === "variant"
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
export function provenance(s: CaptureSubject): string {
  const parts: string[] = [];
  if (s.method) parts.push(`${METHOD[s.method] ?? sentence(s.method)}${s.observed_at ? ` ${dateTime(s.observed_at)}` : ""}`);
  if (s.source === "style") parts.push(`the ${s.style_code ?? "family"}’s figures`);
  if (s.source === "mixed") parts.push(`partly the ${s.style_code ?? "family"}’s figures`);
  return parts.length ? parts.join(" · ") : "Nothing recorded yet";
}

export function weightOf(s: CaptureSubject): ReactNode {
  if (s.gross_weight_g !== null) return kg(s.gross_weight_g);
  return <Faint>{s.weight_absent ? "None, it was said" : "Not weighed"}</Faint>;
}

export function sizeOf(s: CaptureSubject): ReactNode {
  if (s.diameter_mm !== null) return roundWords(s);
  const d = [s.length_mm, s.width_mm, s.height_mm];
  if (d.some((v) => v !== null)) return `${d.map((v) => (v === null ? "?" : centimetres(v))).join(" × ")} cm`;
  return <Faint>{s.dimensions_absent ? "No size" : "Not measured"}</Faint>;
}

/** A round thing's size: "30 cm across, 25 at the base, 40 tall, straight for 8". */
function roundWords(s: CaptureSubject): string {
  const parts = [`${centimetres(s.diameter_mm ?? 0)} cm across`];
  if (s.base_diameter_mm !== null && s.base_diameter_mm !== s.diameter_mm) parts.push(`${centimetres(s.base_diameter_mm)} at the base`);
  if (s.height_mm !== null) parts.push(`${centimetres(s.height_mm)} tall`);
  if (s.top_height_mm !== null) parts.push(`straight for ${centimetres(s.top_height_mm)}`);
  return parts.join(", ");
}

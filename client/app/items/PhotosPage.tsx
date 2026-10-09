import { Check, Crop, ImageOff } from "lucide-react";

import { Suspense, lazy, useState } from "react";

import { Alert, Button, Card, Dialog, EmptyState, Link, Page, PageHeader, Skeleton, Stack, Tabs, TextField, Toolbar, Spacer, cx } from "@ui/index";
import { imageUrl } from "@domain/api";
import { Faint } from "@app/common/cells";

import { FaceCrop } from "./FaceCrop";
import type { Face } from "./subjects";
import type { QueueDesk, Queued } from "./usePhotoQueue";
import type { WrapDesk, Wrapping } from "./useWrapQueue";
import s from "./items.module.css";

// three.js is its own chunk, fetched the first time a tub is shown.
const RoundView = lazy(() => import("./RoundView"));

/**
 * The photographs nobody has cut to their faces yet, at a computer (D181).
 *
 * A phone takes them and has not the memory to find a face, so the computer
 * finds each one, the oldest first, and draws where it put the corners, and
 * checks them against the face as measured (D214). What is right is confirmed
 * with one press; what is not opens in the crop screen. Nothing is kept until
 * the person confirms.
 *
 * Round things are wrapped in their photographs here too (D240), each shown as
 * it would look before it is saved.
 */
export function PhotosPage({ desk, wraps }: { desk: QueueDesk; wraps?: WrapDesk | undefined }) {
  const read = desk.read;
  const open = desk.queued.filter((q) => q.state !== "saved");
  const toWrap = wraps?.wraps.filter((w) => w.state !== "saved") ?? [];
  const looked = open.filter((q) => q.state !== "waiting" && q.state !== "finding").length;
  const still = open.length - looked;
  const [moving, setMoving] = useState<Queued | null>(null);

  return (
    <Page>
      <PageHeader
        title="Photos to crop"
        description="Photos not yet cut to their faces. The computer finds each face and checks it against the measured size; confirm it, or adjust it first."
      />
      {read.kind === "failed" ? (
        <Alert tone="danger">{read.message}</Alert>
      ) : read.kind === "loading" ? (
        <Card>
          <Stack gap={3}>
            <Skeleton width="40%" />
            <Skeleton width="70%" />
          </Stack>
        </Card>
      ) : open.length === 0 && toWrap.length === 0 ? (
        <Card>
          <EmptyState icon={<Crop />} title="Nothing to crop" description="Every photo has been cut to its face." />
        </Card>
      ) : (
        <Stack gap={4}>
          {wraps && toWrap.length > 0 && <WrapList desk={wraps} wraps={toWrap} />}
          {desk.phone && (
            <Alert tone="info">Open this at a computer to have the faces found. Here, each photo opens to have its corners dragged.</Alert>
          )}
          {desk.crop.problem && !desk.adjusting && (
            <Alert tone="danger" onDismiss={desk.crop.dismiss}>
              {desk.crop.problem}
            </Alert>
          )}
          {open.length > 0 && (
          <Card padded={false}>
            <Toolbar>
              <Faint>
                {still > 0 && !desk.phone
                    ? `Finding faces: ${looked} of ${open.length} done`
                    : `${open.length} ${open.length === 1 ? "photo" : "photos"} to check`}
              </Faint>
              <Spacer />
              {open.some((q) => q.state === "failed") && (
                <Button onClick={desk.again}>Try again</Button>
              )}
            </Toolbar>
            <ul className={s.queue} aria-label="Photos to crop">
              {open.map((q) => (
                <QueuedPhoto key={q.photo.image_id} q={q} desk={desk} move={() => setMoving(q)} />
              ))}
            </ul>
          </Card>
          )}
        </Stack>
      )}
      {moving && <MoveDialog desk={desk} q={moving} onClose={() => setMoving(null)} />}
      {desk.adjusting?.subject && (
        <FaceCrop
          key={desk.adjusting.photo.image_id}
          cropping={{
            subject: desk.adjusting.subject,
            face: desk.adjusting.photo.face as Face,
            image_id: desk.adjusting.photo.image_id,
            digest: desk.adjusting.photo.digest,
            corners: desk.adjusting.corners,
          }}
          name={desk.adjusting.name}
          aspect={desk.adjusting.aspect}
          desk={desk.crop}
        />
      )}
    </Page>
  );
}

const SAID: Record<Queued["state"], string> = {
  waiting: "Waiting",
  finding: "Finding the face…",
  found: "Found",
  turned: "Found a quarter turn out: adjust it",
  missed: "No face found: adjust it",
  failed: "Could not be read here",
  saving: "Saving…",
  saved: "Saved",
};

/** Whose photograph it is: an item's, at a level; a family's carton; a variant (D190). */
function whose(q: Queued): string {
  const p = q.photo;
  if (p.variant) return `${p.code} · variant ${p.variant}`;
  if (p.family) return `${p.family} family · carton`;
  return p.level ? `${p.code} · ${p.level === "inner" ? "inner pack" : p.level}` : p.code;
}

/**
 * Move a look's photographs to the item and level they are of (D190): its
 * code, and carton, inner pack or each. Every photograph of that look moves.
 */
function MoveDialog({ desk, q, onClose }: { desk: QueueDesk; q: Queued; onClose: () => void }) {
  const [code, setCode] = useState(q.photo.family ? "" : q.photo.code);
  const [level, setLevel] = useState(q.photo.level ?? "carton");
  const count = desk.queued.filter((x) => x.photo.look_id === q.photo.look_id && x.state !== "saved").length;
  const go = async () => {
    if (await desk.move(q.photo.look_id, code, level)) onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      width={440}
      title={count === 1 ? "Move this photo" : `Move these ${count} photos`}
      description={`Now filed as ${whose(q)}. Every photo taken with it moves too.`}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={desk.crop.busy} disabled={!code.trim()} onClick={() => void go()}>
            Move
          </Button>
        </>
      }
    >
      <Stack gap={3}>
        {desk.crop.problem && (
          <Alert tone="danger" onDismiss={desk.crop.dismiss}>
            {desk.crop.problem}
          </Alert>
        )}
        <TextField label="Item code" autoComplete="off" autoFocus value={code} onChange={(e) => setCode(e.target.value)} />
        <Tabs
          aria-label="Of its"
          value={level}
          onValueChange={setLevel}
          items={[
            { value: "carton", label: "Carton" },
            { value: "inner", label: "Inner pack" },
            { value: "each", label: "Each" },
          ]}
        />
      </Stack>
    </Dialog>
  );
}

/** What the list says of a photograph: its state, and for a found face how it lies against the face as measured (D214). */
function said(q: Queued): { text: string; out: boolean } {
  const face = q.name.toLowerCase();
  if (q.state === "turned") return { text: SAID.turned, out: true };
  if (q.state !== "found" || !q.lie) return { text: SAID[q.state], out: false };
  return q.lie === "matches"
    ? { text: `Found: matches the ${face} as measured`, out: false }
    : { text: `Found, but not the ${face}’s measured shape either way round: check it`, out: true };
}

/** One photograph: where its corners were found, drawn on it, and what to do with it. */
function QueuedPhoto({ q, desk, move }: { q: Queued; desk: QueueDesk; move: () => void }) {
  const saving = q.state === "saving";
  const busy = saving || desk.crop.busy;
  const status = said(q);
  const quad = q.corners ? [0, 1, 2, 3].map((i) => [q.corners![i * 2]!, q.corners![i * 2 + 1]!] as const) : null;
  return (
    <li className={s.queued}>
      <div className={s.queuePhoto}>
        {q.state === "failed" ? (
          <div className={s.photo} role="img" aria-label="Not shown">
            <ImageOff aria-hidden />
          </div>
        ) : (
          <>
            <img src={imageUrl(q.photo.digest)} alt={`${q.photo.code}, ${q.name.toLowerCase()}`} loading="lazy" />
            {quad && (
              <svg className={s.cropMarks} viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true">
                <path className={s.cropShade} fillRule="evenodd" d={`M0 0H1V1H0Z M${quad.map(([x, y]) => `${x} ${y}`).join(" L")}Z`} />
                <polygon className={s.cropEdge} points={quad.map((p) => p.join(",")).join(" ")} />
                <line className={cx(s.cropTop, status.out && s.cropTopOut)} x1={quad[0]![0]} y1={quad[0]![1]} x2={quad[1]![0]} y2={quad[1]![1]} />
              </svg>
            )}
          </>
        )}
      </div>
      <div className={s.queueText}>
        <span className={s.code}>{whose(q)}</span>
        <span>{q.name}</span>
        {status.out ? <span className={s.cropLieOut}>{status.text}</span> : <Faint>{status.text}</Faint>}
      </div>
      <div className={s.queueActions}>
        {q.state === "failed" ? (
          <Link href={`/items/${q.photo.item_id}`}>Open the item</Link>
        ) : (
          <>
            {(q.state === "found" || saving) && (
              <Button size="sm" icon={<Check />} loading={saving} disabled={desk.crop.busy} onClick={() => void desk.save(q.photo.image_id)}>
                {status.out ? "Confirm anyway" : "Confirm"}
              </Button>
            )}
            <Button size="sm" icon={<Crop />} disabled={!q.subject || saving} onClick={() => desk.adjust(q.photo.image_id)}>
              Adjust
            </Button>
            <Button size="sm" disabled={busy} onClick={() => void desk.keep(q.photo.image_id)}>
              Use as taken
            </Button>
            <Button size="sm" disabled={busy} onClick={move}>
              Move…
            </Button>
          </>
        )}
      </div>
    </li>
  );
}

/** Under this, how well the shape fitted a photograph is worth a look before saving. */
const FITS = 0.9;

/**
 * The round things to wrap in their photographs (D240): each made in turn and
 * shown as it would look, its unwrapped side under it, to save or to leave.
 */
function WrapList({ desk, wraps }: { desk: WrapDesk; wraps: Wrapping[] }) {
  return (
    <Card padded={false}>
      <Toolbar>
        <Faint>
          {desk.phone
            ? "Open this at a computer to wrap these round things in their photos."
            : `${wraps.length} round ${wraps.length === 1 ? "thing" : "things"} to wrap in their photos`}
        </Faint>
      </Toolbar>
      <ul className={s.queue} aria-label="Round things to wrap">
        {wraps.map((w) => (
          <WrapRow key={w.key} w={w} desk={desk} />
        ))}
      </ul>
    </Card>
  );
}

function WrapRow({ w, desk }: { w: Wrapping; desk: WrapDesk }) {
  const e = w.entry;
  const level = e.packaging_level === "inner" ? "inner pack" : e.packaging_level;
  const unsure = w.made?.fits.filter((f) => f.fit < FITS) ?? [];
  const shown = w.state === "made" || w.state === "saving";
  return (
    <li className={s.queued}>
      <div className={s.queuePhoto}>
        {shown && w.made && w.preview ? (
          <Suspense fallback={<div className={s.box} />}>
            <RoundView size={w.made.size} wrap={w.preview} label={e.code} urlOf={(u) => u} />
          </Suspense>
        ) : (
          <div className={s.photo} role="img" aria-label="Not made yet">
            <Crop aria-hidden />
          </div>
        )}
      </div>
      <div className={s.queueText}>
        <span className={s.code}>{level ? `${e.code} · ${level}` : e.code}</span>
        <span>{e.description}</span>
        {w.state === "waiting" && <Faint>{desk.phone ? "Wrapped at a computer" : "Waiting"}</Faint>}
        {w.state === "making" && <Faint>{w.step}</Faint>}
        {w.state === "failed" && <span className={s.cropLieOut}>{w.step}</span>}
        {w.state === "saving" && <Faint>Saving…</Faint>}
        {w.state === "made" &&
          (unsure.length > 0 ? (
            <span className={s.cropLieOut}>
              The shape fits the {unsure.map((f) => f.face).join(" and ")} {unsure.length === 1 ? "photo" : "photos"} loosely: check the label lines up
            </span>
          ) : (
            <Faint>Fitted to every photo: check the label reads right round it</Faint>
          ))}
        {w.state === "made" && w.step && <span className={s.cropLieOut}>{w.step}</span>}
        {shown && w.preview && <img className={s.wrapSide} src={w.preview.side} alt={`${e.code}, its side unwrapped`} />}
        {/* A bucket with no lid is open: its top photo looks into it (D241). */}
        {(w.state === "made" || w.state === "failed") && (
          <Tabs
            aria-label="Its top"
            value={w.open ? "open" : "lid"}
            onValueChange={(v) => desk.setOpen(w.key, v === "open")}
            items={[
              { value: "lid", label: "Has a lid" },
              { value: "open", label: "Open at the top" },
            ]}
          />
        )}
      </div>
      <div className={s.queueActions}>
        {shown && (
          <Button size="sm" icon={<Check />} loading={w.state === "saving"} onClick={() => void desk.save(w.key)}>
            Save
          </Button>
        )}
        {(w.state === "made" || w.state === "failed") && (
          <Button size="sm" onClick={() => desk.again(w.key)}>
            Make again
          </Button>
        )}
        <Link href={`/items/${e.open_item}`}>Open the item</Link>
      </div>
    </li>
  );
}

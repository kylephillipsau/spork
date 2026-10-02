import { Crop, ImageOff } from "lucide-react";

import { useState } from "react";

import { Alert, Button, Card, Checkbox, Dialog, EmptyState, Link, Page, PageHeader, Skeleton, Stack, Tabs, TextField, Toolbar, Spacer } from "@ui/index";
import { imageUrl } from "@domain/api";
import { Faint } from "@app/common/cells";

import { FaceCrop } from "./FaceCrop";
import type { Face } from "./subjects";
import type { QueueDesk, Queued } from "./usePhotoQueue";
import s from "./items.module.css";

/**
 * The photographs nobody has cut to their faces yet, at a computer (D181).
 *
 * A phone takes them and has not the memory to find a face, so the computer
 * finds each one, the oldest first, and draws where it put the corners. What
 * is right stays ticked and is saved together; what is not opens in the crop
 * screen. Nothing is kept until the person saves.
 */
export function PhotosPage({ desk }: { desk: QueueDesk }) {
  const read = desk.read;
  const open = desk.queued.filter((q) => q.state !== "saved");
  const looked = open.filter((q) => q.state !== "waiting" && q.state !== "finding").length;
  const ticked = open.filter((q) => q.ticked && q.corners).length;
  const still = open.length - looked;
  const [moving, setMoving] = useState<Queued | null>(null);

  return (
    <Page>
      <PageHeader
        title="Photos to crop"
        description="Photos not yet cut to their faces. The computer finds each face; check them, then save."
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
      ) : open.length === 0 ? (
        <Card>
          <EmptyState icon={<Crop />} title="Nothing to crop" description="Every photo has been cut to its face." />
        </Card>
      ) : (
        <Stack gap={4}>
          {desk.phone && (
            <Alert tone="info">Open this at a computer to have the faces found. Here, each photo opens to have its corners dragged.</Alert>
          )}
          {desk.crop.problem && !desk.adjusting && (
            <Alert tone="danger" onDismiss={desk.crop.dismiss}>
              {desk.crop.problem}
            </Alert>
          )}
          <Card padded={false}>
            <Toolbar>
              <Faint>
                {desk.saving
                  ? `Saving ${desk.saving.done + 1} of ${desk.saving.of}…`
                  : still > 0 && !desk.phone
                    ? `Finding faces: ${looked} of ${open.length} done`
                    : `${open.length} ${open.length === 1 ? "photo" : "photos"} to check`}
              </Faint>
              <Spacer />
              <Button variant="primary" disabled={ticked === 0 || desk.crop.busy} loading={desk.saving !== null} onClick={() => void desk.save()}>
                {ticked === 1 ? "Save the ticked photo" : `Save the ${ticked} ticked`}
              </Button>
            </Toolbar>
            <ul className={s.queue} aria-label="Photos to crop">
              {open.map((q) => (
                <QueuedPhoto key={q.photo.image_id} q={q} desk={desk} move={() => setMoving(q)} />
              ))}
            </ul>
          </Card>
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
  missed: "No face found: adjust it",
  failed: "Could not be read here",
  saved: "Saved",
};

/** One photograph: where its corners were found, drawn on it, and what to do with it. */
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

function QueuedPhoto({ q, desk, move }: { q: Queued; desk: QueueDesk; move: () => void }) {
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
                <line className={s.cropTop} x1={quad[0]![0]} y1={quad[0]![1]} x2={quad[1]![0]} y2={quad[1]![1]} />
              </svg>
            )}
          </>
        )}
      </div>
      <div className={s.queueText}>
        <span className={s.code}>{whose(q)}</span>
        <span>{q.name}</span>
        <Faint>{SAID[q.state]}</Faint>
      </div>
      <div className={s.queueActions}>
        {q.state === "failed" ? (
          <Link href={`/items/${q.photo.item_id}`}>Open the item</Link>
        ) : (
          <>
            <Checkbox label="Looks right" checked={q.ticked} disabled={!q.corners || desk.saving !== null} onCheckedChange={(on) => desk.tick(q.photo.image_id, on)} />
            <Button size="sm" icon={<Crop />} disabled={!q.subject || desk.saving !== null} onClick={() => desk.adjust(q.photo.image_id)}>
              Adjust
            </Button>
            <Button size="sm" disabled={desk.saving !== null || desk.crop.busy} onClick={() => void desk.keep(q.photo.image_id)}>
              Use as taken
            </Button>
            <Button size="sm" disabled={desk.saving !== null || desk.crop.busy} onClick={move}>
              Move…
            </Button>
          </>
        )}
      </div>
    </li>
  );
}

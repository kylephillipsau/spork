import { Crop, ImageOff } from "lucide-react";

import { Alert, Button, Card, Checkbox, EmptyState, Link, Page, PageHeader, Skeleton, Stack, Toolbar, Spacer } from "@ui/index";
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
                <QueuedPhoto key={q.photo.image_id} q={q} desk={desk} />
              ))}
            </ul>
          </Card>
        </Stack>
      )}
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
function QueuedPhoto({ q, desk }: { q: Queued; desk: QueueDesk }) {
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
        <span className={s.code}>{q.photo.code}</span>
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
          </>
        )}
      </div>
    </li>
  );
}

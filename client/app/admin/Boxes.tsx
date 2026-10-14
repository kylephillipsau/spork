import { lazy, Suspense, useState } from "react";
import { Camera, ChevronDown, ChevronRight, Crop, Plus, Trash2 } from "lucide-react";

import { Badge, Button, Card, Checkbox, DataTable, EmptyState, TextField, type Column } from "@ui/index";
import type { PackageTypeRow, Side } from "@domain/types";
import { Faint } from "@app/common/cells";
import { heightCm, kg } from "@app/common/format";
import { Thumb } from "@app/common/Thumb";
import { faceAspect } from "@app/items/box";
import { FaceCrop } from "@app/items/FaceCrop";
import { FilePress } from "@app/items/ItemProperties";

import type { WorkspaceBench } from "./useWorkspace";
import s from "./settings.module.css";

// three.js is its own chunk, fetched the first time a box is shown.
const BoxView = lazy(() => import("@app/items/BoxView"));

/** The sides a box is photographed from: walked round from the front, then its top. */
const SIDES: readonly Side[] = ["front", "right", "back", "left", "top"];

const SIDE_NAMES: Record<Side, string> = { front: "Front", right: "Right", back: "Back", left: "Left", top: "Top", bottom: "Bottom" };

/** A box the bench packs goods into: a fixed size, and no pallet or skid, which carries cartons. */
const fits = (b: PackageTypeRow) => b.dimensions_fixed && !["PAL", "SKI", "SKD"].includes(b.carrier_package_code ?? "");

/** Length, width and height, the box's three, for the drawing; null until all three are known. */
const sizeOf = (b: PackageTypeRow): [number, number, number] | null =>
  b.length_mm && b.width_mm && b.height_mm ? [b.length_mm, b.width_mm, b.height_mm] : null;

/** Each side's picture to draw with: cut to its face when somebody has, else as taken. */
function facesOf(box: PackageTypeRow): Partial<Record<Side, string>> {
  return Object.fromEntries(box.photos.map((p) => [p.face, p.cut?.digest ?? p.digest]));
}

/**
 * Workspace › Boxes: the boxes the bench packs into (D196, D199, D224), each
 * full box with the heights it is cut down to and its photographs (D252).
 *
 * A full box is a row, its heights beside it. Opened, it lists its heights to
 * add to and take from, says which the bench may suggest, and takes its
 * photographs: a carton of it, or of a height cut from it, is drawn with them.
 */
export function Boxes({ boxes, bench, open: first = null }: { boxes: PackageTypeRow[]; bench: WorkspaceBench; open?: string | null }) {
  const own = boxes.filter((b) => b.tenant_owned);
  const full = own.filter((b) => !b.cut);
  const cutsOf = (id: string) => own.filter((b) => b.cut?.from_id === id).sort((a, b) => (b.height_mm ?? 0) - (a.height_mm ?? 0));
  const [open, setOpen] = useState<string | null>(first);
  const opened = full.find((b) => b.id === open) ?? null;

  const columns: Column<PackageTypeRow>[] = [
    {
      key: "name",
      header: "Box",
      grow: true,
      // The name opens the box to its heights and photographs.
      cell: (b) =>
        fits(b) && sizeOf(b) ? (
          <Button
            size="sm"
            variant="ghost"
            icon={open === b.id ? <ChevronDown /> : <ChevronRight />}
            aria-expanded={open === b.id}
            aria-label={`${b.name}: ${open === b.id ? "close" : "open"} its heights and photos`}
            onClick={() => setOpen(open === b.id ? null : b.id)}
          >
            {b.name}
          </Button>
        ) : (
          <strong>{b.name}</strong>
        ),
      sort: (b) => b.name,
    },
    {
      key: "size",
      header: "Inside (L × W × H)",
      label: "Inside",
      cell: (b) => (sizeOf(b) ? `${b.length_mm} × ${b.width_mm} × ${b.height_mm} mm` : <Faint>No fixed size</Faint>),
      sort: (b) => (b.length_mm ?? 0) * (b.width_mm ?? 0) * (b.height_mm ?? 0),
    },
    {
      key: "heights",
      header: "Heights, cm",
      // Room for a box and three cuts on one line, beside a growing name.
      width: "160px",
      cell: (b) =>
        fits(b) && b.height_mm ? (
          <span className={s.heights}>
            <strong>{heightCm(b.height_mm)}</strong>
            {cutsOf(b.id).map((c) => (
              <span key={c.id}>{heightCm(c.height_mm ?? 0)}</span>
            ))}
          </span>
        ) : (
          <Faint>—</Faint>
        ),
    },
    {
      key: "photos",
      header: "Photos",
      cell: (b) => {
        if (!fits(b)) return <Faint>—</Faint>;
        const taken = SIDES.filter((f) => b.photos.some((p) => p.face === f)).length;
        return taken === 0 ? <Faint>None</Faint> : <Badge tone={taken === SIDES.length ? "success" : "neutral"}>{`${taken} of ${SIDES.length}`}</Badge>;
      },
      width: "100px",
    },
    {
      key: "empty",
      header: "Empty",
      cell: (b) => (fits(b) ? b.tare_weight_g === null ? <Faint>Not weighed</Faint> : kg(b.tare_weight_g) : <Faint>—</Faint>),
      sort: (b) => b.tare_weight_g,
      align: "right",
    },
    {
      key: "weight",
      header: "Max weight",
      cell: (b) => (fits(b) ? b.max_payload_g === null ? <Faint>No limit</Faint> : kg(b.max_payload_g) : <Faint>—</Faint>),
      sort: (b) => b.max_payload_g,
      align: "right",
    },
    {
      key: "suggest",
      header: "Suggested",
      cell: (b) =>
        fits(b) ? (
          <Checkbox
            label={<span className={s.srOnly}>Suggest {b.name}</span>}
            checked={b.suggested}
            disabled={bench.busy}
            onCheckedChange={(on) => void bench.suggest(b.id, on)}
          />
        ) : (
          <Faint>{b.dimensions_fixed ? "Carries cartons" : "No fixed size"}</Faint>
        ),
      width: "110px",
    },
  ];

  return (
    <>
      <Card
        title="Boxes"
        count={full.length}
        description="The boxes you pack into, each with the heights it is cut down to: open one to add a height, weigh it or photograph it. The pack bench suggests ticked boxes, the lowest that takes everything and no heavier than its max weight; any box can still be chosen by hand."
        padded={false}
      >
        <DataTable
          aria-label="Boxes"
          columns={columns}
          rows={full}
          rowKey={(b) => b.id}
          selectedKey={open ?? undefined}
          empty={<EmptyState title="No boxes yet" description="Boxes arrive with the packaging presets import." />}
        />
      </Card>
      {opened && <BoxCard box={opened} cuts={cutsOf(opened.id)} bench={bench} close={() => setOpen(null)} />}
      {bench.cropping && (
        <FaceCrop
          key={bench.cropping.image_id}
          cropping={bench.cropping}
          name={SIDE_NAMES[bench.cropping.face as Side] ?? "Photo"}
          aspect={opened && sizeOf(opened) ? faceAspect(bench.cropping.face as Side, sizeOf(opened)!) : null}
          desk={bench}
        />
      )}
    </>
  );
}

/**
 * One full box opened: its heights, and its photographs drawn as the box at
 * the height chosen, a cut showing the bottom of each side at true scale.
 */
function BoxCard({ box, cuts, bench, close }: { box: PackageTypeRow; cuts: PackageTypeRow[]; bench: WorkspaceBench; close: () => void }) {
  const [shown, show] = useState(box.id);
  const [height, setHeight] = useState("");
  const [name, setName] = useState("");
  const full = sizeOf(box)!;
  const rows = [box, ...cuts];
  const previewed = rows.find((r) => r.id === shown) ?? box;
  const tall = previewed.height_mm ?? full[2];
  const keep = tall / full[2];

  const columns: Column<PackageTypeRow>[] = [
    {
      key: "height",
      header: "Height",
      cell: (h) =>
        h.cut ? (
          `${heightCm(h.height_mm ?? 0)} cm`
        ) : (
          <span>
            <strong>{heightCm(full[2])} cm</strong> <Faint>full</Faint>
          </span>
        ),
      width: "120px",
    },
    { key: "name", header: "Booked as", cell: (h) => h.name },
    {
      key: "empty",
      header: "Empty",
      // A height weighs as its box unless it says otherwise (D252).
      cell: (h) => (
        <Kilograms
          label={`What ${h.name} weighs empty`}
          grams={h.tare_weight_g}
          placeholder={h.cut ? (box.tare_weight_g === null ? "As the box" : `${box.tare_weight_g / 1000}, as the box`) : "Not weighed"}
          disabled={bench.busy}
          save={(g) => void bench.boxEmptyWeight(h.id, g)}
        />
      ),
      width: "150px",
    },
    {
      key: "weight",
      header: "Max weight",
      cell: (h) => (
        <Kilograms
          label={`Max weight of ${h.name}`}
          grams={h.max_payload_g}
          placeholder={h.cut ? (box.max_payload_g === null ? "As the box" : `${box.max_payload_g / 1000}, as the box`) : "No limit"}
          disabled={bench.busy}
          save={(g) => void bench.boxWeight(h.id, g)}
        />
      ),
      width: "150px",
    },
    { key: "packed", header: "Packed", cell: (h) => h.packed, align: "right", width: "80px" },
    {
      key: "suggest",
      header: "Suggested",
      cell: (h) => (
        <Checkbox
          label={<span className={s.srOnly}>Suggest {h.name}</span>}
          checked={h.suggested}
          disabled={bench.busy}
          onCheckedChange={(on) => void bench.suggest(h.id, on)}
        />
      ),
      width: "110px",
    },
    {
      key: "remove",
      header: <span className={s.srOnly}>Take away</span>,
      cell: (h) =>
        h.cut ? (
          <Button
            size="sm"
            variant="ghost"
            icon={<Trash2 />}
            aria-label={h.packed > 0 ? `${h.name} has been packed in, so it stays` : `Take away ${h.name}`}
            disabled={bench.busy || h.packed > 0}
            onClick={() => void bench.removeHeight(h.id)}
          />
        ) : null,
      align: "right",
      width: "64px",
    },
  ];

  return (
    <Card
      title={box.name}
      description={`${box.length_mm} × ${box.width_mm} × ${box.height_mm} mm inside, cut at the corners and folded down to these heights. A height weighs and holds as its box unless it says otherwise. One something was packed in stays on file: untick it to stop the bench suggesting it.`}
      actions={<Button onClick={close}>Close</Button>}
    >
      <div className={s.stack}>
        <DataTable aria-label={`Heights of ${box.name}`} columns={columns} rows={rows} rowKey={(h) => h.id} onRowClick={(h) => show(h.id)} selectedKey={shown} />
        <form
          className={s.formRow}
          onSubmit={(e) => {
            e.preventDefault();
            void bench.addHeight(box.id, height, name).then((added) => {
              if (!added) return;
              setHeight("");
              setName("");
            });
          }}
        >
          <div className={s.narrow}>
            <TextField label="Height, cm" inputMode="decimal" autoComplete="off" value={height} onChange={(e) => setHeight(e.target.value)} />
          </div>
          <div className={s.grow}>
            <TextField
              label="Booked as, if not its own"
              autoComplete="off"
              placeholder={`${box.name}, cut to ${height.trim() || "…"}`}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <Button type="submit" icon={<Plus />} disabled={bench.busy || !height.trim()}>
            Add height
          </Button>
        </form>

        <div className={s.boxGrid}>
          <div className={s.boxLook}>
            <Suspense fallback={<div className={s.boxStage} />}>
              <BoxView faces={facesOf(box)} size={[full[0], full[1], tall]} facing={null} label={previewed.name} keep={keep} />
            </Suspense>
            <p className={s.lookCaption}>
              {previewed.cut
                ? `${previewed.name}: the bottom ${Math.round(keep * 100)}% of each side, at true scale.`
                : `Full, ${heightCm(tall)} cm, as photographed. Choose a height to see it cut.`}
            </p>
          </div>
          <ul className={s.sides} aria-label={`Photographs of ${box.name}`}>
            {SIDES.map((face) => {
              const photo = box.photos.find((p) => p.face === face);
              return (
                <li key={face} className={s.side}>
                  <Thumb picture={photo ? { digest: photo.cut?.digest ?? photo.digest, source: "own" } : null} alt={`${box.name}, ${face}`} />
                  <span className={s.muted}>{SIDE_NAMES[face]}</span>
                  {photo && !photo.cut && <Faint>Not cropped</Faint>}
                  <FilePress
                    label={`Photograph the ${face} of ${box.name}`}
                    text={photo ? "Again" : "Take"}
                    icon={<Camera aria-hidden />}
                    camera
                    disabled={bench.busy}
                    chosen={([file]) => void bench.photographBox(box, face, file!)}
                  />
                  {photo && (
                    <Button
                      size="sm"
                      icon={<Crop />}
                      aria-label={`Crop the ${face} of ${box.name}`}
                      disabled={bench.busy}
                      onClick={() =>
                        bench.crop({
                          subject: { box_shaped: true, length_mm: box.length_mm, width_mm: box.width_mm, height_mm: box.height_mm },
                          face,
                          image_id: photo.image_id,
                          digest: photo.digest,
                          corners: photo.cut?.corners ?? null,
                        })
                      }
                    >
                      Crop
                    </Button>
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      </div>
    </Card>
  );
}

/** A weight in kilograms as typed, saved when it changes and makes sense. */
function Kilograms({
  label,
  grams,
  placeholder,
  disabled,
  save,
}: {
  label: string;
  grams: number | null;
  placeholder: string;
  disabled: boolean;
  save: (grams: number | null) => void;
}) {
  const said = grams === null ? "" : String(grams / 1000);
  const [typed, setTyped] = useState(said);
  const commit = () => {
    const t = typed.trim();
    if (t === said) return;
    const kg = Number(t);
    if (t !== "" && !(kg > 0)) {
      setTyped(said);
      return;
    }
    save(t === "" ? null : Math.round(kg * 1000));
  };
  return (
    <TextField
      aria-label={label}
      inputMode="decimal"
      placeholder={placeholder}
      trailing="kg"
      value={typed}
      disabled={disabled}
      onChange={(e) => setTyped(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
      }}
    />
  );
}

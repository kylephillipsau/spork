import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { anAct } from "@domain/acts";
import { api, reason } from "@domain/api";
import type { PackageTypeRow, ReportedField, Side, Workspace } from "@domain/types";
import { handheld } from "@app/items/crop";
import type { Pixels, Point } from "@app/items/cut";
import type { CropOpen } from "@app/items/FaceCrop";

const model = () => import("@app/items/faceModel");

/**
 * The organisation and its warehouses, and the two things a site says about
 * packing: where it packs, and whose stock it holds (migrations 95 and 97).
 * Both are settings rather than acts, so a change is followed by a re-read
 * and nothing is kept locally.
 */

export type WorkspaceState =
  | { kind: "loading" }
  | { kind: "ready"; workspace: Workspace }
  | { kind: "failed"; message: string };

export interface WorkspaceBench {
  state: WorkspaceState;
  busy: boolean;
  /** Why the last change was refused, until dismissed. */
  problem: string | null;
  dismiss: () => void;
  setPackLocation: (siteId: string, code: string) => Promise<void>;
  setOwner: (siteId: string) => Promise<void>;
  /** How far a carton's weight may be off and still agree, in parts per thousand (D244). */
  setWeightTolerance: (siteId: string, perMille: number) => Promise<void>;
  /** The box presets, the workspace's own first; null until read. */
  boxes: PackageTypeRow[] | null;
  /** Say whether the pack bench's suggestion may choose a box (D196). */
  suggest: (boxId: string, suggested: boolean) => Promise<void>;
  /** Say the most a box's goods may weigh, in grams, or null for no limit (D199). */
  boxWeight: (boxId: string, grams: number | null) => Promise<void>;
  /** Say what a box weighs empty, in grams, or null until one is weighed (D224). */
  boxEmptyWeight: (boxId: string, grams: number | null) => Promise<void>;
  /**
   * Add a height a box is cut down to, in centimetres as typed, booked as
   * `name` when one is given (D252). True when it was added.
   */
  addHeight: (boxId: string, cm: string, name: string) => Promise<boolean>;
  /** Take a height away: only one nothing was packed in (D252). */
  removeHeight: (boxId: string) => Promise<void>;
  /** Photograph a side of a box (D252); at a desk it then opens to be cropped. */
  photographBox: (box: PackageTypeRow, face: Side, image: Blob) => Promise<void>;
  /** The photograph open to be cut to its face, if one is. */
  cropping: CropOpen | null;
  crop: (cropping: CropOpen) => void;
  uncrop: () => void;
  /** Cut the open photograph at these corners, `make` straightening it (D176). */
  cut: (corners: number[], make: () => Promise<Blob>) => Promise<void>;
  /** Where the face is in a photograph (D177), as the item page finds it. */
  findFace: (key: string, pixels: Pixels, at?: Point) => Promise<number[] | null>;
  /** NetSuite's fields, and what each is read as (D238); null until read. */
  fields: ReportedField[] | null;
  /** Say what one of them means. */
  sayField: (field: ReportedField, meaning: { role: string; unit: string | null; unit_field: string | null; level: string | null }) => Promise<void>;
}

export function useWorkspace(): WorkspaceBench {
  const [state, setState] = useState<WorkspaceState>({ kind: "loading" });
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  const live = useLive();

  const [boxes, setBoxes] = useState<PackageTypeRow[] | null>(null);
  const [cropping, setCropping] = useState<CropOpen | null>(null);
  const [fields, setFields] = useState<ReportedField[] | null>(null);

  const read = useCallback(async () => {
    try {
      const [workspace, presets, said] = await Promise.all([api.workspace(), api.packageTypes(), api.netsuiteFields()]);
      if (live.current) {
        setState({ kind: "ready", workspace });
        setBoxes(presets);
        setFields(said);
      }
    } catch (error) {
      const message = reason(error, "The server could not be reached.");
      if (live.current) setState({ kind: "failed", message });
    }
  }, [live]);

  useEffect(() => {
    void read();
  }, [read]);

  const change = useCallback(
    async (act: () => Promise<unknown>, failure: string) => {
      setBusy(true);
      setProblem(null);
      try {
        await act();
        await read();
      } catch (error) {
        if (live.current) setProblem(reason(error, failure));
      } finally {
        if (live.current) setBusy(false);
      }
    },
    [read, live],
  );

  const setPackLocation = useCallback(
    (siteId: string, code: string) => change(() => api.setPackLocation(siteId, code), "Could not set where this site packs."),
    [change],
  );
  const setOwner = useCallback(
    (siteId: string) => change(() => api.setSiteOwner(siteId), "Could not set who owns this site's stock."),
    [change],
  );

  const setWeightTolerance = useCallback(
    (siteId: string, perMille: number) =>
      change(() => api.setWeightTolerance(siteId, perMille), "Could not change how far a weight may be off."),
    [change],
  );

  const suggest = useCallback(
    (boxId: string, suggested: boolean) => change(() => api.suggestBox(boxId, suggested), "Could not change whether that box is suggested."),
    [change],
  );

  const boxWeight = useCallback(
    (boxId: string, grams: number | null) => change(() => api.boxWeight(boxId, grams), "Could not change that box's weight limit."),
    [change],
  );

  const boxEmptyWeight = useCallback(
    (boxId: string, grams: number | null) => change(() => api.boxEmptyWeight(boxId, grams), "Could not change what that box weighs empty."),
    [change],
  );

  const addHeight = useCallback(
    async (boxId: string, cm: string, name: string) => {
      const mm = Math.round(Number(cm.trim().replace(",", ".")) * 10);
      if (!cm.trim() || !Number.isFinite(mm) || mm <= 0) {
        setProblem("Say the height in centimetres: 28, or 34.5.");
        return false;
      }
      let added = false;
      await change(async () => {
        await api.addHeight(boxId, mm, name.trim() || null);
        added = true;
      }, "Could not add that height.");
      return added;
    },
    [change],
  );

  const removeHeight = useCallback(
    (boxId: string) => change(() => api.removeHeight(boxId), "Could not take that height away."),
    [change],
  );

  // One look a photograph, as a retake is a new row (D132); at a desk, straight
  // on to marking its corners, as the item page does (D181).
  const photographBox = useCallback(
    async (box: PackageTypeRow, face: Side, image: Blob) => {
      const sent: { photo: { image_id: string; digest: string } | null } = { photo: null };
      await change(async () => {
        const look = await api.recordCapture({ box: box.id, level: null, measurements: [], photographs: true, act: anAct() });
        sent.photo = await api.photograph(look.observation_event_id, face, image);
      }, `Could not keep the photograph of the ${face}.`);
      const { photo } = sent;
      if (photo && live.current && !handheld()) {
        const subject = { box_shaped: true, length_mm: box.length_mm, width_mm: box.width_mm, height_mm: box.height_mm };
        setCropping({ subject, face, image_id: photo.image_id, digest: photo.digest, corners: null });
      }
    },
    [change, live],
  );

  const cut = useCallback(
    (corners: number[], make: () => Promise<Blob>) =>
      change(async () => {
        if (!cropping) return;
        const straight = await api.storeImage(await make());
        await api.recordCut(cropping.image_id, { digest: straight.digest, corners, act: anAct() });
        if (live.current) setCropping(null);
      }, "Could not keep the crop."),
    [change, cropping, live],
  );

  const sayField = useCallback(
    (field: ReportedField, meaning: { role: string; unit: string | null; unit_field: string | null; level: string | null }) =>
      change(
        () => api.sayNetSuiteField({ source: field.source, field: field.field, ...meaning }, anAct()),
        `Could not change what ${field.field} means.`,
      ),
    [change],
  );

  return {
    state,
    busy,
    problem,
    dismiss: () => setProblem(null),
    setPackLocation,
    setOwner,
    setWeightTolerance,
    boxes,
    suggest,
    boxWeight,
    boxEmptyWeight,
    addHeight,
    removeHeight,
    photographBox,
    cropping,
    crop: setCropping,
    uncrop: () => setCropping(null),
    cut,
    findFace: (key, pixels, at) => model().then((m) => m.findFace(key, pixels, at)),
    fields,
    sayField,
  };
}

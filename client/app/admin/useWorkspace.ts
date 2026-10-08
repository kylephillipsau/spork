import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { anAct } from "@domain/acts";
import { api, reason } from "@domain/api";
import type { PackageTypeRow, ReportedField, Workspace } from "@domain/types";

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
  /** The box presets, the workspace's own first; null until read. */
  boxes: PackageTypeRow[] | null;
  /** Say whether the pack bench's suggestion may choose a box (D196). */
  suggest: (boxId: string, suggested: boolean) => Promise<void>;
  /** Say the most a box's goods may weigh, in grams, or null for no limit (D199). */
  boxWeight: (boxId: string, grams: number | null) => Promise<void>;
  /** Say what a box weighs empty, in grams, or null until one is weighed (D224). */
  boxEmptyWeight: (boxId: string, grams: number | null) => Promise<void>;
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
    boxes,
    suggest,
    boxWeight,
    boxEmptyWeight,
    fields,
    sayField,
  };
}

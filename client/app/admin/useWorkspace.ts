import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { Workspace } from "@domain/types";

/** The organisation and its warehouses. A read: no acts on this screen. */

export type WorkspaceState =
  | { kind: "loading" }
  | { kind: "ready"; workspace: Workspace }
  | { kind: "failed"; message: string };

export interface WorkspaceBench {
  state: WorkspaceState;
}

export function useWorkspace(): WorkspaceBench {
  const [state, setState] = useState<WorkspaceState>({ kind: "loading" });

  const live = useLive();

  const read = useCallback(async () => {
    try {
      const workspace = await api.workspace();
      if (live.current) setState({ kind: "ready", workspace });
    } catch (error) {
      const message = reason(error, "The server could not be reached.");
      if (live.current) setState({ kind: "failed", message });
    }
  }, []);

  useEffect(() => {
    void read();
  }, [read]);

  return { state };
}

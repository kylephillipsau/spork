import { useCallback, useEffect, useRef, useState } from "react";

import { useLive } from "@app/acting";
import { reason, api } from "@domain/api";
import type { BackupSummary } from "@domain/types";

/**
 * A workspace's backup (D193): what it would hold, and downloading it.
 *
 * **Downloaded by a form post, not a fetch.** A fetch holds the whole file in
 * memory before it can be saved, and a backup carries every photograph. A
 * form posted into a hidden frame lets the browser save the file as it
 * arrives. A refusal is a page in that frame instead of a download, so its
 * words are read from there.
 */

/** The shortest password a backup is encrypted with; the server says the same. */
export const LEAST_PASSWORD = 12;

export type BackupState =
  | { kind: "loading" }
  | { kind: "ready"; summary: BackupSummary }
  | { kind: "failed"; message: string };

export interface BackupBench {
  state: BackupState;
  /** Asked for, and the browser is waiting for the file. */
  asked: boolean;
  problem: string | null;
  dismiss: () => void;
  download: (password: string) => void;
}

export function useBackup(): BackupBench {
  const live = useLive();
  const [state, setState] = useState<BackupState>({ kind: "loading" });
  const [asked, setAsked] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const frame = useRef<HTMLIFrameElement | null>(null);

  useEffect(() => {
    api.backupSummary().then(
      (summary) => live.current && setState({ kind: "ready", summary }),
      (error) => live.current && setState({ kind: "failed", message: reason(error, "The server could not be reached.") }),
    );
  }, [live]);

  // The frame a download is posted into, made once and removed with the screen.
  useEffect(() => {
    const f = document.createElement("iframe");
    f.name = "spork-backup";
    f.hidden = true;
    f.addEventListener("load", () => {
      // Only a refusal loads here: a download is saved, not shown.
      const text = f.contentDocument?.body?.innerText ?? "";
      if (!text || !live.current) return;
      let said = "The backup could not be made.";
      try {
        const body = JSON.parse(text) as { detail?: string };
        if (body.detail) said = body.detail.charAt(0).toUpperCase() + body.detail.slice(1) + ".";
      } catch {
        // Not the server's words: keep the plain message.
      }
      setAsked(false);
      setProblem(said);
    });
    document.body.appendChild(f);
    frame.current = f;
    return () => f.remove();
  }, [live]);

  const download = useCallback((password: string) => {
    if (!frame.current) return;
    setProblem(null);
    setAsked(true);
    const form = document.createElement("form");
    form.method = "post";
    form.action = "/api/backup";
    form.target = frame.current.name;
    form.hidden = true;
    const field = document.createElement("input");
    field.type = "hidden";
    field.name = "password";
    field.value = password;
    form.appendChild(field);
    document.body.appendChild(form);
    form.submit();
    form.remove();
  }, []);

  return { state, asked, problem, dismiss: () => setProblem(null), download };
}

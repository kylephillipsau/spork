import type { BackupBench, BackupState } from "./useBackup";

/** A workspace's backup, with no network: ready, and asked for. */
export const BACKUP_READY: BackupState = {
  kind: "ready",
  summary: { items: 7214, photos: 486, photo_bytes: 1_610_000_000, schema: "2026-08-04-000112_whoever_set_a_workspace_up_administers_it" },
};

export function fixtureBackup(state: BackupState, over: Partial<BackupBench> = {}): BackupBench {
  return { state, asked: false, problem: null, dismiss: () => {}, download: () => {}, ...over };
}

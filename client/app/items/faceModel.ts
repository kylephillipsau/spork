import type { Pixels, Point } from "./cut";
import { UNFIT } from "./faceFind";
import type { Answer, Ask } from "./faceWorker";

/**
 * The face-finder, as the page asks it (D177). The model runs in a worker of
 * its own, [`faceWorker`], started the first time it is wanted; this only
 * passes it questions and hands back its answers.
 *
 * **A phone may not have the memory for it.** Finding a face takes about
 * 1.5 GB above the page, and a phone's browser reloads a page that grows past
 * what it allows, so the page is gone and nothing says why. So a find is
 * marked as under way while it runs; a page that loads to find the mark still
 * there was taken down by it, and the face-finder is not asked again on that
 * device until somebody says to try it anyway. The marks live in the
 * browser's own storage, which survives the reload.
 */

const TRYING = "spork.faceFinder.trying";
const OFF = "spork.faceFinder.off";

function read(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}
function write(key: string, value: string | null): void {
  try {
    if (value === null) window.localStorage.removeItem(key);
    else window.localStorage.setItem(key, value);
  } catch {
    // Storage refused (a private window): the guard is a convenience.
  }
}

// Loaded fresh after a reload: a find still marked as under way did not finish.
if (read(TRYING) !== null) {
  write(OFF, new Date().toISOString());
  write(TRYING, null);
}

let worker: Worker | undefined;
let asked = 0;
const waiting = new Map<number, { resolve: (corners: number[] | null) => void; reject: (error: Error) => void }>();

function hire(): Worker {
  if (worker) return worker;
  const hired = new Worker(new URL("./faceWorker.ts", import.meta.url), { type: "module", name: "face-finder" });
  hired.addEventListener("message", (e: MessageEvent<Answer>) => {
    const answer = e.data;
    const job = waiting.get(answer.id);
    waiting.delete(answer.id);
    if ("error" in answer) job?.reject(new Error(answer.error));
    else job?.resolve(answer.corners);
  });
  // A worker that cannot start answers nothing: everything waiting is told,
  // and the next question starts another.
  hired.addEventListener("error", (e) => {
    for (const job of waiting.values()) job.reject(new Error(`the face-finder could not start: ${e.message}`));
    waiting.clear();
    hired.terminate();
    if (worker === hired) worker = undefined;
  });
  worker = hired;
  return hired;
}

type Question = Ask extends infer A ? (A extends Ask ? Omit<A, "id"> : never) : never;

function ask(question: Question, transfer: Transferable[] = []): Promise<number[] | null> {
  const id = ++asked;
  return new Promise((resolve, reject) => {
    waiting.set(id, { resolve, reject });
    hire().postMessage({ ...question, id }, transfer);
  });
}

/** Fetch and compile the model, once, so it is ready by the first crop. */
export const warm = () => ask({ kind: "warm" });

/**
 * The face at a point of a photograph, as eight corner fractions from its
 * top-left, or nothing. `pixels` is the photograph scaled to the model's size
 * on its longest side, handed over rather than copied; `at` is a fraction of
 * it, the middle when not given. Refused with [`UNFIT`] on a device it took
 * down before, unless `anyway`.
 */
export async function findFace(key: string, pixels: Pixels, at: Point = [0.5, 0.5], anyway = false): Promise<number[] | null> {
  if (anyway) write(OFF, null);
  if (read(OFF) !== null) throw new Error(UNFIT);
  write(TRYING, new Date().toISOString());
  try {
    return await ask({ kind: "find", key, pixels, at }, [pixels.data.buffer]);
  } finally {
    write(TRYING, null);
  }
}

/**
 * Let the model go, and the memory it holds: a worker's memory is returned
 * only when it ends. The next question starts another.
 */
export function release(): void {
  if (!worker) return;
  for (const job of waiting.values()) job.reject(new Error("the face-finder was let go"));
  waiting.clear();
  worker.terminate();
  worker = undefined;
}

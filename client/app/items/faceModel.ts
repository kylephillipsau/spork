import type { Pixels, Point } from "./cut";
import type { Answer, Ask } from "./faceWorker";

/**
 * The face-finder, as the page asks it (D177). The model runs in a worker of
 * its own, [`faceWorker`], started the first time it is wanted; this only
 * passes it questions and hands back its answers.
 *
 * Never on a phone, which has not the memory (D181).
 */

let worker: Worker | undefined;
let asked = 0;
const waiting = new Map<number, { resolve: (answer: Answer) => void; reject: (error: Error) => void }>();

function hire(): Worker {
  if (worker) return worker;
  const hired = new Worker(new URL("./faceWorker.ts", import.meta.url), { type: "module", name: "face-finder" });
  hired.addEventListener("message", (e: MessageEvent<Answer>) => {
    const answer = e.data;
    const job = waiting.get(answer.id);
    waiting.delete(answer.id);
    if ("error" in answer) job?.reject(new Error(answer.error));
    else job?.resolve(answer);
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

function ask(question: Question, transfer: Transferable[] = []): Promise<Answer> {
  const id = ++asked;
  return new Promise((resolve, reject) => {
    waiting.set(id, { resolve, reject });
    hire().postMessage({ ...question, id }, transfer);
  });
}

/** Fetch and compile the model, once, so it is ready by the first crop. */
export const warm = () => ask({ kind: "warm" }).then(() => undefined);

/**
 * The face at a point of a photograph, as eight corner fractions from its
 * top-left, or nothing. `pixels` is the photograph scaled to the model's size
 * on its longest side, handed over rather than copied; `at` is a fraction of
 * it, the middle when not given.
 */
export const findFace = (key: string, pixels: Pixels, at: Point = [0.5, 0.5]) =>
  ask({ kind: "find", key, pixels, at }, [pixels.data.buffer]).then((a) => ("corners" in a ? a.corners : null));

/**
 * The outlines a whole thing could be (D240), asked at each of some points
 * of a photograph, or of the one thing at all of a few: yes or no per pixel
 * of `pixels`, which is the photograph at the model's size and is handed over.
 */
export const findWholes = (key: string, pixels: Pixels, at: Point[][]) =>
  ask({ kind: "wholes", key, pixels, at }, [pixels.data.buffer]).then((a) => ("outlines" in a ? a.outlines : []));

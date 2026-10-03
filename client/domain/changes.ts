/**
 * HEARING THAT SOMETHING CHANGED (D206).
 *
 * The server keeps a stream open, `GET /changes`, and says `changed` whenever an
 * act is recorded or an order arrives or moves on at this device's site. A
 * screen that hears it reads again what it is showing. The stream says where,
 * never what, so there is nothing to apply and nothing to replay: a device that
 * was away reads once when it is back.
 *
 * **Read with `fetch`, not `EventSource`.** On a handheld the session is a
 * bearer token and every request goes through the platform's own client
 * (`Transport` in api.ts), and `EventSource` can do neither. So the stream is
 * read here, a piece at a time.
 *
 * **When the stream doesn't come, the device asks on a timer.** A stream
 * opens with `hello`; one that doesn't say it within a few seconds, because
 * something between is holding the response back, is treated as no stream.
 * Until one works again, listeners are told every so often anyway, so a
 * screen is never more than that far behind.
 *
 * Pure apart from the timers and the `open` it is given, so it runs under
 * `node --test`.
 */

export interface Frame {
  event: string;
  data: string;
}

/** Reads `text/event-stream` as it arrives, in pieces of any size. */
export class FrameReader {
  private buffer = "";

  push(text: string): Frame[] {
    this.buffer += text.replace(/\r/g, "");
    const frames: Frame[] = [];
    for (let at = this.buffer.indexOf("\n\n"); at >= 0; at = this.buffer.indexOf("\n\n")) {
      const block = this.buffer.slice(0, at);
      this.buffer = this.buffer.slice(at + 2);
      let event = "message";
      const data: string[] = [];
      let said = false;
      for (const line of block.split("\n")) {
        // A blank line, or a comment such as the server's keepalive.
        if (line === "" || line.startsWith(":")) continue;
        const colon = line.indexOf(":");
        const field = colon < 0 ? line : line.slice(0, colon);
        const raw = colon < 0 ? "" : line.slice(colon + 1);
        const value = raw.startsWith(" ") ? raw.slice(1) : raw;
        if (field === "event") {
          event = value;
          said = true;
        } else if (field === "data") {
          data.push(value);
          said = true;
        }
      }
      if (said) frames.push({ event, data: data.join("\n") });
    }
    return frames;
  }
}

/** How long to wait before trying the stream again: one second, doubling, at most thirty. */
export function backoff(attempt: number): number {
  return Math.min(30_000, 1000 * 2 ** Math.max(0, attempt));
}

export interface Timings {
  /** A stream that hasn't said hello by then is no stream. */
  helloWithin: number;
  /** A burst of changes is one read. */
  settle: number;
  /** How often listeners are told anyway while there is no stream. */
  pollEvery: number;
  /** How long to wait before the next try, by how many have failed. */
  retry: (attempt: number) => number;
}

export const TIMINGS: Timings = { helloWithin: 8_000, settle: 300, pollEvery: 20_000, retry: backoff };

export type Standing = "off" | "connecting" | "live" | "polling";

/**
 * One stream for the whole device, opened when the first listener comes and
 * closed when the last one goes.
 */
export class Channel {
  private readonly listeners = new Set<() => void>();
  private abort: AbortController | null = null;
  private failures = 0;
  /** Whether a stream has said hello since the first listener came. */
  private heardOnce = false;
  private settling: ReturnType<typeof setTimeout> | null = null;
  private polling: ReturnType<typeof setInterval> | null = null;
  private retrying: ReturnType<typeof setTimeout> | null = null;
  standing: Standing = "off";

  private readonly open: (signal: AbortSignal) => Promise<Response>;
  private readonly timings: Timings;

  constructor(open: (signal: AbortSignal) => Promise<Response>, timings: Timings = TIMINGS) {
    this.open = open;
    this.timings = timings;
  }

  /** Be told when something changed. Returns the way to stop. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    if (this.listeners.size === 1) void this.connect();
    return () => {
      this.listeners.delete(listener);
      if (this.listeners.size === 0) this.stop();
    };
  }

  /**
   * The device is back: the screen woke, or the network returned. Tell the
   * listeners now, and if there is no stream, try for one at once.
   */
  nudge(): void {
    if (this.listeners.size === 0) return;
    this.tell();
    if (this.standing !== "live" && this.standing !== "connecting") {
      this.failures = 0;
      if (this.retrying) clearTimeout(this.retrying);
      this.retrying = null;
      void this.connect();
    }
  }

  private stop(): void {
    this.abort?.abort();
    this.abort = null;
    for (const t of [this.settling, this.retrying]) if (t) clearTimeout(t);
    if (this.polling) clearInterval(this.polling);
    this.settling = this.retrying = this.polling = null;
    this.failures = 0;
    this.heardOnce = false;
    this.standing = "off";
  }

  private async connect(): Promise<void> {
    const abort = new AbortController();
    this.abort?.abort();
    this.abort = abort;
    this.retrying = null;
    if (this.standing !== "polling") this.standing = "connecting";
    let hello = false;
    const quiet = setTimeout(() => abort.abort(), this.timings.helloWithin);
    try {
      const response = await this.open(abort.signal);
      if (!response.ok || !response.body) throw new Error(`no stream (${response.status})`);
      const reader = response.body.getReader();
      // Stopping ends the read too, whether or not `open` honoured the signal.
      abort.signal.addEventListener("abort", () => void reader.cancel().catch(() => undefined));
      if (abort.signal.aborted) void reader.cancel().catch(() => undefined);
      const decoder = new TextDecoder();
      const frames = new FrameReader();
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        for (const frame of frames.push(decoder.decode(value, { stream: true }))) {
          if (frame.event === "hello") {
            hello = true;
            clearTimeout(quiet);
            this.failures = 0;
            this.standing = "live";
            if (this.polling) clearInterval(this.polling);
            this.polling = null;
            // The first hello is the screen's own first read. Any after it
            // follow a gap in which something may have changed unheard.
            if (this.heardOnce) this.settle();
            this.heardOnce = true;
          } else if (frame.event === "changed") {
            this.settle();
          }
        }
      }
    } catch {
      /* a stream that failed is handled below, the same as one that ended */
    } finally {
      clearTimeout(quiet);
    }
    if (this.abort !== abort) return; // stopped, or another took its place
    this.abort = null;
    if (hello) {
      // It worked and ended, as the server ends every stream after a few
      // minutes, or the connection dropped. Straight back.
      this.standing = "connecting";
      this.retrying = setTimeout(() => void this.connect(), 0);
    } else {
      this.failures += 1;
      this.poll();
      this.retrying = setTimeout(() => void this.connect(), this.timings.retry(this.failures));
    }
  }

  private poll(): void {
    this.standing = "polling";
    if (this.polling) return;
    this.polling = setInterval(() => this.tell(), this.timings.pollEvery);
  }

  private settle(): void {
    if (this.settling) return;
    this.settling = setTimeout(() => {
      this.settling = null;
      this.tell();
    }, this.timings.settle);
  }

  private tell(): void {
    for (const listener of [...this.listeners]) {
      try {
        listener();
      } catch {
        /* one screen's failure to read is not another's */
      }
    }
  }
}

import { Alert, Button } from "@ui/index";

import s from "./kept.module.css";

/**
 * Acts kept on this device until they can be sent (D207): what is waiting,
 * with a way to send it now; any the server refused when they went, until
 * read; and anybody else's left on a shared device. Picking and the pack
 * bench each say it in their own words.
 */
export function Kept({
  waiting,
  send,
  refused,
  dismiss,
  others,
}: {
  /** What is waiting, in a sentence; null when nothing is. */
  waiting: string | null;
  send: () => void;
  refused: { key: string; text: string }[];
  dismiss: (key: string) => void;
  /** Somebody else's, a sentence each. */
  others: string[];
}) {
  return (
    <>
      {waiting && (
        <Alert tone="warning">
          <span className={s.line}>
            <span>{waiting}</span>
            <Button onClick={send}>Send now</Button>
          </span>
        </Alert>
      )}
      {refused.map((r) => (
        <Alert key={r.key} tone="danger" onDismiss={() => dismiss(r.key)}>
          {r.text}
        </Alert>
      ))}
      {others.map((o) => (
        <p key={o} className={s.others}>
          {o}
        </p>
      ))}
    </>
  );
}

/** "1 pick by Sam is", "2 picks by Sam are": somebody else's, waiting for them. */
export function othersSaying(others: { name: string; count: number }[], one: string, many: string): string[] {
  return others.map(
    (o) => `${o.count === 1 ? `1 ${one}` : `${o.count} ${many}`} by ${o.name} ${o.count === 1 ? "is" : "are"} waiting for them to sign in on this device.`,
  );
}

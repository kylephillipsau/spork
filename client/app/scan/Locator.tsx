import { Code, Faint, Key, Lamp, Panel, Record, Records, Row, ScanInput, Stack } from "@design/index";
import { href } from "@app/routing/location";
import { Link } from "@design/index";
import type { ChromeScan } from "./useScan";
import styles from "./locator.module.css";

/**
 * The locator, in the chrome (D111).
 *
 * *"This is the navigation system, and the rail is the fallback."* The fastest
 * path to a record is the barcode on the thing in your hand, and until now the
 * only screen that could take one was Capture.
 *
 * **It never claims the caret** (D117). A screen that owns the scanner keeps
 * it; this waits to be clicked or reached with `/`. The one thing it must not
 * do is take focus from somebody mid-scan.
 *
 * Anything less certain than one subject is drawn here rather than resolved by
 * preference — with the scanned string echoed verbatim in mono, because what
 * the reader sent is evidence and an operator comparing it against a label
 * needs the characters, not a paraphrase.
 */
export function Locator({ scan }: { scan: ChromeScan }) {
  return (
    <span className={styles.locator}>
      <ScanInput
        label="Scan"
        focus="none"
        value={scan.value}
        busy={scan.busy}
        onChange={scan.type}
        onScan={() => void scan.scan()}
      />
      {scan.landing && scan.landing.kind !== "go" && (
        <Panel elevation="lifted" frame="bezel" as="div">
          <Stack gap={3}>
            <Row gap={3} wrap>
              <Lamp kind="finding" />
              <Code>{scan.landing.scanned}</Code>
              <Key size="small" onClick={scan.dismiss}>
                Dismiss
              </Key>
            </Row>
            {scan.landing.kind === "choose" && (
              <Stack gap={2}>
                <Faint>More than one match. Choose one.</Faint>
                <Records>
                  {scan.landing.options.map((o) => (
                    <Record
                      key={o.label}
                      name={
                        <Link href={href(o.path)} on="chassis">
                          {o.label}
                        </Link>
                      }
                      tags={o.detail && <Faint>{o.detail}</Faint>}
                    />
                  ))}
                </Records>
              </Stack>
            )}
            {scan.landing.kind === "unknown" && (
              <Faint>
Nothing here holds that code.
              </Faint>
            )}
            {scan.landing.kind === "unrecognised" && (
              <Faint>
Not a code this system reads.
              </Faint>
            )}
            {scan.landing.kind === "nowhere" && (
              <Faint>
That is a {scan.landing.what}. There is no screen for one yet.
              </Faint>
            )}
          </Stack>
        </Panel>
      )}
    </span>
  );
}

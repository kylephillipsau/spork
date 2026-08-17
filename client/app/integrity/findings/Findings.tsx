import {
  Band,
  Code,
  EmptySlot,
  EvidencePair,
  Face,
  FaceWell,
  Fact,
  Faint,
  Field,
  Finding,
  Key,
  Notice,
  Panel,
  Photo,
  Pill,
  Record,
  Row,
  Soft,
  Spacer,
  Stack,
  Tabs,
} from "@design/index";
import { imageUrl } from "@domain/api";
import type { DiscrepancyRow } from "@domain/types";
import { evidenceSubjectName, type FindingsDesk } from "./useFindings";
import { VIEWS } from "./views";
import styles from "./findings.module.css";

/**
 * The queue this system exists to produce.
 *
 * architecture.md calls findings *"the most valuable output"*, and D110 gives
 * them a navigation group of their own rather than a tab, because a finding is
 * the thing a warehouse management system is usually unable to say: the model
 * and the world disagree, here, by this much, and somebody saw it.
 *
 * **The evidence is on the row, not behind it.** That is the one design rule
 * this screen owes. A queue of `count_variance · GLOVE-M · investigate?` makes
 * the operator open each one to find out whether it matters; expected against
 * observed, on the row, is the difference between triage and a click-through.
 *
 * # This screen is what D135 was waiting for
 *
 * D135 said the route table survives until a screen whose state is on the
 * server needs a link — *"where a deep link restores everything it names"* —
 * and this is that screen. A selected finding is a row in the database, so
 * `/findings/{id}` restores the queue, the tab and the panel with nothing lost:
 * choosing a row is a history entry, a refresh comes back to the same evidence,
 * and the link can be sent to somebody else. The tab is derived from the
 * finding rather than carried beside it — see `views.ts`.
 */
export function Findings({ desk }: { desk: FindingsDesk }) {
  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <FaceWell>
            <Row gap={3} align="center" wrap>
              <Soft>Findings</Soft>
              <Spacer />
              {/* **Not keys.** These were four `Key`s with `live` on the
                  current one, which put three lit keys on this screen counting
                  the row's and Accept's — and `Key` says of itself that two lit
                  keys mean neither is. A tab is where you are, not what you are
                  about to do. */}
              <Tabs
                label="Which findings"
                value={desk.view}
                onChange={desk.look}
                options={VIEWS.map((v) => ({ value: v.key, label: v.label }))}
              />
            </Row>
          </FaceWell>
        </Face>

        {/* **The problem goes where the eye already is.** With a finding open
            this is on the rail, beside the act that failed; with none open —
            a link to a finding this company cannot see — the rail is not
            drawn at all, and an unexplained empty panel is worse than the
            sentence saying why. */}
        {desk.problem && !desk.selected && (
          <Notice onDismiss={desk.dismiss}>{desk.problem}</Notice>
        )}

        <Queue desk={desk} />
      </Stack>
    </Panel>
  );
}

function Queue({ desk }: { desk: FindingsDesk }) {
  if (desk.status.kind === "loading") {
    return (
      <Face>
        <Faint>Loading…</Faint>
      </Face>
    );
  }

  if (desk.status.kind === "failed") {
    return (
      <Notice>{desk.status.message}</Notice>
    );
  }

  const findings = desk.status.findings;

  return (
    <Face pad={false} as="section">
      <Band count={findings.length}>Findings</Band>
      <FaceWell>
        {findings.length === 0 ? (
          // **Not an error state, and drawn as the good one it is.** An empty
          // findings queue is the system working — the whole premise is that
          // disagreement surfaces rather than accumulates silently.
          <EmptySlot
            label="No open findings"
            note="Everything agrees with the ledger."
          />
        ) : (
          <Stack gap={3}>
            {findings.map((f) => (
              <FindingRow
                key={f.id}
                finding={f}
                selected={desk.selected?.id === f.id}
                onSelect={() => desk.select(f)}
              />
            ))}
          </Stack>
        )}
      </FaceWell>
    </Face>
  );
}

function FindingRow({
  finding,
  selected,
  onSelect,
}: {
  finding: DiscrepancyRow;
  selected: boolean;
  onSelect: () => void;
}) {
  const pair = finding.expected_quantity !== null || finding.observed_quantity !== null;

  return (
    // **Still a card, and deliberately** (D167). A finding is a piece of
    // evidence carrying the only amber in the interface (D115), not an entry on
    // a worklist — so it keeps `Finding`'s frame and its outline selection
    // rather than joining a hairline-separated `Records` list. What it takes
    // from the pattern is the row *inside* the card, which is the part six
    // screens were each deriving.
    <div className={selected ? styles.rowSelected : styles.row}>
      <Finding kind={finding.kind.replace(/_/g, " ")}>
        <Record
          /* A finding against a location or a package carries no item code —
             the kind on the card's edge already says which, so the fallback is
             the next identifier rather than a placeholder. */
          name={
            <Code>
              {finding.item_code ?? finding.location_code ?? finding.package_barcode ?? "—"}
            </Code>
          }
          tags={
            <>
              {finding.item_code && finding.location_code && (
                <Faint>{finding.location_code}</Faint>
              )}
              {finding.item_code && finding.package_barcode && (
                <Faint>{finding.package_barcode}</Faint>
              )}
              <Pill tone={finding.state === "open" ? "state" : "quiet"}>{finding.state}</Pill>
            </>
          }
          facts={
            finding.variance !== null ? (
              <Fact value={finding.variance} label="difference" />
            ) : undefined
          }
          note={
            pair || finding.detail ? (
            <Stack gap={2}>
              {/* The evidence, not a sentence about it. */}
              {pair && (
                <EvidencePair
                  expected={finding.expected_quantity ?? "—"}
                  observed={finding.observed_quantity ?? "—"}
                  observedLabel="Counted"
                />
              )}
              {finding.detail && <Faint>{finding.detail}</Faint>}
            </Stack>
            ) : undefined
          }
          meta={
            /* D11's non-repudiable floor, made visible. Null is a scheduled
               check, which is a different answer from an unnamed person. */
            <>
              <Faint>
                {finding.detected_by_name
                  ? `Found by ${finding.detected_by_name}`
                  : "Found by a scheduled check"}
              </Faint>
              <Faint>{age(finding.detected_at)}</Faint>
            </>
          }
          action={
            /* Also not lit: choosing which row to read is a selection, and the
               one lit key on this screen is Accept. */
            <Key size="small" disabled={selected} onClick={onSelect}>
              {selected ? "Showing" : "Evidence"}
            </Key>
          }
        />
      </Finding>
    </div>
  );
}

/**
 * The evidence panel: D111's third navigation mechanism.
 *
 * *"You cannot act on that from a list, and you should not have to lose your
 * place to see it."* So this goes in `DeskShell`'s rail — beside the queue,
 * never over it — and the queue keeps its scroll position while one row is
 * examined.
 */
export function FindingsRail({ desk }: { desk: FindingsDesk }) {
  const f = desk.selected;
  if (!f) return null;

  const open = f.state === "open" || f.state === "investigating";

  return (
    <Panel elevation="raised" frame="bezel" as="aside">
      <Stack gap={3}>
        <Face pad={false}>
          <Band>{f.kind.replace(/_/g, " ")}</Band>
          <FaceWell>
            <Stack gap={3}>
              <Row gap={3} align="baseline" wrap>
                {f.item_code && <Code>{f.item_code}</Code>}
                <Spacer />
                <Pill tone={f.state === "open" ? "state" : "quiet"}>{f.state}</Pill>
              </Row>

              {(f.expected_quantity !== null || f.observed_quantity !== null) && (
                <EvidencePair
                  expected={f.expected_quantity ?? "—"}
                  observed={f.observed_quantity ?? "—"}
                  observedLabel="Counted"
                />
              )}

              <Stack gap={2}>
                <Where label="Item" value={f.item_code} />
                <Where label="Bin" value={f.location_code} />
                <Where label="Carton" value={f.package_barcode} />
                <Where label="Difference" value={f.variance} />
                <Where label="Found" value={when(f.detected_at)} />
                <Where
                  label="By"
                  value={f.detected_by_name ?? "a scheduled check"}
                />
                {f.resolved_at && <Where label="Closed" value={when(f.resolved_at)} />}
                {f.resolved_by_name && <Where label="Closed by" value={f.resolved_by_name} />}
                {f.resolution_reason && <Where label="Because" value={f.resolution_reason} />}
              </Stack>

              {f.detail && <Faint>{f.detail}</Faint>}
            </Stack>
          </FaceWell>
        </Face>

        <Evidence desk={desk} />

        {open ? (
          <Face>
            <Stack gap={3}>
              {/* **Two acts, and they are not the same act.** Investigate is a
                  state change and nothing else. Accept is a manager saying the
                  model stands, which needs a reason — and resolution, which
                  writes a movement, is not on this screen at all: it goes
                  through `/adjustments` and is the Adjust row of the screen
                  map. */}
              {f.state === "open" && (
                <Row gap={3}>
                  <Key
                    size="small"
                    disabled={desk.busy}
                    onClick={() => void desk.investigate(f.id)}
                  >
                    Investigate
                  </Key>
                  <Spacer />
                  <Faint>Marks this finding as under investigation.</Faint>
                </Row>
              )}

              <Field
                label="Why this can be accepted"
                numeric={false}
                value={desk.reason}
                onChange={desk.typeReason}
                disabled={desk.busy}
                onSubmit={() => void desk.accept(f.id)}
              />
              <Row gap={3} align="center">
                <Faint>Closes the finding without moving stock.</Faint>
                <Spacer />
                <Key
                  live
                  disabled={desk.busy || desk.reason.trim() === ""}
                  onClick={() => void desk.accept(f.id)}
                >
                  Accept
                </Key>
              </Row>
            </Stack>
          </Face>
        ) : (
          <Face>
            <Faint>
              This finding is {f.state}. Closed findings are kept and are not
              reopened from here.
            </Faint>
          </Face>
        )}

        {(desk.problem || desk.said) && (
          <Notice kind={desk.problem ? "finding" : "recorded"} onDismiss={desk.dismiss}>
            {desk.problem ?? desk.said?.join(" ")}
          </Notice>
        )}
      </Stack>
    </Panel>
  );
}

/** One labelled fact, or nothing at all — an empty row is worse than absence. */
function Where({ label, value }: { label: string; value: string | null }) {
  if (!value) return null;
  return (
    <Row gap={3} align="baseline">
      <Faint>{label}</Faint>
      <Spacer />
      <Code>{value}</Code>
    </Row>
  );
}

/**
 * How long ago, in the coarsest unit that is still true.
 *
 * D114: the client formats values and the server phrases judgements. "3 days"
 * is ours; "overdue" would be theirs. Coarse on purpose — a findings queue is
 * triaged by rough age, and `2 days 4 hours 11 minutes` is three numbers to
 * read where one would do.
 */
function age(iso: string): string {
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return "—";
  const minutes = Math.max(0, Math.round((Date.now() - then) / 60000));
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `${hours} h`;
  return `${Math.round(hours / 24)} d`;
}

/** The moment itself, for the panel, where the exact time is worth having. */
function when(iso: string): string {
  const at = new Date(iso);
  return Number.isNaN(at.getTime()) ? "—" : at.toLocaleString();
}

/**
 * The photographs offered in support of this finding, and the camera.
 *
 * **On the rail, beside the numbers.** The screen's founding rule is that the
 * evidence is on the row rather than behind it, and a picture of a crushed
 * carton is the most direct evidence a finding can have — putting it behind a
 * link would be the exact thing D111's third navigation mechanism exists to
 * avoid.
 *
 * D140 makes this optional everywhere and required nowhere, so a finding with
 * no pictures draws the camera and nothing else. There is no empty state to
 * apologise for: most findings will never have one.
 */
function Evidence({ desk }: { desk: FindingsDesk }) {
  const f = desk.selected;
  if (!f) return null;

  const subject = evidenceSubjectName(f);

  return (
    <Face pad={false}>
      <Band count={f.evidence.length}>Photographs</Band>
      <FaceWell>
        <Stack gap={3}>
          {f.evidence.length > 0 && (
            <Row gap={3} wrap>
              {f.evidence.map((digest) => (
                // `own`, because a photograph taken against this finding is
                // this finding's. D141's label is about a picture borrowed
                // from a style, which is a question the pick walk asks and
                // this one cannot.
                <Photo
                  key={digest}
                  src={imageUrl(digest)}
                  source="own"
                  alt="Evidence photograph"
                />
              ))}
            </Row>
          )}

          {subject ? (
            <Row gap={3} align="center" wrap>
              {/* **What it will be a photograph of, said before it is taken.**
                  D140 refuses to infer the subject in the writer, because a
                  finding names an item, a bin and a carton without those being
                  exclusive. The screen picks the narrowest and shows which. */}
              <Soft>of {subject}</Soft>
              <Spacer />
              <label className={styles.camera}>
                <span className={styles.hidden}>Photograph {subject}</span>
                <span aria-hidden="true">Photograph</span>
                <input
                  type="file"
                  accept="image/*"
                  capture="environment"
                  disabled={desk.busy}
                  onChange={(e) => {
                    const file = e.currentTarget.files?.[0];
                    // Cleared so the same file twice still fires: a second
                    // photograph of the same thing is a second row.
                    e.currentTarget.value = "";
                    if (file) void desk.attach(file);
                  }}
                />
              </label>
            </Row>
          ) : (
            <Faint>
              This finding names no carton, bin or item, so there is nothing to
              photograph it against.
            </Faint>
          )}
        </Stack>
      </FaceWell>
    </Face>
  );
}

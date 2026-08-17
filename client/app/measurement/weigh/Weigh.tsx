import {
  Band,
  Chooser,
  Code,
  EmptySlot,
  Face,
  FaceWell,
  Fact,
  Faint,
  Field,
  Key,
  Lamp,
  Notice,
  Panel,
  Pill,
  Readout,
  Record,
  Records,
  Row,
  Soft,
  Spacer,
  Stack,
  Steel,
  grams,
} from "@design/index";
import type { ToWeigh } from "@domain/types";
import { current, type WeighBench } from "./useWeigh";

/**
 * One thing on the scale at a time.
 *
 * **The act that makes an interval mean something.** Until a weight has been
 * measured once, its `observed_at` is the date of the import that carried it —
 * so its age is unknown rather than small, and no revalidation schedule can be
 * built on it. 115 of the 116 weights on file are `transcribed`, which is why
 * the worklist behind this is two lists and not one.
 *
 * # What this screen owes, and it is one thing
 *
 * **Say what was held, before the operator types.** A fresh reading that
 * disagrees with the held figure by a factor of ten is a mistyped unit, and the
 * moment to notice is with the thing still on the scale. The server decides
 * whether a disagreement is material — a tenth, with a five-gram floor, because
 * ten per cent of a 200 g glove is arguable and ten per cent of a 300 kg pallet
 * is not — and raises a finding when it is. So this screen and Findings are two
 * ends of one act, and the answer says so with the finding's own id.
 */
export function Weigh({ bench }: { bench: WeighBench }) {
  if (bench.status.kind === "loading") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Faint>Loading…</Faint>
        </Face>
      </Panel>
    );
  }

  if (bench.status.kind === "failed") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Notice>{bench.status.message}</Notice>
      </Panel>
    );
  }

  const queue = bench.status.queue;
  const subject = current(bench.status, bench.at);
  const left = Math.max(0, queue.length - bench.at);

  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <FaceWell>
            <Row gap={3} align="baseline" wrap>
              <Soft>Weigh</Soft>
              <Spacer />
              <Pill tone={left > 0 ? "state" : "good"}>
                {left > 0 ? `${left} to weigh` : "Nothing waiting"}
              </Pill>
            </Row>
          </FaceWell>
        </Face>

        <Answer bench={bench} />

        {subject ? (
          <OnTheScale bench={bench} subject={subject} />
        ) : (
          <Face pad={false}>
            <Band>Scale</Band>
            <FaceWell>
              <EmptySlot
                label="Nothing waiting"
                note="All in date."
              />
            </FaceWell>
          </Face>
        )}

        <Next queue={queue} at={bench.at} />
      </Stack>
    </Panel>
  );
}

function OnTheScale({ bench, subject }: { bench: WeighBench; subject: ToWeigh }) {
  return (
    <Face pad={false}>
      <Band>The scale</Band>
      <FaceWell>
        <Stack gap={4}>
          <Row gap={3} align="baseline" wrap>
            <Code>{subject.code}</Code>
            <Pill tone="quiet">{subject.packaging_level}</Pill>
            {/* The server's word, not a judgement made here (D114). `never`
                means nobody has put this on a scale, whatever its apparent
                age; `overdue` means somebody did, long ago. */}
            <Pill tone="state">
              {subject.because === "never" ? "never measured" : "overdue"}
            </Pill>
            <Spacer />
            {subject.demand > 0 && (
              <>
                <Steel>{subject.demand}</Steel>
                <Faint>on order</Faint>
              </>
            )}
          </Row>

          {subject.description && <Faint>{subject.description}</Faint>}

          {/* **Held, before typed.** A reading a factor of ten out is a
              mistyped unit, and the moment to see it is now. */}
          <Row gap={5} wrap align="end">
            <Readout
              label="Held"
              value={subject.held_g === null ? "—" : grams(subject.held_g)}
              {...(subject.held_g === null ? {} : { unit: "kg" })}
            />
            {/* **A word, not a figure.** `Readout` is the number somebody
                walked over to read — tabular, large, grouped — and putting
                `transcribed` in one sets a word at the same weight as the
                weight beside it. Same category error as the expiry on the
                capture screen, and the same fix. */}
            {subject.held_method && (
              <Row gap={2} align="baseline">
                <Faint>method</Faint>
                <Pill tone="quiet">{subject.held_method}</Pill>
              </Row>
            )}
          </Row>

          <Row gap={4} align="end" wrap>
            <Field
              label="The scale says"
              width="measure"
              value={bench.reading}
              onChange={bench.type}
              disabled={bench.busy}
              onSubmit={() => void bench.record()}
            />
            {/* Two units, because a bench scale reads one or the other and the
                operator should not be converting in their head — the writer
                converts, exactly, and keeps what was typed beside it. */}
            <Chooser
              label="Unit"
              value={bench.unit}
              onChange={bench.setUnit}
              disabled={bench.busy}
              options={[
                { value: "kg", label: "kg" },
                { value: "g", label: "g" },
              ]}
            />
            <Spacer />
            <Key size="small" disabled={bench.busy} onClick={bench.skip}>
              Can't reach it
            </Key>
            <Key
              live
              disabled={bench.busy || bench.reading.trim() === ""}
              onClick={() => void bench.record()}
            >
              Record
            </Key>
          </Row>
        </Stack>
      </FaceWell>
    </Face>
  );
}

/**
 * What the last weighing said.
 *
 * **A disagreement is not an error.** It is the finding this whole side of the
 * system exists to produce, and drawing it as a failure would teach people to
 * dismiss it. So the lamp is `finding` for a disagreement and `recorded` for an
 * ordinary weighing, and the sentence says which.
 */
function Answer({ bench }: { bench: WeighBench }) {
  if (!bench.problem && !bench.recorded) return null;

  if (bench.problem) {
    return (
      <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>
    );
  }

  const r = bench.recorded;
  if (!r) return null;

  return (
    <Face>
      <Stack gap={3}>
        <Row gap={3} wrap>
          <Lamp kind={r.disagreed ? "finding" : "recorded"} />
          <span>
            {r.code} weighed {grams(r.recorded_g)} kg
            {r.disagreed ? ", and that disagrees with what was held." : "."}
          </span>
          <Spacer />
          <Key size="small" onClick={bench.dismiss}>
            Dismiss
          </Key>
        </Row>
        {r.disagreed && (
          <Row gap={5} wrap align="end">
            <Readout
              label="Was"
              value={r.previous_g === null ? "—" : grams(r.previous_g)}
              {...(r.previous_g === null ? {} : { unit: "kg" })}
            />
            <Readout label="Now" value={grams(r.recorded_g)} unit="kg" />
            {r.previous_method && (
              <Row gap={2} align="baseline">
                <Faint>previous method</Faint>
                <Pill tone="quiet">{r.previous_method}</Pill>
              </Row>
            )}
            <Spacer />
            {/* Both figures stay on file and a finding carries the pair. This
                is the comparison `proposal.md` says the system exists to make
                possible, arriving as a row somebody can chase. */}
            <Faint>Both weights are kept. A finding records the difference.</Faint>
          </Row>
        )}
      </Stack>
    </Face>
  );
}

/** What is coming, so the operator can gather more than one thing per trip. */
function Next({ queue, at }: { queue: ToWeigh[]; at: number }) {
  const rest = queue.slice(at + 1, at + 6);
  if (rest.length === 0) return null;

  return (
    <Face pad={false}>
      <Band count={Math.max(0, queue.length - at - 1)}>Next</Band>
      <FaceWell>
        <Records>
          {rest.map((s) => (
            <Record
              key={`${s.item_id ?? s.item_style_id}/${s.packaging_level}`}
              name={<Code>{s.code}</Code>}
              tags={<Faint>{s.packaging_level}</Faint>}
              facts={s.demand > 0 ? <Fact value={s.demand} label="on order" /> : undefined}
              meta={
                s.because === "never" ? <Pill tone="quiet">never measured</Pill> : undefined
              }
            />
          ))}
        </Records>
      </FaceWell>
    </Face>
  );
}

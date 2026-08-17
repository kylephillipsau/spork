import {
  Band,
  Face,
  FaceWell,
  Faint,
  Field,
  Key,
  Lamp,
  Panel,
  Pill,
  Record,
  Records,
  Row,
  Spacer,
  Stack,
} from "@design/index";
import type { Passkey } from "@domain/types";
import type { KeysBench } from "./useKeys";

/**
 * Passkeys.
 *
 * The maud page it replaces built its rows by string concatenation and escaped
 * a label with `replace(/[<>&]/g, '')` — which is a filter rather than an
 * escape, and drops the characters instead of showing them. A key called
 * `Kyle & Co` was silently renamed. React quotes it because that is what it
 * does.
 */

function when(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

function KeyRow({ passkey, bench }: { passkey: Passkey; bench: KeysBench }) {
  return (
    <Record
      name={<span>{passkey.label ?? "Unnamed key"}</span>}
      /* Synced means losing the device does not lose the key, which is the
         one property of a passkey worth surfacing without being asked. */
      tags={passkey.backup_state === true ? <Pill tone="quiet">synced</Pill> : undefined}
      meta={
        <>
          <Faint>
            {passkey.last_used_at ? `used ${when(passkey.last_used_at)}` : "never used"}
          </Faint>
          <Faint>added {when(passkey.created_at)}</Faint>
        </>
      }
      action={
        <Key size="small" disabled={bench.busy} onClick={() => void bench.revoke(passkey.id)}>
          Remove
        </Key>
      }
    />
  );
}

export function Keys({ bench }: { bench: KeysBench }) {
  if (bench.state.kind === "loading") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Faint>Loading…</Faint>
        </Face>
      </Panel>
    );
  }

  if (bench.state.kind === "failed") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Row gap={3} wrap>
            <Lamp kind="finding" />
            <span>{bench.state.message}</span>
          </Row>
        </Face>
      </Panel>
    );
  }

  const keys = bench.state.keys;

  return (
    <Stack gap={3}>
      <Panel elevation="raised" frame="bezel">
        <Face pad={false}>
          <Band>Add a key</Band>
          <FaceWell>
            {bench.supported ? (
              <Stack gap={3}>
                <Row gap={3} wrap align="end">
                  <Field
                    label="What it is"
                    numeric={false}
                    value={bench.label}
                    onChange={bench.type}
                    onSubmit={() => void bench.add()}
                  />
                  <Spacer />
                  <Key live disabled={bench.busy} onClick={() => void bench.add()}>
                    Add
                  </Key>
                </Row>
                {bench.problem && (
                  <Row gap={3} wrap>
                    <Lamp kind="finding" />
                    <span>{bench.problem}</span>
                  </Row>
                )}
              </Stack>
            ) : (
              // Said once, in place of a control that could never work.
              <Faint>This browser has no passkey support.</Faint>
            )}
          </FaceWell>
        </Face>
      </Panel>

      <Panel elevation="raised" frame="bezel">
        <Face pad={false}>
          <Band>Passkeys</Band>
          <FaceWell>
            {keys.length === 0 ? (
              <Faint>None yet.</Faint>
            ) : (
              <Records>
                {keys.map((k) => (
                  <KeyRow key={k.id} passkey={k} bench={bench} />
                ))}
              </Records>
            )}
          </FaceWell>
        </Face>
      </Panel>
    </Stack>
  );
}

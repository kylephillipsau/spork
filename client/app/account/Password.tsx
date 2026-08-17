import {
  Band,
  Code,
  Face,
  FaceWell,
  Faint,
  Field,
  Key,
  Lamp,
  Notice,
  Panel,
  Row,
  Soft,
  Spacer,
  Stack,
} from "@design/index";
import type { PasswordBench } from "./usePassword";

/**
 * Changing your own password.
 *
 * D143, and D142 owed it: until this existed the password chosen at setup was
 * the password forever, which on this project's own deployment meant a
 * fixture password committed in this repository, live on the internet.
 *
 * # Whose password, said out loud
 *
 * The screen names the person and the organisation before it asks for
 * anything. Every other screen here can leave that to the chrome, because
 * getting the wrong screen costs a click; typing a new password into the wrong
 * account costs an account.
 *
 * # The current password is asked for, and the screen says why
 *
 * Somebody already signed in reasonably asks what the old password is for. The
 * answer is short and worth printing: a session expires and a password does
 * not, so without it thirty seconds at an unlocked terminal is permanent.
 * Saying so is the difference between a field people fill in and a field people
 * resent.
 */
export function Password({ bench }: { bench: PasswordBench }) {
  if (bench.state.kind === "asking") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Faint>Checking your session…</Faint>
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

  if (bench.state.kind === "done") {
    const ended = bench.state.result.other_sessions_ended;
    return (
      <Panel elevation="lifted" frame="bezel">
        <Stack gap={3}>
          <Face>
            <Row gap={3} wrap>
              <Lamp kind="recorded" />
              <span>Password changed.</span>
              <Spacer />
            </Row>
          </Face>
          <Face>
            {/* **The count, rather than a reassuring sentence.** Somebody
                changing a password because they think one was stolen is owed
                the number: nought means there was nothing else open, and three
                means there was. A screen that says "other sessions have been
                ended" either way tells them neither. */}
            <Faint>
              {ended === 0
                ? "No other sessions were open."
                : ended === 1
                  ? "1 other session was signed out."
                  : `${ended} other sessions were signed out. Anything signed in with the old password is now out.`}
            </Faint>
          </Face>
        </Stack>
      </Panel>
    );
  }

  const who = bench.state.who;
  const d = bench.draft;
  const ready =
    d.current.length > 0 &&
    d.next.length >= 12 &&
    d.again.length > 0 &&
    !bench.mismatched;

  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <FaceWell>
            <Stack gap={2}>
              <Row gap={3} align="baseline" wrap>
                <Soft>Change your password</Soft>
              </Row>
              {/* Whose. A password screen that does not say is a password
                  screen somebody uses on the wrong account. */}
              <Row gap={3} align="baseline" wrap>
                <Faint>Signed in as</Faint>
                <Code>{who.display_name}</Code>
                <Faint>at</Faint>
                <Code>{who.tenant_name}</Code>
                {who.site_code && (
                  <>
                    <Faint>·</Faint>
                    <Code>{who.site_code}</Code>
                  </>
                )}
              </Row>
            </Stack>
          </FaceWell>
        </Face>

        <Face pad={false}>
          <Band>Current password</Band>
          <FaceWell>
            <Stack gap={3}>
              <Field
                label="Current password"
                numeric={false}
                secret
                value={d.current}
                onChange={(v) => bench.type("current", v)}
                disabled={bench.busy}
              />
              <Faint>
Ten wrong tries locks the account for fifteen minutes.
              </Faint>
            </Stack>
          </FaceWell>
        </Face>

        <Face pad={false}>
          <Band>New password</Band>
          <FaceWell>
            <Stack gap={4}>
              <Field
                label="New password"
                numeric={false}
                secret
                value={d.next}
                onChange={(v) => bench.type("next", v)}
                disabled={bench.busy}
              />
              <Field
                label="New password again"
                numeric={false}
                secret
                value={d.again}
                onChange={(v) => bench.type("again", v)}
                disabled={bench.busy}
                onSubmit={() => ready && void bench.submit()}
              />
              {bench.mismatched ? (
                <Row gap={3} wrap>
                  <Lamp kind="finding" />
                  <span>The two do not match.</span>
                </Row>
              ) : (
                <Faint>
Twelve characters or more.
                </Faint>
              )}
            </Stack>
          </FaceWell>
        </Face>

        {bench.problem && (
          <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>
        )}

        <Face>
          <Row gap={3} align="center" wrap>
            <Faint>
              Every other session signs out. This one stays.
            </Faint>
            <Spacer />
            <Key live disabled={bench.busy || !ready} onClick={() => void bench.submit()}>
              Change it
            </Key>
          </Row>
        </Face>
      </Stack>
    </Panel>
  );
}

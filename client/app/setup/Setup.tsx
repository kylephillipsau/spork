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
  Pill,
  Row,
  Soft,
  Spacer,
  Stack,
} from "@design/index";
import type { SetupBench } from "./useSetup";

/**
 * Setting a deployment up, once.
 *
 * A deployment built by migrations has a schema and nobody in it — no tenant,
 * no site, no person — so there is nothing to sign in as. This is the way in,
 * and it closes the moment it is used (D142).
 *
 * # The token is explained rather than demanded
 *
 * The obvious form asks for a token and says nothing about it, which leaves
 * somebody hunting a wiki. The token is printed in the server's log when a
 * deployment has nobody in it, so the screen says exactly that and shows the
 * command — the operator is already at a terminal, because that is where a
 * deployment comes from.
 *
 * It exists because "no persons yet" is a race with strangers rather than a
 * lock: between going live and having an administrator, a form gated only on
 * emptiness belongs to whoever finds it first. The screen does not lecture
 * about that, but it is why there is a field at all.
 */
export function Setup({ bench }: { bench: SetupBench }) {
  if (bench.state.kind === "asking") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Faint>Checking…</Faint>
        </Face>
      </Panel>
    );
  }

  if (bench.state.kind === "failed") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Row gap={3}>
            <Lamp kind="finding" />
            <span>{bench.state.message}</span>
          </Row>
        </Face>
      </Panel>
    );
  }

  if (bench.state.kind === "closed") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Stack gap={3}>
          <Face>
            <Row gap={3} wrap>
              <Lamp kind="recorded" />
              <span>This deployment is already set up.</span>
              <Spacer />
              <Pill tone="good">nothing to do</Pill>
            </Row>
          </Face>
          <Face>
            <Faint>Sign in instead.</Faint>
          </Face>
        </Stack>
      </Panel>
    );
  }

  if (bench.state.kind === "done") {
    const r = bench.state.result;
    return (
      <Panel elevation="lifted" frame="bezel">
        <Stack gap={3}>
          <Face>
            <Row gap={3} wrap>
              <Lamp kind="recorded" />
              <span>Set up. You can sign in now.</span>
            </Row>
          </Face>
          <Face pad={false}>
            <Band>Created</Band>
            <FaceWell>
              <Stack gap={2}>
                <Row gap={3} align="baseline">
                  <Faint>Organisation</Faint>
                  <Spacer />
                  <Code>{r.tenant_id}</Code>
                </Row>
                <Row gap={3} align="baseline">
                  <Faint>Site</Faint>
                  <Spacer />
                  <Code>{r.site_id}</Code>
                </Row>
                <Row gap={3} align="baseline">
                  <Faint>You</Faint>
                  <Spacer />
                  <Code>{r.person_id}</Code>
                </Row>
              </Stack>
            </FaceWell>
          </Face>
          <Face>
            {/* **Said once, here, because there is nowhere else to say it.**
                The token is gone and this endpoint is shut; the only way to add
                another person today is the database. */}
            <Faint>The setup token is destroyed. Change your password at /account.</Faint>
          </Face>
        </Stack>
      </Panel>
    );
  }

  const d = bench.details;
  const ready =
    d.token.trim() !== "" &&
    d.organisation.trim() !== "" &&
    d.siteName.trim() !== "" &&
    d.siteCode.trim() !== "" &&
    d.timezone.trim() !== "" &&
    d.displayName.trim() !== "" &&
    d.email.trim() !== "" &&
    d.password.length >= 12;

  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <FaceWell>
            <Row gap={3} align="baseline" wrap>
              <Soft>Set up this deployment</Soft>
              <Spacer />
              <Pill tone="state">nobody here yet</Pill>
            </Row>
          </FaceWell>
        </Face>

        <Face pad={false}>
          <Band>Setup token</Band>
          <FaceWell>
            <Stack gap={3}>
              <Faint>
                Printed in the server's log when it starts: the window running{" "}
                <Code>scripts\local.ps1 start</Code>. It lasts 30 minutes; restart the server
                to print a new one.
              </Faint>
              {!bench.state.status.token_ready && (
                <Row gap={3} wrap>
                  <Lamp kind="finding" />
                  <span>
                    The server has no token right now — the last one expired.
                    Restart it and read the log again.
                  </span>
                </Row>
              )}
              <Field
                label="Setup token"
                numeric={false}
                value={d.token}
                onChange={(v) => bench.type("token", v)}
                disabled={bench.busy}
              />
            </Stack>
          </FaceWell>
        </Face>

        <Face pad={false}>
          <Band>Organisation</Band>
          <FaceWell>
            <Stack gap={4}>
              <Field
                label="Organisation name"
                numeric={false}
                value={d.organisation}
                onChange={(v) => bench.type("organisation", v)}
                disabled={bench.busy}
              />
              {/* A site, because a session names one — signing in without one is
                  not possible, so a deployment without one is not usable. */}
              <Row gap={4} wrap align="end">
                <Field
                  label="First site"
                  numeric={false}
                  value={d.siteName}
                  onChange={(v) => bench.type("siteName", v)}
                  disabled={bench.busy}
                />
                <Field
                  label="Site code"
                  numeric={false}
                  width="inline"
                  value={d.siteCode}
                  onChange={(v) => bench.type("siteCode", v)}
                  disabled={bench.busy}
                />
              </Row>
              <Field
                label="Timezone"
                numeric={false}
                value={d.timezone}
                onChange={(v) => bench.type("timezone", v)}
                disabled={bench.busy}
              />
              <Faint>Decides when a day starts for despatch and counting. Guessed from this browser.</Faint>
            </Stack>
          </FaceWell>
        </Face>

        <Face pad={false}>
          <Band>Your account</Band>
          <FaceWell>
            <Stack gap={4}>
              <Field
                label="Your name"
                numeric={false}
                value={d.displayName}
                onChange={(v) => bench.type("displayName", v)}
                disabled={bench.busy}
              />
              <Field
                label="Email"
                numeric={false}
                value={d.email}
                onChange={(v) => bench.type("email", v)}
                disabled={bench.busy}
              />
              <Field
                label="Password"
                numeric={false}
                secret
                value={d.password}
                onChange={(v) => bench.type("password", v)}
                disabled={bench.busy}
                onSubmit={() => ready && void bench.submit()}
              />
              <Faint>Twelve characters or more. Change it later at /account, which also signs out every other session.</Faint>
            </Stack>
          </FaceWell>
        </Face>

        {bench.problem && (
          <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>
        )}

        <Face>
          <Row gap={3} align="center" wrap>
            <Faint>The token can only be used once.</Faint>
            <Spacer />
            <Key live disabled={bench.busy || !ready} onClick={() => void bench.submit()}>
              Set up
            </Key>
          </Row>
        </Face>
      </Stack>
    </Panel>
  );
}

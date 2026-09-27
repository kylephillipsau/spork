import { CircleCheck } from "lucide-react";

import { Alert, Badge, Button, Card, Fact, Facts, Inline, Link, Section, Spinner, Stack, TextField } from "@ui/index";
import { href } from "@app/routing/location";
import { useNavigate } from "@app/routing/Router";

import type { SetupBench } from "./useSetup";
import s from "@app/session/session-pages.module.css";

/**
 * Setting a deployment up, once (D142, D171 layout; logic in useSetup).
 *
 * A deployment built by migrations has a schema and nobody in it, so there is
 * nothing to sign in as. This is the way in, and it closes once used.
 *
 * The token exists because "no persons yet" is a race with strangers rather
 * than a lock: a form gated only on emptiness belongs to whoever finds it
 * first. The screen says where the token is printed rather than just asking
 * for it, because the operator is at the terminal that started the server.
 */
export function SetupPage({ bench }: { bench: SetupBench }) {
  const navigate = useNavigate();
  if (bench.state.kind === "asking") {
    return (
      <Card>
        <div className={s.centre}>
          <Spinner label="Checking" />
        </div>
      </Card>
    );
  }

  if (bench.state.kind === "failed") {
    return (
      <Card>
        <Alert tone="danger">{bench.state.message}</Alert>
      </Card>
    );
  }

  if (bench.state.kind === "closed") {
    return (
      <Card>
        <Stack gap={4}>
          <div>
            <h1 className={s.title}>Already set up</h1>
            <p className={s.subtitle}>This deployment is already set up.</p>
          </div>
          <Link href={href("/sign-in")}>Sign in</Link>
        </Stack>
      </Card>
    );
  }

  if (bench.state.kind === "done") {
    const r = bench.state.result;
    return (
      <Card>
        <Stack gap={5}>
          <div className={s.done}>
            <CircleCheck aria-hidden />
            <h1 className={s.title}>Set up</h1>
          </div>
          <p className={s.subtitle}>You can sign in now.</p>
          <Facts columns={1}>
            <Fact label="Organisation" mono>
              {r.tenant_id}
            </Fact>
            <Fact label="Site" mono>
              {r.site_id}
            </Fact>
            <Fact label="You" mono>
              {r.person_id}
            </Fact>
          </Facts>
          {/* Said once, here: the token is gone and this endpoint is shut. */}
          <p className={s.subtitle}>The setup token has been used. Change your password at /account.</p>
          <Button variant="primary" size="lg" block onClick={() => navigate("/sign-in")}>
            Sign in
          </Button>
        </Stack>
      </Card>
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
    <Card>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (ready && !bench.busy) void bench.submit();
        }}
      >
        <Stack gap={6}>
          <div>
            <Inline gap={2}>
              <h1 className={s.title}>Set up this deployment</h1>
              <Badge tone="accent">New</Badge>
            </Inline>
            <p className={s.subtitle}>The token can only be used once.</p>
          </div>

          <Section title="Setup token">
            <Stack gap={3}>
              <p className={s.subtitle}>
                Shown in the server log at startup (the window running <code>scripts\local.ps1 start</code>). Valid for
                30 minutes. Restart the server for a new one.
              </p>
              {!bench.state.status.token_ready && (
                <Alert tone="warning">The setup token has expired. Restart the server for a new one.</Alert>
              )}
              <TextField
                aria-label="Setup token"
                autoComplete="off"
                spellCheck={false}
                value={d.token}
                onChange={(e) => bench.type("token", e.target.value)}
                disabled={bench.busy}
                autoFocus
              />
            </Stack>
          </Section>

          <Section title="Organisation">
            <Stack gap={4}>
              <TextField
                label="Organisation name"
                value={d.organisation}
                onChange={(e) => bench.type("organisation", e.target.value)}
                disabled={bench.busy}
              />
              {/* A site, because a session names one: without one nobody can sign in. */}
              <div className={s.pair}>
                <TextField
                  label="First site"
                  value={d.siteName}
                  onChange={(e) => bench.type("siteName", e.target.value)}
                  disabled={bench.busy}
                />
                <TextField
                  label="Site code"
                  value={d.siteCode}
                  onChange={(e) => bench.type("siteCode", e.target.value)}
                  disabled={bench.busy}
                />
              </div>
              <TextField
                label="Timezone"
                hint="Sets when the working day starts for despatch and counts. Detected from this browser."
                value={d.timezone}
                onChange={(e) => bench.type("timezone", e.target.value)}
                disabled={bench.busy}
              />
            </Stack>
          </Section>

          <Section title="Your account">
            <Stack gap={4}>
              <TextField
                label="Your name"
                autoComplete="name"
                value={d.displayName}
                onChange={(e) => bench.type("displayName", e.target.value)}
                disabled={bench.busy}
              />
              <TextField
                label="Email"
                type="email"
                autoComplete="username"
                value={d.email}
                onChange={(e) => bench.type("email", e.target.value)}
                disabled={bench.busy}
              />
              <TextField
                label="Password"
                type="password"
                autoComplete="new-password"
                hint="At least 12 characters. You can change it later at /account."
                value={d.password}
                onChange={(e) => bench.type("password", e.target.value)}
                disabled={bench.busy}
              />
            </Stack>
          </Section>

          {bench.problem && (
            <Alert tone="danger" onDismiss={bench.dismiss}>
              {bench.problem}
            </Alert>
          )}

          <Button type="submit" variant="primary" size="lg" block loading={bench.busy} disabled={!ready}>
            Set up
          </Button>
        </Stack>
      </form>
    </Card>
  );
}

import { CircleAlert, CircleCheck, Fingerprint } from "lucide-react";

import { Button, Card, Checkbox, Stack, TextField } from "@ui/index";

import type { SignInBench } from "./useSignIn";
import s from "./session-pages.module.css";

/** Sign in with email and password, or a passkey (D171 layout; logic in useSignIn). */
export function SignInPage({ bench }: { bench: SignInBench }) {
  if (bench.state.kind === "done") {
    return (
      <Card>
        <p className={s.done}>
          <CircleCheck aria-hidden /> Signed in. Loading…
        </p>
      </Card>
    );
  }

  const choosing = bench.state.kind === "choose";
  const withKey = choosing && bench.via === "key";
  const ready = withKey
    ? bench.tenant !== null
    : bench.credentials.email.trim() !== "" && bench.credentials.password !== "" && (!choosing || bench.tenant !== null);
  const carryOn = () => (withKey ? void bench.useKey() : void bench.submit());

  return (
    <Card>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (ready && !bench.busy) carryOn();
        }}
      >
        <Stack gap={5}>
          <div>
            <h1 className={s.title}>{choosing ? "Choose an organisation" : "Sign in"}</h1>
            <p className={s.subtitle}>
              {choosing ? "Your account belongs to more than one organisation." : "Use your work email and password."}
            </p>
          </div>

          {!withKey && (
            <Stack gap={4}>
              <TextField
                label="Email"
                type="email"
                autoComplete="username"
                value={bench.credentials.email}
                onChange={(e) => bench.type("email", e.target.value)}
                disabled={bench.busy || choosing}
                autoFocus
              />
              <TextField
                label="Password"
                type="password"
                autoComplete="current-password"
                value={bench.credentials.password}
                onChange={(e) => bench.type("password", e.target.value)}
                disabled={bench.busy || choosing}
              />
              {!choosing && (
                <Checkbox
                  label="Keep me signed in on this device"
                  checked={bench.remember}
                  onCheckedChange={bench.setRemember}
                  disabled={bench.busy}
                />
              )}
            </Stack>
          )}

          {bench.state.kind === "choose" && (
            <div className={s.choices} role="radiogroup" aria-label="Organisation">
              {bench.state.tenants.map((t) => (
                <button
                  key={t.tenant_id}
                  type="button"
                  role="radio"
                  aria-checked={bench.tenant === t.tenant_id}
                  className={s.choice}
                  onClick={() => bench.choose(t.tenant_id)}
                >
                  <span className={s.radio} aria-hidden />
                  {t.name}
                </button>
              ))}
            </div>
          )}

          {bench.problem && (
            <p className={s.problem} role="alert">
              <CircleAlert aria-hidden />
              {bench.problem}
            </p>
          )}

          <Stack gap={3}>
            <Button type="submit" variant="primary" size="lg" block loading={bench.busy} disabled={!ready}>
              {choosing ? "Continue" : "Sign in"}
            </Button>
            {!choosing && bench.keys && (
              <>
                <div className={s.or}>
                  <span>or</span>
                </div>
                <Button size="lg" block icon={<Fingerprint />} disabled={bench.busy} onClick={() => void bench.useKey()}>
                  Sign in with a passkey
                </Button>
              </>
            )}
          </Stack>
        </Stack>
      </form>
    </Card>
  );
}

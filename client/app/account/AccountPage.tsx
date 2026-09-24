import { Avatar, Button, Card, PageHeader, Skeleton, TextField, cx } from "@ui/index";
import { Alert } from "@app/admin/Alert";

import type { PasswordBench } from "./usePassword";
import s from "@app/admin/settings.module.css";

/** Who you are signed in as, and changing your password (D143, D171). */
export function AccountPage({ bench }: { bench: PasswordBench }) {
  const st = bench.state;
  const who = st.kind === "ready" || st.kind === "done" ? st.who : null;
  const ready = bench.draft.current !== "" && bench.draft.next !== "" && bench.draft.again !== "" && !bench.mismatched;

  return (
    <div className={cx(s.page, s.narrowPage)}>
      <PageHeader title="Account" description="Your profile and password." />

      {st.kind === "failed" && <Alert tone="danger">{st.message}</Alert>}

      <Card title="Profile">
        {who ? (
          <div className={s.formRow}>
            <Avatar name={who.display_name} size={40} />
            <div>
              <div>
                <strong>{who.display_name}</strong>
              </div>
              <div className={s.muted}>
                {who.tenant_name}
                {who.site_code ? ` · ${who.site_code}` : ""}
              </div>
            </div>
          </div>
        ) : (
          <Skeleton width="40%" />
        )}
      </Card>

      <Card title="Change password" description="Changing it signs out every other session.">
        {st.kind === "done" && (
          <div className={s.stack}>
            <Alert tone="success">
              Password changed.{" "}
              {st.result.other_sessions_ended === 0
                ? "No other sessions were signed in."
                : st.result.other_sessions_ended === 1
                  ? "1 other session was signed out."
                  : `${st.result.other_sessions_ended} other sessions were signed out.`}
            </Alert>
          </div>
        )}
        {st.kind === "ready" && (
          <form
            className={s.stack}
            onSubmit={(e) => {
              e.preventDefault();
              if (ready) void bench.submit();
            }}
          >
            <TextField
              label="Current password"
              type="password"
              autoComplete="current-password"
              value={bench.draft.current}
              onChange={(e) => bench.type("current", e.target.value)}
              disabled={bench.busy}
            />
            <TextField
              label="New password"
              type="password"
              autoComplete="new-password"
              hint="At least 12 characters."
              value={bench.draft.next}
              onChange={(e) => bench.type("next", e.target.value)}
              disabled={bench.busy}
            />
            <TextField
              label="Confirm new password"
              type="password"
              autoComplete="new-password"
              value={bench.draft.again}
              onChange={(e) => bench.type("again", e.target.value)}
              error={bench.mismatched ? "The two new passwords do not match." : undefined}
              disabled={bench.busy}
            />
            {bench.problem && (
              <Alert tone="danger" onDismiss={bench.dismiss}>
                {bench.problem}
              </Alert>
            )}
            <div className={s.actions}>
              <Button type="submit" variant="primary" loading={bench.busy} disabled={!ready}>
                Change password
              </Button>
            </div>
          </form>
        )}
        {st.kind === "asking" && <Skeleton width="60%" />}
      </Card>
    </div>
  );
}

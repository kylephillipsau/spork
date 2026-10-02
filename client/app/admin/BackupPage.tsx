import { useState } from "react";
import { DatabaseBackup } from "lucide-react";

import { Alert, Button, Card, Fact, Facts, Page, PageHeader, Skeleton, Stack, TextField } from "@ui/index";

import { LEAST_PASSWORD, type BackupBench } from "./useBackup";
import s from "./settings.module.css";

/** Bytes, as a person reads a file's size. */
function size(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/**
 * A backup of the whole workspace (D193): every item, measurement, order and
 * stock record, the people who sign in, and every photo, in one zip encrypted
 * with a password set here. Restored with a command into an empty Spork.
 */
export function BackupPage({ bench }: { bench: BackupBench }) {
  const [password, setPassword] = useState("");
  const [again, setAgain] = useState("");
  const short = password.length > 0 && password.length < LEAST_PASSWORD;
  const differs = again.length > 0 && again !== password;
  const ready = password.length >= LEAST_PASSWORD && again === password;

  return (
    <Page>
      <PageHeader title="Backup" description="Download everything in this workspace as one encrypted file." />
      <Stack gap={4}>
        {bench.state.kind === "failed" && <Alert tone="danger">{bench.state.message}</Alert>}
        <Card title="Download a backup" description="Items, measurements, orders, stock, sign-ins and every photo.">
          <Stack gap={4}>
            {bench.state.kind === "loading" ? (
              <Skeleton width="60%" />
            ) : bench.state.kind === "ready" ? (
              <Facts columns={3}>
                <Fact label="Items">{bench.state.summary.items.toLocaleString()}</Fact>
                <Fact label="Photos">
                  {bench.state.summary.photos.toLocaleString()} ({size(bench.state.summary.photo_bytes)})
                </Fact>
                <Fact label="Database version">{Number(bench.state.summary.schema.slice(11, 17))}</Fact>
              </Facts>
            ) : null}
            {bench.problem && (
              <Alert tone="danger" onDismiss={bench.dismiss}>
                {bench.problem}
              </Alert>
            )}
            {bench.asked && !bench.problem && (
              <Alert tone="info">Your browser saves the file when it is ready. A workspace with many photos takes a minute or two.</Alert>
            )}
            <form
              className={s.stack}
              onSubmit={(e) => {
                e.preventDefault();
                if (ready) bench.download(password);
              }}
            >
              <TextField
                label="Password"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                hint={`At least ${LEAST_PASSWORD} characters. Without it the backup cannot be opened, so keep it somewhere safe.`}
                error={short ? `At least ${LEAST_PASSWORD} characters` : undefined}
              />
              <TextField
                label="Password again"
                type="password"
                autoComplete="new-password"
                value={again}
                onChange={(e) => setAgain(e.target.value)}
                error={differs ? "The two do not match" : undefined}
              />
              <div>
                <Button type="submit" variant="primary" icon={<DatabaseBackup />} disabled={!ready || bench.state.kind !== "ready"}>
                  Download backup
                </Button>
              </div>
            </form>
          </Stack>
        </Card>
        <Card title="Restoring a backup" description="Into a fresh Spork of the same version, at the computer that runs it.">
          <ol className={s.steps}>
            <li>
              Install Spork at the version the backup was taken with: <code>scripts\local.ps1 setup</code>.
            </li>
            <li>
              Run <code>scripts\local.ps1 restore</code> with the backup file, and type its password.
            </li>
            <li>
              Start Spork with <code>scripts\local.ps1 start</code>. Everyone signs in as before.
            </li>
          </ol>
        </Card>
      </Stack>
    </Page>
  );
}

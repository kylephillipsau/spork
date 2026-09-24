import { useState } from "react";
import { Fingerprint, KeyRound } from "lucide-react";

import { Alert, Badge, Button, Card, DataTable, Dialog, EmptyState, Page, PageHeader, TextField, type Column } from "@ui/index";
import type { Passkey } from "@domain/types";
import { Faint, shortDate } from "@app/common/cells";

import type { KeysBench } from "./useKeys";
import s from "@app/admin/settings.module.css";

/** Passkeys on this account: sign in with this device instead of a password. */
export function KeysPage({ bench }: { bench: KeysBench }) {
  const [removing, setRemoving] = useState<Passkey | null>(null);
  const keys = bench.state.kind === "ready" ? bench.state.keys : [];

  const columns: Column<Passkey>[] = [
    { key: "label", header: "Name", cell: (k) => k.label ?? <Faint>Unnamed passkey</Faint>, sort: (k) => k.label, grow: true },
    {
      key: "synced",
      header: "Synced",
      cell: (k) =>
        k.backup_state === null ? <Faint>—</Faint> : <Badge tone={k.backup_state ? "info" : "neutral"}>{k.backup_state ? "Synced" : "This device only"}</Badge>,
      width: "150px",
    },
    { key: "added", header: "Added", cell: (k) => shortDate(k.created_at), sort: (k) => k.created_at, width: "100px" },
    {
      key: "used",
      header: "Last used",
      cell: (k) => (k.last_used_at ? shortDate(k.last_used_at) : <Faint>Never</Faint>),
      sort: (k) => k.last_used_at,
      width: "100px",
    },
    {
      key: "action",
      header: "",
      cell: (k) => (
        <Button size="sm" variant="ghost" onClick={() => setRemoving(k)}>
          Remove
        </Button>
      ),
      align: "right",
      width: "100px",
    },
  ];

  return (
    <Page narrow>
      <PageHeader title="Passkeys" description="Sign in with this device's fingerprint, face or PIN instead of a password." />

      <Card title="Add a passkey">
        {bench.supported ? (
          <form
            className={s.formRow}
            onSubmit={(e) => {
              e.preventDefault();
              void bench.add();
            }}
          >
            <div className={s.grow}>
              <TextField
                label="Name"
                placeholder="e.g. Office PC"
                value={bench.label}
                onChange={(e) => bench.type(e.target.value)}
                disabled={bench.busy}
              />
            </div>
            <Button type="submit" variant="primary" icon={<Fingerprint />} loading={bench.busy}>
              Add passkey
            </Button>
          </form>
        ) : (
          <p className={s.muted}>This browser does not support passkeys. Use a current version of Chrome, Edge, Firefox or Safari.</p>
        )}
        {bench.problem && (
          <div className={s.below}>
            <Alert tone="danger">{bench.problem}</Alert>
          </div>
        )}
      </Card>

      <Card title="Your passkeys" padded={false}>
        {bench.state.kind === "failed" ? (
          <div className={s.inset}>
            <Alert tone="danger">{bench.state.message}</Alert>
          </div>
        ) : (
          <DataTable
            aria-label="Passkeys"
            columns={columns}
            rows={keys}
            rowKey={(k) => k.id}
            loading={bench.state.kind === "loading"}
            empty={<EmptyState icon={<KeyRound />} title="No passkeys yet" description="Add one to sign in without a password." />}
          />
        )}
      </Card>

      <Dialog
        open={removing !== null}
        onOpenChange={(o) => {
          if (!o) setRemoving(null);
        }}
        title="Remove this passkey?"
        description="You will no longer be able to sign in with it. Your password still works."
        footer={
          <>
            <Button onClick={() => setRemoving(null)}>Cancel</Button>
            <Button
              variant="danger"
              loading={bench.busy}
              onClick={() => {
                if (!removing) return;
                void bench.revoke(removing.id).then(() => setRemoving(null));
              }}
            >
              Remove passkey
            </Button>
          </>
        }
      />
    </Page>
  );
}

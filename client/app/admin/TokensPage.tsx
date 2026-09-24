import { useState } from "react";
import { Check, Copy, KeyRound, Plus } from "lucide-react";

import {
  Badge,
  Button,
  Card,
  DataTable,
  Dialog,
  EmptyState,
  PageHeader,
  TextField,
  useToast,
  type Column,
} from "@ui/index";
import type { ApiToken } from "@domain/types";
import { Faint, shortDate } from "@app/common/cells";

import { standing, type TokensBench } from "./useTokens";
import { Alert } from "./Alert";
import s from "./settings.module.css";

const STANDING = {
  live: { label: "Active", tone: "success" },
  expired: { label: "Expired", tone: "neutral" },
  revoked: { label: "Withdrawn", tone: "neutral" },
} as const;

/**
 * Import tokens (D158, D171): credentials for machines that send data to
 * Spork — the NetSuite userscript, a nightly loader. The secret is shown once.
 */
export function TokensPage({ bench }: { bench: TokensBench }) {
  const [withdrawing, setWithdrawing] = useState<ApiToken | null>(null);
  const tokens = bench.state.kind === "ready" ? bench.state.tokens : [];

  const columns: Column<ApiToken>[] = [
    { key: "label", header: "Used for", cell: (t) => t.label, sort: (t) => t.label, grow: true },
    {
      key: "status",
      header: "Status",
      cell: (t) => {
        const st = STANDING[standing(t)];
        return (
          <Badge tone={st.tone} dot>
            {st.label}
          </Badge>
        );
      },
      sort: (t) => standing(t),
      width: "120px",
    },
    { key: "created", header: "Created", cell: (t) => shortDate(t.created_at), sort: (t) => t.created_at, width: "100px" },
    {
      key: "used",
      header: "Last used",
      cell: (t) => (t.last_used_at ? shortDate(t.last_used_at) : <Faint>Never</Faint>),
      sort: (t) => t.last_used_at,
      width: "100px",
    },
    { key: "expires", header: "Expires", cell: (t) => shortDate(t.expires_at), sort: (t) => t.expires_at, width: "100px" },
    {
      key: "action",
      header: "",
      cell: (t) =>
        standing(t) === "live" ? (
          <Button size="sm" variant="ghost" onClick={() => setWithdrawing(t)}>
            Withdraw
          </Button>
        ) : null,
      align: "right",
      width: "110px",
    },
  ];

  return (
    <div className={s.page}>
      <PageHeader
        title="Import tokens"
        description="Credentials for machines that send data to Spork, such as the NetSuite userscript."
      />

      {bench.minted && <Minted token={bench.minted.token} label={bench.minted.label} onDone={bench.dismiss} />}

      <Card title="New token">
        <form
          className={s.formRow}
          onSubmit={(e) => {
            e.preventDefault();
            void bench.mint();
          }}
        >
          <div className={s.grow}>
            <TextField
              label="Used for"
              placeholder="e.g. NetSuite userscript on the office PC"
              value={bench.draft.label}
              onChange={(e) => bench.type("label", e.target.value)}
              disabled={bench.busy}
            />
          </div>
          <div className={s.narrow}>
            <TextField
              label="Valid for"
              type="number"
              min={1}
              max={365}
              placeholder="90"
              trailing="days"
              value={bench.draft.days}
              onChange={(e) => bench.type("days", e.target.value)}
              disabled={bench.busy}
            />
          </div>
          <Button type="submit" variant="primary" icon={<Plus />} loading={bench.busy} disabled={!bench.draft.label.trim()}>
            Create token
          </Button>
        </form>
        {bench.problem && (
          <div className={s.below}>
            <Alert tone="danger" onDismiss={bench.dismiss}>
              {bench.problem}
            </Alert>
          </div>
        )}
      </Card>

      <Card title="Tokens" padded={false}>
        {bench.state.kind === "failed" ? (
          <div className={s.inset}>
            <Alert tone="danger">{bench.state.message}</Alert>
          </div>
        ) : (
          <DataTable
            aria-label="Import tokens"
            columns={columns}
            rows={tokens}
            rowKey={(t) => t.id}
            loading={bench.state.kind === "loading"}
            initialSort={{ key: "created", direction: "desc" }}
            empty={<EmptyState icon={<KeyRound />} title="No tokens yet" description="Create one for each machine that sends data." />}
          />
        )}
      </Card>

      <Dialog
        open={withdrawing !== null}
        onOpenChange={(o) => {
          if (!o) setWithdrawing(null);
        }}
        title="Withdraw this token?"
        description={withdrawing ? `“${withdrawing.label}” stops working immediately. This cannot be undone.` : undefined}
        footer={
          <>
            <Button onClick={() => setWithdrawing(null)}>Cancel</Button>
            <Button
              variant="danger"
              loading={bench.busy}
              onClick={() => {
                if (!withdrawing) return;
                void bench.revoke(withdrawing.id).then(() => setWithdrawing(null));
              }}
            >
              Withdraw token
            </Button>
          </>
        }
      />
    </div>
  );
}

/** The one moment the secret exists in a form anybody can use. */
function Minted({ token, label, onDone }: { token: string; label: string; onDone: () => void }) {
  const toast = useToast();
  const [copied, setCopied] = useState(false);
  return (
    <section className={s.minted}>
      <div className={s.mintedHead}>
        <div>
          <h2 className={s.mintedTitle}>Token created for “{label}”</h2>
          <p className={s.muted}>Copy it now. It will not be shown again.</p>
        </div>
        <Button size="sm" variant="ghost" onClick={onDone}>
          Done
        </Button>
      </div>
      <div className={s.secret}>
        <code className={s.secretValue}>{token}</code>
        <Button
          size="sm"
          icon={copied ? <Check /> : <Copy />}
          onClick={() => {
            void navigator.clipboard?.writeText(token).then(
              () => {
                setCopied(true);
                toast({ title: "Token copied", tone: "success" });
              },
              () => toast({ title: "Could not copy", description: "Select the token and copy it by hand.", tone: "danger" }),
            );
          }}
        >
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
    </section>
  );
}

import { useState } from "react";
import { RefreshCw, UserPlus, Users } from "lucide-react";

import { Alert, Badge, Button, Card, DataTable, Dialog, EmptyState, Page, PageHeader, Select, TextField, type Column } from "@ui/index";
import type { WorkspacePerson } from "@domain/types";
import { Faint, shortDate } from "@app/common/cells";

import { firstPassword, LEAST_PASSWORD, type PeopleBench } from "./usePeople";
import s from "./settings.module.css";

const ROLES = [
  { value: "operator", label: "Operator" },
  { value: "administrator", label: "Administrator" },
] as const;

/** How somebody signs in, in a few words. */
function signsIn(p: WorkspacePerson): string {
  const keys = p.passkeys === 1 ? "1 passkey" : `${p.passkeys} passkeys`;
  if (p.password && p.passkeys > 0) return `Password, ${keys}`;
  if (p.password) return "Password";
  if (p.passkeys > 0) return keys;
  return "Not yet";
}

/**
 * The people of the workspace (D205): each picker and packer signs in as
 * themselves, so a run, a pick and a pack name who did them. An administrator
 * adds a person with a first password to hand them, says what each is, and
 * takes somebody out; what they recorded stays theirs.
 */
export function PeoplePage({ bench }: { bench: PeopleBench }) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<WorkspacePerson["role"]>("operator");
  const [password, setPassword] = useState(firstPassword);
  const [removing, setRemoving] = useState<WorkspacePerson | null>(null);
  const people = bench.state.kind === "ready" ? bench.state.people : [];
  const short = password.length > 0 && password.length < LEAST_PASSWORD;
  const ready = name.trim() !== "" && email.includes("@") && password.length >= LEAST_PASSWORD;

  const columns: Column<WorkspacePerson>[] = [
    {
      key: "name",
      header: "Name",
      cell: (p) => (
        <>
          {p.display_name} {p.you && <Badge tone="info">You</Badge>}
        </>
      ),
      sort: (p) => p.display_name.toLowerCase(),
      grow: true,
    },
    { key: "email", header: "Email", cell: (p) => p.email ?? <Faint>None</Faint>, sort: (p) => p.email ?? "" },
    {
      key: "role",
      header: "Role",
      cell: (p) =>
        p.left_at || p.you ? (
          ROLES.find((r) => r.value === p.role)?.label
        ) : (
          <Select
            size="sm"
            aria-label={`What ${p.display_name} is`}
            value={p.role}
            options={ROLES}
            disabled={bench.busy}
            onValueChange={(v) => {
              if (v !== p.role) void bench.setRole(p, v as WorkspacePerson["role"]);
            }}
          />
        ),
      sort: (p) => p.role,
      width: "170px",
    },
    { key: "signs", header: "Signs in with", cell: (p) => signsIn(p), width: "150px" },
    {
      key: "joined",
      header: "Since",
      cell: (p) =>
        p.left_at ? (
          <Badge tone="neutral">Left {shortDate(p.left_at)}</Badge>
        ) : (
          shortDate(p.joined_at)
        ),
      sort: (p) => p.left_at ?? p.joined_at,
      width: "120px",
    },
    {
      key: "action",
      header: "",
      cell: (p) =>
        p.left_at ? (
          p.email ? (
            <Button
              size="sm"
              variant="ghost"
              icon={<RefreshCw />}
              disabled={bench.busy}
              onClick={() => void bench.add({ name: p.display_name, email: p.email ?? "", password: "", role: p.role })}
            >
              Bring back
            </Button>
          ) : null
        ) : p.you ? null : (
          <Button size="sm" variant="ghost" disabled={bench.busy} onClick={() => setRemoving(p)}>
            Remove
          </Button>
        ),
      align: "right",
      width: "130px",
    },
  ];

  return (
    <Page>
      <PageHeader title="People" description="Who can sign in to this workspace, each as themselves, and what they can do." />

      <Card title="Add a person" description="Give them the first password. They change it under Account, and can add a passkey there.">
        <form
          className={s.stack}
          onSubmit={(e) => {
            e.preventDefault();
            if (!ready) return;
            void bench.add({ name, email, password, role }).then((added) => {
              if (!added) return;
              setName("");
              setEmail("");
              setRole("operator");
              setPassword(firstPassword());
            });
          }}
        >
          <div className={s.formRow}>
            <div className={s.grow}>
              <TextField label="Name" autoComplete="off" value={name} onChange={(e) => setName(e.target.value)} disabled={bench.busy} />
            </div>
            <div className={s.grow}>
              <TextField
                label="Email"
                type="email"
                autoComplete="off"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                disabled={bench.busy}
              />
            </div>
            <div className={s.narrow}>
              <Select label="Role" value={role} options={ROLES} onValueChange={(v) => setRole(v as WorkspacePerson["role"])} disabled={bench.busy} />
            </div>
          </div>
          <div className={s.formRow}>
            <div className={s.grow}>
              <TextField
                label="First password"
                autoComplete="off"
                spellCheck={false}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                disabled={bench.busy}
                hint={`At least ${LEAST_PASSWORD} characters. Write it down for them now. If they already sign in to another workspace, they keep their own.`}
                error={short ? `At least ${LEAST_PASSWORD} characters` : undefined}
              />
            </div>
            <Button type="button" icon={<RefreshCw />} onClick={() => setPassword(firstPassword())} disabled={bench.busy}>
              Another
            </Button>
            <Button type="submit" variant="primary" icon={<UserPlus />} loading={bench.busy} disabled={!ready}>
              Add person
            </Button>
          </div>
        </form>
      </Card>

      {bench.problem && (
        <Alert tone="danger" onDismiss={bench.dismiss}>
          {bench.problem}
        </Alert>
      )}
      {bench.said && !bench.problem && (
        <Alert tone="success" onDismiss={bench.dismiss}>
          {bench.said}
        </Alert>
      )}

      <Card title="People" count={people.filter((p) => !p.left_at).length} padded={false}>
        {bench.state.kind === "failed" ? (
          <div className={s.inset}>
            <Alert tone="danger">{bench.state.message}</Alert>
          </div>
        ) : (
          <DataTable
            aria-label="People"
            columns={columns}
            rows={people}
            rowKey={(p) => p.person_id}
            loading={bench.state.kind === "loading"}
            empty={<EmptyState icon={<Users />} title="Nobody yet" description="Add the first person above." />}
          />
        )}
      </Card>

      <Dialog
        open={removing !== null}
        onOpenChange={(o) => {
          if (!o) setRemoving(null);
        }}
        title={removing ? `Take ${removing.display_name} out of the workspace?` : "Take them out?"}
        description="They are signed out everywhere at once and can't sign in again here. What they recorded still names them, and you can bring them back."
        footer={
          <>
            <Button onClick={() => setRemoving(null)}>Cancel</Button>
            <Button
              variant="danger"
              loading={bench.busy}
              onClick={() => {
                if (!removing) return;
                void bench.remove(removing).then(() => setRemoving(null));
              }}
            >
              Take them out
            </Button>
          </>
        }
      />
    </Page>
  );
}

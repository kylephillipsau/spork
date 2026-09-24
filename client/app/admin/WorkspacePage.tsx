import { Warehouse } from "lucide-react";

import { Badge, Card, DataTable, EmptyState, PageHeader, Skeleton, type Column } from "@ui/index";
import type { WorkspaceSite } from "@domain/types";
import { Faint, Progress, shortDate } from "@app/common/cells";

import type { WorkspaceBench } from "./useWorkspace";
import { Alert } from "./Alert";
import s from "./settings.module.css";

/** The organisation and its warehouses, as the imports have described them. */
export function WorkspacePage({ bench }: { bench: WorkspaceBench }) {
  const st = bench.state;
  const ws = st.kind === "ready" ? st.workspace : null;

  return (
    <div className={s.page}>
      <PageHeader title="Workspace" description="Your organisation and its warehouses." />

      {st.kind === "failed" && <Alert tone="danger">{st.message}</Alert>}

      <Card title="Organisation">
        {ws ? (
          <dl className={s.facts}>
            <div>
              <dt>Name</dt>
              <dd>{ws.organisation.name}</dd>
            </div>
            <div>
              <dt>Short name</dt>
              <dd className={s.mono}>{ws.organisation.slug}</dd>
            </div>
            <div>
              <dt>Status</dt>
              <dd>
                <Badge tone={ws.organisation.active ? "success" : "neutral"} dot>
                  {ws.organisation.active ? "Active" : "Inactive"}
                </Badge>
              </dd>
            </div>
            <div>
              <dt>Created</dt>
              <dd>{shortDate(ws.organisation.created_at)}</dd>
            </div>
          </dl>
        ) : (
          <Skeleton width="50%" />
        )}
      </Card>

      <Card title="Warehouses" padded={false}>
        <DataTable
          aria-label="Warehouses"
          columns={COLUMNS}
          rows={ws?.sites ?? []}
          rowKey={(x) => x.id}
          loading={st.kind === "loading"}
          empty={<EmptyState icon={<Warehouse />} title="No warehouses yet" description="Import a bin list to create them." />}
        />
      </Card>
    </div>
  );
}

const COLUMNS: Column<WorkspaceSite>[] = [
  { key: "code", header: "Code", cell: (x) => <strong className={s.mono}>{x.code}</strong>, sort: (x) => x.code, width: "80px" },
  {
    key: "name",
    header: "Name",
    cell: (x) => (
      <>
        {x.name} {x.current && <Badge tone="accent">You are here</Badge>}
      </>
    ),
    sort: (x) => x.name,
    grow: true,
  },
  { key: "tz", header: "Time zone", cell: (x) => x.timezone, sort: (x) => x.timezone, width: "190px" },
  { key: "bins", header: "Bins", cell: (x) => x.locations.toLocaleString(), sort: (x) => x.locations, align: "right", width: "90px" },
  {
    key: "sequenced",
    header: "In walk order",
    cell: (x) => (x.locations ? <Progress done={x.sequenced} of={x.locations} label="in walk order" /> : <Faint>—</Faint>),
    sort: (x) => (x.locations ? x.sequenced / x.locations : 0),
    width: "180px",
  },
  {
    key: "active",
    header: "Status",
    cell: (x) => (
      <Badge tone={x.active ? "success" : "neutral"} dot>
        {x.active ? "Active" : "Inactive"}
      </Badge>
    ),
    width: "110px",
  },
];

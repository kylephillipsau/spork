import { useState } from "react";
import { Warehouse } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  Checkbox,
  DataTable,
  EmptyState,
  Fact,
  Facts,
  Page,
  PageHeader,
  Select,
  Skeleton,
  TextField,
  type Column,
} from "@ui/index";
import type { Organisation, PackageTypeRow, ReportedField, WorkspaceSite } from "@domain/types";
import { NETSUITE_ROLES } from "@app/items/ItemProperties";
import { Faint, Progress, shortDate } from "@app/common/cells";

import type { WorkspaceBench } from "./useWorkspace";
import s from "./settings.module.css";

/** The organisation and its warehouses, as the imports have described them. */
export function WorkspacePage({ bench }: { bench: WorkspaceBench }) {
  const st = bench.state;
  const ws = st.kind === "ready" ? st.workspace : null;
  const here = ws ? current(ws.sites) : null;

  return (
    <Page>
      <PageHeader title="Workspace" description="Your organisation and its warehouses." />

      {st.kind === "failed" && <Alert tone="danger">{st.message}</Alert>}

      <Card title="Organisation">
        {ws ? (
          <Facts columns={4}>
            <Fact label="Name">{ws.organisation.name}</Fact>
            <Fact label="Short name" mono>
              {ws.organisation.slug}
            </Fact>
            <Fact label="Status">
              <Badge tone={ws.organisation.active ? "success" : "neutral"} dot>
                {ws.organisation.active ? "Active" : "Inactive"}
              </Badge>
            </Fact>
            <Fact label="Created">{shortDate(ws.organisation.created_at)}</Fact>
          </Facts>
        ) : (
          <Skeleton width="50%" />
        )}
      </Card>

      {ws && here && <Packing site={here} organisation={ws.organisation} bench={bench} />}
      {ws && bench.boxes && <Boxes boxes={bench.boxes} bench={bench} />}
      {ws && bench.fields && <NetSuiteFields fields={bench.fields} bench={bench} />}

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
    </Page>
  );
}

const current = (sites: WorkspaceSite[]) => sites.find((x) => x.current) ?? null;

/**
 * What packing at the current warehouse needs the site to say (migrations 95
 * and 97): where packing happens, and whose stock it holds. Until both are
 * said, the pack bench cannot make a carton or take goods picked in NetSuite.
 */
function Packing({ site, organisation, bench }: { site: WorkspaceSite; organisation: Organisation; bench: WorkspaceBench }) {
  const [code, setCode] = useState(site.pack_location ?? "PACK");
  return (
    <Card
      title={`Packing at ${site.code}`}
      description="Where new cartons are made and where goods picked in NetSuite are put down, and who owns the stock here."
    >
      <div className={s.stack}>
        {bench.problem && (
          <Alert tone="danger" onDismiss={bench.dismiss}>
            {bench.problem}
          </Alert>
        )}
        <Facts columns={2}>
          <Fact label="Packs at" always>
            {site.pack_location ? <span className={s.mono}>{site.pack_location}</span> : <Faint>Not set</Faint>}
          </Fact>
          <Fact label="Stock belongs to" always>
            {site.owner ?? <Faint>Not set</Faint>}
          </Fact>
        </Facts>
        <form
          className={s.formRow}
          onSubmit={(e) => {
            e.preventDefault();
            if (code.trim()) void bench.setPackLocation(site.id, code.trim());
          }}
        >
          <div className={s.grow} style={{ maxWidth: 320 }}>
            <TextField
              label="Packing location"
              hint="A code this warehouse doesn't have yet becomes a new location."
              value={code}
              onChange={(e) => setCode(e.target.value)}
              disabled={bench.busy}
            />
          </div>
          <Button type="submit" disabled={bench.busy || !code.trim() || code.trim() === site.pack_location}>
            Pack here
          </Button>
        </form>
        {!site.owner && (
          <div className={s.actions}>
            <Button variant="primary" disabled={bench.busy} onClick={() => void bench.setOwner(site.id)}>
              The stock here belongs to {organisation.name}
            </Button>
          </div>
        )}
      </div>
    </Card>
  );
}

/**
 * The boxes this workspace packs into, and which the pack bench may suggest
 * (D196). Every box can still be chosen by hand; a shovel box is the kind the
 * suggestion should leave alone. Only a box with a fixed size can be
 * suggested, and a pallet or a skid never is: it carries cartons.
 */
function Boxes({ boxes, bench }: { boxes: PackageTypeRow[]; bench: WorkspaceBench }) {
  const own = boxes.filter((b) => b.tenant_owned);
  const fits = (b: PackageTypeRow) => b.dimensions_fixed && !["PAL", "SKI", "SKD"].includes(b.carrier_package_code ?? "");
  const columns: Column<PackageTypeRow>[] = [
    { key: "name", header: "Box", cell: (b) => <strong>{b.name}</strong>, sort: (b) => b.name },
    {
      key: "size",
      header: "Inside (L × W × H)",
      cell: (b) => (b.length_mm && b.width_mm && b.height_mm ? `${b.length_mm} × ${b.width_mm} × ${b.height_mm} mm` : <Faint>No fixed size</Faint>),
      sort: (b) => (b.length_mm ?? 0) * (b.width_mm ?? 0) * (b.height_mm ?? 0),
    },
    {
      key: "empty",
      header: "Empty",
      cell: (b) =>
        fits(b) ? (
          <Kilograms
            label={`What ${b.name} weighs empty`}
            grams={b.tare_weight_g}
            placeholder="Not weighed"
            disabled={bench.busy}
            save={(g) => void bench.boxEmptyWeight(b.id, g)}
          />
        ) : (
          <Faint>—</Faint>
        ),
      width: "150px",
    },
    {
      key: "weight",
      header: "Max weight",
      cell: (b) =>
        fits(b) ? (
          <Kilograms
            label={`Max weight of ${b.name}`}
            grams={b.max_payload_g}
            placeholder="No limit"
            disabled={bench.busy}
            save={(g) => void bench.boxWeight(b.id, g)}
          />
        ) : (
          <Faint>—</Faint>
        ),
      width: "150px",
    },
    {
      key: "suggest",
      header: "Suggested",
      cell: (b) =>
        fits(b) ? (
          <Checkbox
            label={<span className={s.srOnly}>Suggest {b.name}</span>}
            checked={b.suggested}
            disabled={bench.busy}
            onCheckedChange={(on) => void bench.suggest(b.id, on)}
          />
        ) : (
          <Faint>{b.dimensions_fixed ? "Carries cartons" : "No fixed size"}</Faint>
        ),
      width: "160px",
    },
  ];
  return (
    <Card
      title="Boxes"
      count={own.length}
      description="The boxes you pack into. The pack bench suggests ticked boxes, no heavier than their max weight, and weighs a box of goods as its goods and its empty weight; any box can still be chosen by hand."
      padded={false}
    >
      <DataTable aria-label="Boxes" columns={columns} rows={own} rowKey={(b) => b.id} empty={<EmptyState title="No boxes yet" description="Boxes arrive with the packaging presets import." />} />
    </Card>
  );
}

/** The units a weight or a size may be fixed in, as NetSuite's labels say them. */
const UNITS = { weight: ["g", "kg", "lb", "oz"], size: ["mm", "cm", "m", "in"] };

/**
 * NetSuite's fields as the item details carry them (D238), and what each is
 * read as: a weight or a size to compare with what Spork measured, a barcode
 * at a level, a pack count, something shown beside the code, a warning, or
 * only kept. Whatever is chosen, what NetSuite said is kept as it said it.
 */
function NetSuiteFields({ fields, bench }: { fields: ReportedField[]; bench: WorkspaceBench }) {
  const say = (f: ReportedField, change: Partial<Pick<ReportedField, "role" | "unit" | "unit_field" | "level">>) => {
    const next = { role: f.role, unit: f.unit, unit_field: f.unit_field, level: f.level, ...change };
    const measured = ["weight", "length", "width", "height"].includes(next.role);
    void bench.sayField(f, {
      role: next.role,
      unit: measured ? next.unit : null,
      unit_field: measured ? next.unit_field : null,
      level: next.role === "barcode" ? next.level : null,
    });
  };
  const others = (f: ReportedField) => fields.filter((o) => o.source === f.source && o.field !== f.field).map((o) => o.field);
  const columns: Column<ReportedField>[] = [
    {
      key: "field",
      header: "Field",
      cell: (f) => (
        <span>
          <strong>{f.field}</strong>
          {f.said && <Faint> · said here</Faint>}
        </span>
      ),
      sort: (f) => f.field,
    },
    { key: "example", header: "Example", cell: (f) => <Faint>{f.example}</Faint> },
    { key: "items", header: "Items", cell: (f) => f.items.toLocaleString(), align: "right", mono: true, width: "90px", sort: (f) => f.items },
    {
      key: "role",
      header: "Read as",
      cell: (f) => (
        <Select
          aria-label={`What ${f.field} is read as`}
          size="sm"
          value={f.role}
          disabled={bench.busy}
          onValueChange={(role) => say(f, { role })}
          options={NETSUITE_ROLES.map((r) => ({ value: r.role, label: r.label }))}
        />
      ),
      width: "220px",
    },
    {
      key: "detail",
      header: "Unit or level",
      cell: (f) => {
        if (f.role === "barcode") {
          return (
            <Select
              aria-label={`Which level ${f.field} is the barcode of`}
              size="sm"
              value={f.level ?? "unit"}
              disabled={bench.busy}
              onValueChange={(v) => say(f, { level: v === "unit" ? null : v })}
              options={[
                { value: "unit", label: "What NetSuite counts" },
                { value: "each", label: "A single one" },
                { value: "inner", label: "A pack" },
                { value: "carton", label: "A carton" },
              ]}
            />
          );
        }
        if (!["weight", "length", "width", "height"].includes(f.role)) return <Faint>—</Faint>;
        const units = f.role === "weight" ? UNITS.weight : UNITS.size;
        return (
          <Select
            aria-label={`${f.field}'s unit`}
            size="sm"
            value={f.unit_field ? `field:${f.unit_field}` : f.unit ? `unit:${f.unit}` : "none"}
            disabled={bench.busy}
            onValueChange={(v) =>
              say(f, v.startsWith("field:") ? { unit_field: v.slice(6), unit: null } : { unit: v.slice(5), unit_field: null })
            }
            options={[
              ...(f.unit || f.unit_field ? [] : [{ value: "none", label: "Not said" }]),
              ...units.map((u) => ({ value: `unit:${u}`, label: u })),
              ...others(f).map((o) => ({ value: `field:${o}`, label: `As ${o} says` })),
            ]}
          />
        );
      },
      width: "200px",
    },
  ];
  return (
    <Card
      title="NetSuite fields"
      count={fields.length}
      description="What the Spork Bridge sends of each item, as NetSuite said it, and what Spork reads each field as. What NetSuite said is always kept; this only decides how it is used and compared."
      padded={false}
    >
      <DataTable
        aria-label="NetSuite fields"
        columns={columns}
        rows={fields}
        rowKey={(f) => `${f.source}:${f.field}`}
        empty={<EmptyState title="No fields yet" description="They arrive with the Spork Bridge's item details." />}
      />
    </Card>
  );
}

/**
 * A weight of a box, in kilograms as a scale reads, saved on Enter or on
 * leaving the field: the most its goods may weigh (D199), or what it weighs
 * empty (D224). Empty says nothing.
 */
function Kilograms({
  label,
  grams,
  placeholder,
  disabled,
  save,
}: {
  label: string;
  grams: number | null;
  placeholder: string;
  disabled: boolean;
  save: (grams: number | null) => void;
}) {
  const said = grams === null ? "" : String(grams / 1000);
  const [typed, setTyped] = useState(said);
  const commit = () => {
    const t = typed.trim();
    if (t === said) return;
    const kg = Number(t);
    if (t !== "" && !(kg > 0)) {
      setTyped(said);
      return;
    }
    save(t === "" ? null : Math.round(kg * 1000));
  };
  return (
    <TextField
      aria-label={label}
      inputMode="decimal"
      placeholder={placeholder}
      trailing="kg"
      value={typed}
      disabled={disabled}
      onChange={(e) => setTyped(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
      }}
    />
  );
}

const COLUMNS: Column<WorkspaceSite>[] = [
  { key: "code", header: "Code", cell: (x) => <strong className={s.mono}>{x.code}</strong>, sort: (x) => x.code, width: "80px" },
  {
    key: "name",
    header: "Name",
    cell: (x) => (
      <>
        {x.name} {x.current && <Badge tone="accent">Current</Badge>}
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

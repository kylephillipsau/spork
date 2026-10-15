import { useState } from "react";
import { Plus, Truck } from "lucide-react";

import { Badge, Button, Card, Checkbox, DataTable, EmptyState, Select, TextField, type Column } from "@ui/index";
import type { BuildRule, CarrierRow, RuleKind, RuleSetRow, Uuid } from "@domain/types";
import { Faint, shortDate } from "@app/common/cells";

import type { WorkspaceBench } from "./useWorkspace";
import s from "./settings.module.css";

const NONE = "none";

/**
 * Workspace › Carriers (settled 9): who the workspace sends with, the rules
 * each one's pallets are built to (D250), and which is the usual one. A
 * pallet is started for the usual carrier unless the bench is told another,
 * and keeps the rules it was started under.
 */
export function CarriersCard({ carriers, rules, bench }: { carriers: CarrierRow[]; rules: RuleSetRow[]; bench: WorkspaceBench }) {
  const [name, setName] = useState("");
  const [serviceOf, setServiceOf] = useState<string | undefined>(undefined);
  const [service, setService] = useState("");
  const sets = [{ value: NONE, label: "None" }, ...rules.map((r) => ({ value: r.id, label: r.name }))];
  const own = carriers.filter((c) => c.own);
  const ruleSelect = (c: CarrierRow, which: "breakdown_rules_id" | "as_one_rules_id", label: string) =>
    c.own ? (
      <Select
        size="sm"
        aria-label={`${label} of ${c.name}`}
        value={c[which] ?? NONE}
        disabled={bench.busy}
        onValueChange={(v) => void bench.saveCarrier(c, { [which]: v === NONE ? null : v })}
        options={sets}
      />
    ) : (
      <Faint>{rules.find((r) => r.id === c[which])?.name ?? "None"}</Faint>
    );

  const columns: Column<CarrierRow>[] = [
    {
      key: "name",
      header: "Carrier",
      grow: true,
      cell: (c) => (
        <span className={s.carrierName}>
          <strong>{c.name}</strong>
          {c.usual && <Badge tone="accent">Usual</Badge>}
        </span>
      ),
      sort: (c) => c.name,
    },
    { key: "services", header: "Services", cell: (c) => (c.services.length ? c.services.map((x) => x.name).join(", ") : <Faint>None</Faint>) },
    { key: "breakdown", header: "Breakdown pallets", cell: (c) => ruleSelect(c, "breakdown_rules_id", "Breakdown pallets"), width: "200px" },
    { key: "as_one", header: "Pallets that go as one", cell: (c) => ruleSelect(c, "as_one_rules_id", "Pallets that go as one"), width: "200px" },
    {
      key: "usual",
      header: <span className={s.srOnly}>Usual</span>,
      cell: (c) =>
        c.usual ? null : (
          <Button size="sm" variant="ghost" disabled={bench.busy} onClick={() => void bench.setUsualCarrier(c.id)}>
            Make usual
          </Button>
        ),
      align: "right",
      width: "120px",
    },
  ];

  return (
    <Card
      title="Carriers"
      count={carriers.length}
      description="Who you send with, and the rules each one's pallets are built to. A pallet is started for the usual carrier unless the bench is told another, and keeps the rules it was started under. Booking stays in NetSuite."
    >
      <div className={s.stack}>
        <DataTable
          aria-label="Carriers"
          columns={columns}
          rows={carriers}
          rowKey={(c) => c.id}
          empty={<EmptyState icon={<Truck />} title="No carriers yet" description="Add the ones you send with." />}
        />
        <form
          className={s.formRow}
          onSubmit={(e) => {
            e.preventDefault();
            if (name.trim()) void bench.addCarrier(name.trim()).then((added) => added && setName(""));
          }}
        >
          <div className={s.grow}>
            <TextField label="Add a carrier" autoComplete="off" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <Button type="submit" icon={<Plus />} disabled={bench.busy || !name.trim()}>
            Add carrier
          </Button>
        </form>
        {own.length > 0 && (
          <form
            className={s.formRow}
            onSubmit={(e) => {
              e.preventDefault();
              const to = serviceOf ?? own[0]!.id;
              if (service.trim()) void bench.addService(to, service.trim()).then((added) => added && setService(""));
            }}
          >
            <div className={s.narrow}>
              <Select
                label="Service of"
                value={serviceOf ?? own[0]!.id}
                disabled={bench.busy}
                onValueChange={setServiceOf}
                options={own.map((c) => ({ value: c.id, label: c.name }))}
              />
            </div>
            <div className={s.grow}>
              <TextField label="Add a service" autoComplete="off" value={service} onChange={(e) => setService(e.target.value)} />
            </div>
            <Button type="submit" icon={<Plus />} disabled={bench.busy || !service.trim()}>
              Add service
            </Button>
          </form>
        )}
      </div>
    </Card>
  );
}

/** Each rule's name and what its figure is, as the set shows it. */
const RULES: Record<RuleKind, { name: string; unit?: { of: string; per: number }; before?: string; after?: string; words?: string }> = {
  height: { name: "Height", unit: { of: "m", per: 1000 } },
  weight: { name: "Weight", unit: { of: "kg", per: 1000 }, after: "with the pallet" },
  overhang_sides: { name: "Overhang, left and right", unit: { of: "mm", per: 1 }, after: "either side of the forks" },
  overhang_ends: { name: "Overhang, front and back", unit: { of: "mm", per: 1 }, after: "the front is where the forks go in" },
  labels: { name: "Labels", words: "Each parcel's label on a side facing out, hidden behind nothing" },
  support: { name: "Support", unit: { of: "%", per: 10 }, after: "of each box's base on something" },
  heavy: { name: "Heavy", before: "Mark a parcel heavy over", unit: { of: "kg", per: 1000 } },
  loose: { name: "Loose", before: "A parcel over", unit: { of: "kg", per: 1000 }, after: "goes on a pallet" },
  together: { name: "Together", words: "Each order's parcels next to one another" },
  columns: { name: "Columns", words: "Corners over corners where the boxes allow" },
  low_and_centred: { name: "Low and centred", words: "The heavy and weight-taking low, the weight over the middle" },
  flat_top: { name: "Flat on top", words: "As few heights on top as it can" },
};

const PREFER_ONLY: readonly RuleKind[] = ["low_and_centred", "flat_top"];
const WEIGHTS = [
  { value: "1", label: "Low" },
  { value: "2", label: "Medium" },
  { value: "3", label: "High" },
];

/** "7 must · 5 prefer". */
const counted = (rules: BuildRule[]) =>
  `${rules.filter((r) => r.strength === "must").length} must · ${rules.filter((r) => r.strength === "prefer").length} prefer`;

/**
 * Workspace › Build rules (D250): how a pallet must be built, a named set at
 * a time. Each rule is a must, a preference with a weight, or off. A carrier
 * points at a set; the bench reads the rules, never the carrier.
 */
export function BuildRulesCard({ rules, bench }: { rules: RuleSetRow[]; bench: WorkspaceBench }) {
  const [open, setOpen] = useState<Uuid | null>(rules[0]?.id ?? null);
  const [name, setName] = useState("");
  const [from, setFrom] = useState(NONE);
  const opened = rules.find((r) => r.id === open) ?? rules[0] ?? null;

  return (
    <Card
      title="Build rules"
      count={rules.length}
      description="How a pallet must be built. A carrier points at a set, and a pallet keeps the rules it was started under: changing a set changes the pallets started after. The bench checks heavy, loose and a pallet's weight now; the rest once the planner places cartons."
    >
      <div className={s.rulesGrid}>
        <div className={s.sets} role="group" aria-label="Sets of rules">
          {rules.map((r) => (
            <Button key={r.id} aria-pressed={opened?.id === r.id} onClick={() => setOpen(r.id)} className={s.set}>
              <span className={s.setName}>{r.name}</span>
              <span className={s.muted}>{r.used_by.length ? `For ${r.used_by.join(", ")}` : "No carrier points at it"}</span>
              <span className={s.muted}>{counted(r.rules)}</span>
            </Button>
          ))}
          <form
            className={s.stack}
            onSubmit={(e) => {
              e.preventDefault();
              if (!name.trim()) return;
              void bench.newRuleSet(name.trim(), from === NONE ? null : from).then((id) => {
                if (!id) return;
                setName("");
                setOpen(id);
              });
            }}
          >
            <TextField label="New set" autoComplete="off" placeholder="Order pallet" value={name} onChange={(e) => setName(e.target.value)} />
            <Select
              label="Start from"
              value={from}
              disabled={bench.busy}
              onValueChange={setFrom}
              options={[{ value: NONE, label: "The rules to start" }, ...rules.map((r) => ({ value: r.id, label: r.name }))]}
            />
            <Button type="submit" icon={<Plus />} disabled={bench.busy || !name.trim()}>
              Start set
            </Button>
          </form>
        </div>
        {opened ? (
          <RuleSetEditor key={`${opened.id}:${opened.version_id}`} set={opened} bench={bench} />
        ) : (
          <EmptyState title="No rules yet" description="Start a set, then point a carrier at it." />
        )}
      </div>
    </Card>
  );
}

function RuleSetEditor({ set, bench }: { set: RuleSetRow; bench: WorkspaceBench }) {
  const [name, setName] = useState(set.name);
  const [draft, setDraft] = useState<BuildRule[]>(set.rules);
  const changed = name.trim() !== set.name || JSON.stringify(draft) !== JSON.stringify(set.rules);
  const put = (kind: RuleKind, change: Partial<BuildRule>) => setDraft((d) => d.map((r) => (r.kind === kind ? { ...r, ...change } : r)));

  return (
    <form
      className={s.ruleSet}
      aria-label={set.name}
      onSubmit={(e) => {
        e.preventDefault();
        if (changed && name.trim()) void bench.saveRuleSet(set.id, name.trim(), draft);
      }}
    >
      <div className={s.formRow}>
        <div className={s.grow}>
          <TextField label="Name" autoComplete="off" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <span className={s.muted}>
          Saved {shortDate(set.saved_at)} by {set.saved_by}
        </span>
      </div>
      <ul className={s.rules} aria-label={`The rules of ${set.name}`}>
        {draft.map((r) => (
          <RuleRow key={r.kind} rule={r} put={(change) => put(r.kind, change)} busy={bench.busy} />
        ))}
      </ul>
      <div className={s.actions}>
        <Button
          disabled={bench.busy || !changed}
          onClick={() => {
            setName(set.name);
            setDraft(set.rules);
          }}
        >
          Discard
        </Button>
        <Button type="submit" disabled={bench.busy || !changed || !name.trim()}>
          Save rules
        </Button>
      </div>
    </form>
  );
}

/** One rule: its figure, must, prefer or off, and a preference's weight. */
function RuleRow({ rule, put, busy }: { rule: BuildRule; put: (change: Partial<BuildRule>) => void; busy: boolean }) {
  const says = RULES[rule.kind];
  const strengths = (PREFER_ONLY.includes(rule.kind) ? ["prefer", "off"] : ["must", "prefer", "off"]) as BuildRule["strength"][];
  return (
    <li className={s.ruleRow}>
      <span className={s.ruleName}>{says.name}</span>
      <span className={s.ruleSetting}>
        {says.words && <span className={s.muted}>{says.words}</span>}
        {says.before && <span className={s.muted}>{says.before}</span>}
        {says.unit && (
          <Amount
            label={says.name}
            amount={rule.amount}
            unit={says.unit}
            disabled={busy}
            save={(amount) => put({ amount })}
          />
        )}
        {says.after && <span className={s.muted}>{says.after}</span>}
        {rule.kind === "height" && (
          <Checkbox label="with the pallet" checked={rule.with_base ?? true} disabled={busy} onCheckedChange={(on) => put({ with_base: on })} />
        )}
        {rule.kind === "labels" && (
          <Checkbox
            label="A label on top of the top layer counts"
            checked={rule.top_counts ?? false}
            disabled={busy}
            onCheckedChange={(on) => put({ top_counts: on })}
          />
        )}
      </span>
      <span className={s.strength} role="group" aria-label={`${says.name}: must, prefer or off`}>
        {strengths.map((x) => (
          <Button
            key={x}
            size="sm"
            aria-pressed={rule.strength === x}
            disabled={busy}
            onClick={() => put({ strength: x, weight: x === "prefer" ? (rule.weight ?? 2) : null })}
          >
            {x === "must" ? "Must" : x === "prefer" ? "Prefer" : "Off"}
          </Button>
        ))}
      </span>
      <span className={s.ruleWeight}>
        {rule.strength === "prefer" && (
          <Select
            size="sm"
            aria-label={`How much ${says.name} counts`}
            value={String(rule.weight ?? 2)}
            disabled={busy}
            onValueChange={(v) => put({ weight: Number(v) })}
            options={WEIGHTS}
          />
        )}
      </span>
    </li>
  );
}

/** A rule's figure in the unit it is said in, kept in the unit it is stored in. */
function Amount({
  label,
  amount,
  unit,
  disabled,
  save,
}: {
  label: string;
  amount: number | null;
  unit: { of: string; per: number };
  disabled: boolean;
  save: (amount: number) => void;
}) {
  const said = amount === null ? "" : String(amount / unit.per);
  const [typed, setTyped] = useState(said);
  const commit = () => {
    const n = Number(typed.trim().replace(",", "."));
    if (typed.trim() === "" || !(n >= 0)) setTyped(said);
    else if (typed.trim() !== said) save(Math.round(n * unit.per));
  };
  return (
    <span className={s.amount}>
      <TextField
        aria-label={`${label}, ${unit.of}`}
        inputMode="decimal"
        trailing={unit.of}
        value={typed}
        disabled={disabled}
        onChange={(e) => setTyped(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          // Kept in the draft, not saved: Save rules saves the set.
          if (e.key === "Enter") {
            e.preventDefault();
            commit();
          }
        }}
      />
    </span>
  );
}

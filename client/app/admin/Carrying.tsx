import { useState } from "react";

import { Button, Card, Checkbox, Fact, Facts, Select, TextField } from "@ui/index";
import type { BoxBoard, Carries, Conditions, PackageTypeRow } from "@domain/types";
import { Faint } from "@app/common/cells";
import { kgLimit } from "@app/common/format";

import { Kilograms } from "@app/common/Kilograms";
import type { WorkspaceBench } from "./useWorkspace";
import s from "./settings.module.css";

/** "≈ 33 kg" estimated, "40 kg" from a test (D249). */
export function wallsWords(c: Carries): string {
  return c.walls_estimated ? `≈ ${kgLimit(c.walls_g)}` : kgLimit(c.walls_g);
}

/** "10 kg", or "Nothing" for a lid that carries none. */
export function lidWords(c: Carries): string {
  return c.lid_g === 0 ? "Nothing" : kgLimit(c.lid_g);
}

const boardWords = (b: BoxBoard) => `${b.walls === 2 ? "Double" : "Single"} wall, ${b.ect} ECT`;

/**
 * What a full box carries (D249), in its opened card: on its walls, estimated
 * from its size and board unless a test on the floor says, and on its lid
 * when hollow, 10 kg to start. Its heights carry as it does.
 */
export function BoxCarries({ box, bench }: { box: PackageTypeRow; bench: WorkspaceBench }) {
  const c = box.carries;
  const [ect, setEct] = useState(String(c?.board.ect ?? 32));
  if (!c) return null;
  const said = { board: c.board_said ? c.board : null, wall_load_g: c.walls_estimated ? null : c.walls_g, lid_load_g: c.lid_said ? c.lid_g : null };
  const save = (change: Partial<typeof said>) => void bench.setCarries(box.id, { ...said, ...change });

  return (
    <div className={s.stack}>
      <Facts columns={3}>
        <Fact label="On its walls">
          <span>
            {wallsWords(c)}{" "}
            <Faint>{c.walls_estimated ? `estimated, of ${kgLimit(c.crush_g)} it crushes at empty` : "from a test"}</Faint>
          </span>
        </Fact>
        <Fact label="On its lid, when hollow">
          <span>
            {lidWords(c)} {!c.lid_said && <Faint>to start</Faint>}
          </span>
        </Fact>
        <Fact label="Board">
          <span>
            {boardWords(c.board)} {!c.board_said && <Faint>assumed</Faint>}
          </span>
        </Fact>
      </Facts>
      <div className={s.formRow}>
        <div className={s.narrow}>
          <Select
            label="Board"
            value={c.board_said ? String(c.board.walls) : "unsaid"}
            disabled={bench.busy}
            onValueChange={(v) => save({ board: v === "unsaid" ? null : { walls: Number(v), ect: c.board.ect } })}
            options={[
              { value: "unsaid", label: "Not said" },
              { value: "1", label: "Single wall" },
              { value: "2", label: "Double wall" },
            ]}
          />
        </div>
        <div className={s.narrow}>
          <TextField
            label="Edge crush, ECT"
            inputMode="numeric"
            value={ect}
            disabled={bench.busy || !c.board_said}
            onChange={(e) => setEct(e.target.value)}
            onBlur={() => {
              const n = Number(ect);
              if (n >= 10 && n <= 200 && n !== c.board.ect) save({ board: { walls: c.board.walls, ect: n } });
              else setEct(String(c.board.ect));
            }}
          />
        </div>
        <div className={s.narrow}>
          <Kilograms
            label={`What the walls of ${box.name} carried in a test`}
            grams={said.wall_load_g}
            placeholder="Estimated"
            disabled={bench.busy}
            save={(g) => save({ wall_load_g: g })}
          />
        </div>
        <div className={s.narrow}>
          <Kilograms
            label={`What the lid of ${box.name} carries`}
            grams={said.lid_load_g}
            placeholder="10, to start"
            disabled={bench.busy}
            nothing
            save={(g) => save({ lid_load_g: g })}
          />
        </div>
      </div>
      <p className={s.muted}>
        The walls from a test, and the lid when it is hollow, in kilograms. Say the board as stamped on the box and the estimate follows it.
      </p>
    </div>
  );
}

const HUMIDITY = [50, 60, 70, 80, 90] as const;
const DAYS: { value: Conditions["days_under_load"]; label: string }[] = [
  { value: 1, label: "A day or two" },
  { value: 10, label: "Ten days" },
  { value: 30, label: "A month" },
  { value: 90, label: "Three months" },
  { value: 180, label: "Six months" },
];
const STACKING: { value: Conditions["stacking"]; label: string }[] = [
  { value: "columns", label: "In columns, corners over corners" },
  { value: "misaligned", label: "In columns, not lined up" },
  { value: "interlocked", label: "Interlocked, each layer turned" },
];
const HANDLING: { value: Conditions["handling"]; label: string }[] = [
  { value: "careful", label: "Careful" },
  { value: "some_rough", label: "Some rough" },
  { value: "rough", label: "Rough" },
];

/** "about an eighth", from what of a box's strength the conditions leave. */
export function shareWords(share: number): string {
  const n = Math.round(1 / share);
  const words: Record<number, string> = { 2: "a half", 3: "a third", 4: "a quarter", 5: "a fifth", 6: "a sixth", 7: "a seventh", 8: "an eighth", 9: "a ninth", 10: "a tenth" };
  return words[n] ?? `1/${n}`;
}

/**
 * The warehouse's conditions (D249): what every box's wall estimate allows for,
 * one setting for the workspace. The multipliers are the Fibre Box
 * Association's, and the road factor what the walls must carry over the load.
 */
export function ConditionsCard({ conditions, share, bench }: { conditions: Conditions; share: number; bench: WorkspaceBench }) {
  const [c, setC] = useState(conditions);
  const [road, setRoad] = useState(String(conditions.road_factor));
  const factor = Number(road);
  const next = { ...c, road_factor: factor };
  const changed = JSON.stringify(next) !== JSON.stringify(conditions);
  return (
    <Card
      title="Conditions in this warehouse"
      description={`What the estimate of each box's walls allows for, one setting for every box and carrier. Walls carry about ${shareWords(share)} of what a box crushes at empty.`}
    >
      <form
        className={s.formRow}
        onSubmit={(e) => {
          e.preventDefault();
          if (changed && factor >= 1 && factor <= 10) void bench.setConditions(next);
        }}
      >
        <div className={s.narrow}>
          <Select
            label="Humidity, most days"
            value={String(c.humidity_pct)}
            disabled={bench.busy}
            onValueChange={(v) => setC({ ...c, humidity_pct: Number(v) as Conditions["humidity_pct"] })}
            options={HUMIDITY.map((h) => ({ value: String(h), label: `Up to ${h}%` }))}
          />
        </div>
        <div className={s.narrow}>
          <Select
            label="A load stands"
            value={String(c.days_under_load)}
            disabled={bench.busy}
            onValueChange={(v) => setC({ ...c, days_under_load: Number(v) as Conditions["days_under_load"] })}
            options={DAYS.map((d) => ({ value: String(d.value), label: d.label }))}
          />
        </div>
        <div className={s.grow}>
          <Select
            label="Stacked"
            value={c.stacking}
            disabled={bench.busy}
            onValueChange={(v) => setC({ ...c, stacking: v as Conditions["stacking"] })}
            options={STACKING}
          />
        </div>
        <div className={s.narrow}>
          <Select
            label="Handling"
            value={c.handling}
            disabled={bench.busy}
            onValueChange={(v) => setC({ ...c, handling: v as Conditions["handling"] })}
            options={HANDLING}
          />
        </div>
        <div className={s.narrow}>
          <TextField label="On the road" inputMode="numeric" leading="×" value={road} disabled={bench.busy} onChange={(e) => setRoad(e.target.value)} />
        </div>
        <Checkbox label="Pallet decks have gaps" checked={c.deck_gaps} disabled={bench.busy} onCheckedChange={(on) => setC({ ...c, deck_gaps: on })} />
        <Button type="submit" disabled={bench.busy || !changed || !(factor >= 1 && factor <= 10)}>
          Save
        </Button>
      </form>
    </Card>
  );
}

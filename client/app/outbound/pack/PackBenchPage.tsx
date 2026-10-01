import { useState } from "react";
import { Boxes, FileText, PackageOpen, Plus, Trash2 } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  DataTable,
  EmptyState,
  Fact,
  Facts,
  Link,
  Page,
  PageHeader,
  Select,
  Skeleton,
  Spacer,
  TextField,
  Toolbar,
  type Column,
} from "@ui/index";
import type { BenchLine, BenchScreen, CartonSummary, ExpectedWeight, OwnCarton, PackedRow, StatedSize } from "@domain/types";
import { agreement, provenance } from "@app/measurement/baseline";
import { href } from "@app/routing/location";
import { Faint } from "@app/common/cells";
import { kg } from "@app/common/format";
import { ItemCode, ItemDrawer } from "@app/items/ItemProperties";

import type { PackBench } from "./usePackBench";
import s from "./pack-bench.module.css";

/** What the packer has chosen on one line: the bin, and how many. */
interface Draft {
  cell: string;
  qty: string;
}

/**
 * The pack bench for one fulfilment (D171): what is left to pack on the left,
 * the cartons on the right. Add puts units from a bin into the open carton;
 * Seal closes it. One primary action at a time: Start carton when none is
 * open, otherwise Seal on the open one.
 */
export function PackBenchPage({ bench }: { bench: PackBench }) {
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  // The item whose size, weight and photos are open beside the bench (D174).
  const [looking, setLooking] = useState<string | null>(null);
  const st = bench.status;

  if (st.kind !== "ready") {
    return (
      <Page>
        <PageHeader title="Pack order" />
        {st.kind === "failed" ? (
          <Alert tone="danger">{st.message}</Alert>
        ) : (
          <Card>
            <Skeleton width="50%" />
          </Card>
        )}
      </Page>
    );
  }

  const screen = st.screen;
  const left = screen.lines.reduce((t, l) => t + Math.max(0, l.remaining), 0);
  const draft = (l: BenchLine): Draft => drafts[l.line_id] ?? { cell: l.cells[0]?.stock_id ?? "", qty: String(l.remaining) };
  const change = (l: BenchLine, next: Partial<Draft>) => setDrafts((d) => ({ ...d, [l.line_id]: { ...draft(l), ...next } }));
  const canAdd = (l: BenchLine) => l.remaining > 0 && l.cells.length > 0 && !!bench.openCarton && !bench.busy;
  // Picked elsewhere and not yet here: the goods are handed over rather than
  // taken from a bin (D172). Into the open carton, or else to staging.
  // Where the picking happened, when it was not here: one document for the
  // whole fulfilment, so it is said once rather than on every line.
  const elsewhere = screen.lines.find((l) => l.elsewhere)?.elsewhere ?? null;
  const handing = (l: BenchLine) => !!l.elsewhere && l.cells.length === 0 && l.remaining > 0;
  const canHand = (l: BenchLine) => handing(l) && (!!bench.openCarton || !!screen.staging_id) && !bench.busy;
  const hand = (l: BenchLine) => {
    const n = Number.parseInt(draft(l).qty, 10);
    if (canHand(l) && n > 0) void bench.handOver({ line: l.line_id, quantity: n });
  };
  // Whole cartons of a line, shipped as they came (migration 98): from the bin
  // chosen when this site holds the stock, else handed over.
  const wholeCartons = (l: BenchLine) => (l.own_carton ? Math.floor(Math.max(0, l.remaining) / l.own_carton.units) : 0);
  const canShipOwn = (l: BenchLine) =>
    wholeCartons(l) > 0 && !!screen.dock_id && !bench.busy && (l.cells.length > 0 || !!l.elsewhere);
  const shipOwn = (l: BenchLine) => {
    const own = l.own_carton;
    if (!own || !canShipOwn(l)) return;
    const stock = l.cells.length > 0 ? draft(l).cell : undefined;
    void bench.shipOwnCartons({
      line: l.line_id,
      config: own.item_packing_config_id,
      units: own.units,
      cartons: wholeCartons(l),
      ...(stock ? { stock } : {}),
    });
  };
  const add = (l: BenchLine) => {
    const d = draft(l);
    const n = Number.parseInt(d.qty, 10);
    if (canAdd(l) && n > 0) void bench.addToCarton({ line: l.line_id, stock: d.cell, quantity: n });
  };

  const lineColumns: Column<BenchLine>[] = [
    {
      key: "item",
      header: "Item",
      cell: (l) => (
        <span className={l.remaining === 0 ? s.done : undefined}>
          <ItemCode code={l.item_code} onOpen={() => setLooking(l.item_id)} />
          {/* An item made from a code alone has the code as its description;
              saying it twice is noise. */}
          {l.description && l.description !== l.item_code && <span className={s.desc}>{l.description}</span>}
        </span>
      ),
      grow: true,
    },
    {
      key: "from",
      header: "From",
      cell: (l) =>
        l.cells.length === 0 ? (
          l.elsewhere ? (
            // Where it was picked is said once, above the table; the line says
            // only how much of it is here.
            <Faint>
              {l.elsewhere.handed} of {l.elsewhere.reported} handed over
            </Faint>
          ) : (
            <Faint>No stock at this site</Faint>
          )
        ) : (
          <Select
            aria-label={`Pick ${l.item_code} from`}
            value={draft(l).cell}
            onValueChange={(cell) => change(l, { cell })}
            size="sm"
            options={l.cells.map((c) => ({
              value: c.stock_id,
              label: `${c.location} · ${c.available} free${c.lot ? ` · lot ${c.lot}` : ""}`,
            }))}
          />
        ),
      width: "200px",
    },
    { key: "left", header: "Left", cell: (l) => (l.remaining > 0 ? <strong>{l.remaining}</strong> : <Faint>0</Faint>), align: "right", width: "64px" },
    {
      key: "add",
      header: "Add",
      cell: (l) => (
        <div className={s.actions}>
          {addForm(l)}
          {l.own_carton && wholeCartons(l) > 0 && (
            <Button
              size="sm"
              icon={<Boxes />}
              disabled={!canShipOwn(l)}
              onClick={() => shipOwn(l)}
              title={describeOwn(l.own_carton)}
            >
              {shipLabel(wholeCartons(l), l.own_carton.units)}
            </Button>
          )}
        </div>
      ),
      align: "right",
      width: "190px",
    },
  ];

  function addForm(l: BenchLine) {
    return l.cells.length > 0 && l.remaining > 0 ? (
          <form
            className={s.add}
            onSubmit={(e) => {
              e.preventDefault();
              add(l);
            }}
          >
            <TextField
              aria-label={`Quantity of ${l.item_code}`}
              inputMode="numeric"
              className={s.qty}
              value={draft(l).qty}
              onChange={(e) => change(l, { qty: e.target.value })}
              disabled={!canAdd(l)}
            />
            <Button type="submit" size="sm" icon={<Plus />} disabled={!canAdd(l)}>
              Add
            </Button>
          </form>
        ) : handing(l) ? (
          <form
            className={s.add}
            onSubmit={(e) => {
              e.preventDefault();
              hand(l);
            }}
          >
            <TextField
              aria-label={`Quantity of ${l.item_code} handed over`}
              inputMode="numeric"
              className={s.qty}
              value={draft(l).qty}
              onChange={(e) => change(l, { qty: e.target.value })}
              disabled={!canHand(l)}
            />
            <Button type="submit" size="sm" icon={<Plus />} disabled={!canHand(l)}>
              {bench.openCarton ? "Into carton" : "To staging"}
            </Button>
          </form>
        ) : null;
  }

  return (
    <Page>
      <PageHeader
        title={
          <>
            Pack <span className={s.ref}>{screen.reference}</span>
          </>
        }
        description={`${screen.customer} · Order ${screen.order_reference}`}
        actions={<Badge tone={left ? "accent" : "success"}>{left ? `${left} ${left === 1 ? "unit" : "units"} to pack` : "Everything packed"}</Badge>}
      />

      {bench.problem && (
        <Alert tone="danger" onDismiss={bench.dismiss}>
          {bench.problem}
        </Alert>
      )}
      {/* Said before somebody tries, rather than as the refusal after. */}
      {screen.unready && (
        <Alert tone="warning">
          {screen.unready} <Link href="/workspace">Open Workspace</Link>
        </Alert>
      )}

      <div className={s.split}>
        <Card
          title="To pack"
          description={[
            elsewhere?.provenance,
            bench.openCarton ? "Items are added to the open carton." : "Start a carton to add items.",
          ]
            .filter(Boolean)
            .join(". ")}
          padded={false}
        >
          <DataTable
            aria-label="Lines to pack"
            columns={lineColumns}
            rows={screen.lines}
            rowKey={(l) => l.line_id}
            empty={<EmptyState title="Nothing committed" />}
          />
        </Card>

        <div className={s.cartons}>
          <NewCarton screen={screen} bench={bench} />
          {screen.cartons.map((c) => (
            <Carton key={c.id} carton={c} bench={bench} look={setLooking} />
          ))}
        </div>
      </div>
      <ItemDrawer itemId={looking} onClose={() => setLooking(null)} />
    </Page>
  );
}

function NewCarton({ screen, bench }: { screen: BenchScreen; bench: PackBench }) {
  const [preset, setPreset] = useState(screen.presets[0]?.id ?? "");
  return (
    <Card>
      <form
        className={s.newCarton}
        onSubmit={(e) => {
          e.preventDefault();
          if (preset) void bench.startCarton(preset);
        }}
      >
        <div className={s.preset}>
          <Select
            label="New carton"
            value={preset || undefined}
            onValueChange={setPreset}
            options={screen.presets.map((p) => ({ value: p.id, label: p.name }))}
            placeholder="No presets"
          />
        </div>
        <Button
          type="submit"
          variant={bench.openCarton ? "secondary" : "primary"}
          icon={<PackageOpen />}
          disabled={bench.busy || !preset || !screen.dock_id}
        >
          Start carton
        </Button>
      </form>
    </Card>
  );
}

function Carton({ carton, bench, look }: { carton: CartonSummary; bench: PackBench; look: (itemId: string) => void }) {
  const stated = carton.stated_size?.height_mm ?? null;
  const [weight, setWeight] = useState(carton.gross_weight_g === null ? "" : (carton.gross_weight_g / 1000).toFixed(3));
  const [height, setHeight] = useState(String(carton.height_mm ?? stated ?? ""));
  const cut = height.trim() !== "" && height.trim() !== String(stated ?? "");
  const open = carton.id === bench.openCarton;
  const footprint = carton.stated_size ? `${carton.stated_size.length_mm} × ${carton.stated_size.width_mm} mm` : null;

  const contentColumns: Column<PackedRow>[] = [
    { key: "item", header: "Item", cell: (r) => <ItemCode code={r.item_code} onOpen={() => look(r.item_id)} />, grow: true },
    { key: "lot", header: "Lot", cell: (r) => r.lot_code ?? <Faint>—</Faint>, mono: true, width: "110px" },
    { key: "units", header: "Units", cell: (r) => r.quantity, align: "right", width: "70px" },
  ];
  if (!carton.sealed) {
    contentColumns.push({
      key: "out",
      header: "",
      cell: (r) => (
        <Button size="sm" variant="ghost" disabled={bench.busy} onClick={() => void bench.takeOut({ picks: r.picks, quantity: r.quantity })}>
          Take out
        </Button>
      ),
      align: "right",
      width: "100px",
    });
  }

  return (
    <Card
      title={
        <span className={s.cartonTitle}>
          Carton {carton.sequence}
          {carton.package_type && <Badge>{carton.package_type}</Badge>}
          {carton.own_carton_of && <Badge>{carton.own_carton_of} carton</Badge>}
          {carton.sealed ? (
            <Badge tone="success" dot>
              Sealed
            </Badge>
          ) : open ? (
            <Badge tone="accent" dot>
              Open
            </Badge>
          ) : null}
        </span>
      }
      actions={
        carton.sealed ? (
          <Link variant="plain" href={href(`/print/packing-list/${carton.id}`)} className={s.print}>
            <FileText aria-hidden /> Packing list
          </Link>
        ) : undefined
      }
      padded={false}
    >
      <DataTable
        aria-label={`Carton ${carton.sequence} contents`}
        columns={contentColumns}
        rows={carton.contents}
        rowKey={(r) => `${r.item_code}-${r.lot_code ?? ""}`}
        empty={<p className={s.empty}>Empty.</p>}
      />

      {carton.sealed ? (
        <div className={s.body}>
          <Facts columns={4}>
            <Fact label="Gross">
              <span className={s.big}>{kg(carton.gross_weight_g)}</span>
            </Fact>
            {carton.expected && <ExpectedFact expected={carton.expected} />}
            {carton.listed_weight_g !== null && (
              <Fact label="Listed">
                <span>
                  {kg(carton.listed_weight_g)}
                  <span className={s.note}>by the record, not weighed</span>
                </span>
              </Fact>
            )}
            <Fact label="Height">{carton.height_mm !== null ? `${carton.height_mm} mm` : null}</Fact>
            {carton.own_carton_of && carton.stated_size ? (
              <Fact label="Size">{sized(carton.stated_size)}</Fact>
            ) : (
              <Fact label="Footprint">{footprint}</Fact>
            )}
          </Facts>
        </div>
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void bench.measure({
              carton: carton.id,
              ...(weight.trim() ? { weightKg: weight.trim() } : {}),
              ...(cut ? { heightMm: height.trim() } : {}),
            });
          }}
        >
          <div className={s.body}>
            <div className={s.measure}>
              <div className={s.measureField}>
                <TextField label="Weight" inputMode="decimal" trailing="kg" value={weight} onChange={(e) => setWeight(e.target.value)} />
              </div>
              <div className={s.measureField}>
                <TextField label="Height" inputMode="numeric" trailing="mm" value={height} onChange={(e) => setHeight(e.target.value)} />
              </div>
            </div>
            <Facts>
              {carton.expected && <ExpectedFact expected={carton.expected} />}
              <Fact label="Footprint">{footprint}</Fact>
            </Facts>
          </div>
          <Toolbar placement="bottom">
            {carton.contents.length === 0 && (
              <Button variant="ghost" icon={<Trash2 />} disabled={bench.busy} onClick={() => void bench.discard(carton.id)}>
                Discard
              </Button>
            )}
            <Spacer />
            <Button type="submit" disabled={bench.busy}>
              Record
            </Button>
            <Button variant={open ? "primary" : "secondary"} disabled={bench.busy} onClick={() => void bench.seal(carton.id)}>
              Seal
            </Button>
          </Toolbar>
        </form>
      )}
    </Card>
  );
}

/** "2 own cartons of 6": what the press will do, in words short enough for the column. */
function shipLabel(cartons: number, units: number): string {
  return `${cartons} own carton${cartons === 1 ? "" : "s"} of ${units}`;
}

/** What one of its cartons measures and weighs by the record, and whose figures. */
function describeOwn(own: OwnCarton): string {
  const parts = [own.size ? sized(own.size) : "size not recorded"];
  if (own.listed_weight_g !== null) parts.push(`${kg(own.listed_weight_g)} listed`);
  if (own.source === "style" && own.style_code) parts.push(`the ${own.style_code} family's`);
  return parts.join(" · ");
}

function sized(size: StatedSize): string {
  return `${size.length_mm} × ${size.width_mm} × ${size.height_mm} mm`;
}

/** What this carton has weighed before, and on what basis. */
function ExpectedFact({ expected }: { expected: ExpectedWeight }) {
  const gap = agreement(expected);
  return (
    <Fact label="Expected">
      <span>
        {kg(expected.grams)}
        <span className={s.note}>
          {provenance(expected)}
          {gap ? ` · ${gap}` : ""}
        </span>
      </span>
    </Fact>
  );
}

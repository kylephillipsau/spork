import { useState } from "react";
import { FileText, PackageOpen, Plus, Trash2 } from "lucide-react";

import { Badge, Button, Card, EmptyState, Link, PageHeader, Select, Skeleton, TextField, cx } from "@ui/index";
import { grams } from "@design/format";
import type { BenchLine, BenchScreen, CartonSummary, ExpectedWeight } from "@domain/types";
import { agreement, provenance } from "@app/measurement/baseline";
import { href } from "@app/routing/location";
import { Faint } from "@app/common/cells";
import { Alert } from "@app/admin/Alert";

import type { PackBench } from "./usePackBench";
import s from "./pack-bench.module.css";

const kg = (g: number | null) => (g === null ? "—" : `${grams(g)} kg`);

/**
 * The pack bench for one fulfilment (D171): what is left to pack on the left,
 * the cartons on the right. Add puts units from a bin into the open carton;
 * Seal closes it. One primary action at a time: Start a carton when none is
 * open, otherwise Seal on the open one.
 */
export function PackBenchPage({ bench }: { bench: PackBench }) {
  const st = bench.status;
  if (st.kind === "loading") {
    return (
      <div className={s.page}>
        <PageHeader title="Pack order" />
        <Card>
          <Skeleton width="50%" />
        </Card>
      </div>
    );
  }
  if (st.kind === "failed") {
    return (
      <div className={s.page}>
        <PageHeader title="Pack order" />
        <Alert tone="danger">{st.message}</Alert>
      </div>
    );
  }

  const screen = st.screen;
  const left = screen.lines.reduce((t, l) => t + Math.max(0, l.remaining), 0);

  return (
    <div className={s.page}>
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

      <div className={s.split}>
        <Card title="To pack" description={bench.openCarton ? "Add goes into the open carton." : "Start a carton to add to it."} padded={false}>
          {screen.lines.length === 0 ? (
            <EmptyState title="Nothing committed" />
          ) : (
            <table className={s.lines}>
              <thead>
                <tr>
                  <th>Item</th>
                  <th>From</th>
                  <th className={s.num}>Left</th>
                  <th className={s.num}>Add</th>
                </tr>
              </thead>
              <tbody>
                {screen.lines.map((line) => (
                  <LineRow key={line.line_id} line={line} bench={bench} />
                ))}
              </tbody>
            </table>
          )}
        </Card>

        <div className={s.cartons}>
          <NewCarton screen={screen} bench={bench} />
          {screen.cartons.map((c) => (
            <Carton key={c.id} carton={c} bench={bench} />
          ))}
        </div>
      </div>
    </div>
  );
}

function LineRow({ line, bench }: { line: BenchLine; bench: PackBench }) {
  const [cell, setCell] = useState(line.cells[0]?.stock_id ?? "");
  const [qty, setQty] = useState(String(line.remaining));
  const can = line.remaining > 0 && line.cells.length > 0 && !!bench.openCarton && !bench.busy;
  const add = () => {
    const n = Number.parseInt(qty, 10);
    if (can && n > 0) void bench.addToCarton({ line: line.line_id, stock: cell, quantity: n });
  };

  return (
    <tr className={line.remaining === 0 ? s.done : undefined}>
      <td>
        <div className={s.code}>{line.item_code}</div>
        {line.description && <div className={s.desc}>{line.description}</div>}
      </td>
      <td className={s.from}>
        {line.cells.length === 0 ? (
          <Faint>No stock at this site</Faint>
        ) : (
          <Select
            aria-label={`Pick ${line.item_code} from`}
            value={cell}
            onValueChange={setCell}
            size="sm"
            options={line.cells.map((c) => ({
              value: c.stock_id,
              label: `${c.location} · ${c.available} free${c.lot ? ` · lot ${c.lot}` : ""}`,
            }))}
          />
        )}
      </td>
      <td className={s.num}>{line.remaining > 0 ? <strong>{line.remaining}</strong> : <Faint>0</Faint>}</td>
      <td className={s.num}>
        {line.cells.length > 0 && line.remaining > 0 && (
          <form
            className={s.add}
            onSubmit={(e) => {
              e.preventDefault();
              add();
            }}
          >
            <TextField
              aria-label={`Quantity of ${line.item_code}`}
              inputMode="numeric"
              className={s.qty}
              value={qty}
              onChange={(e) => setQty(e.target.value)}
              disabled={!can}
            />
            <Button type="submit" size="sm" icon={<Plus />} disabled={!can}>
              Add
            </Button>
          </form>
        )}
      </td>
    </tr>
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
        <Button type="submit" variant={bench.openCarton ? "secondary" : "primary"} icon={<PackageOpen />} disabled={bench.busy || !preset}>
          Start carton
        </Button>
      </form>
    </Card>
  );
}

function Carton({ carton, bench }: { carton: CartonSummary; bench: PackBench }) {
  const stated = carton.stated_size?.height_mm ?? null;
  const [weight, setWeight] = useState(carton.gross_weight_g === null ? "" : grams(carton.gross_weight_g));
  const [height, setHeight] = useState(String(carton.height_mm ?? stated ?? ""));
  const cut = height.trim() !== "" && height.trim() !== String(stated ?? "");
  const open = carton.id === bench.openCarton;

  return (
    <Card
      title={
        <span className={s.cartonTitle}>
          Carton {carton.sequence}
          {carton.package_type && <Badge>{carton.package_type}</Badge>}
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
      {carton.contents.length === 0 ? (
        <p className={s.empty}>Empty.</p>
      ) : (
        <table className={s.contents}>
          <thead>
            <tr>
              <th>Item</th>
              <th>Lot</th>
              <th className={s.num}>Units</th>
              {!carton.sealed && <th />}
            </tr>
          </thead>
          <tbody>
            {carton.contents.map((row) => (
              <tr key={`${row.item_code}-${row.lot_code ?? ""}`}>
                <td className={s.code}>{row.item_code}</td>
                <td className={s.code}>{row.lot_code ?? <Faint>—</Faint>}</td>
                <td className={s.num}>{row.quantity}</td>
                {!carton.sealed && (
                  <td className={s.num}>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={bench.busy}
                      onClick={() => void bench.takeOut({ picks: row.picks, quantity: row.quantity })}
                    >
                      Take out
                    </Button>
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {carton.sealed ? (
        <div className={s.figures}>
          <Figure label="Gross" value={kg(carton.gross_weight_g)} big />
          {carton.expected && <Expected expected={carton.expected} />}
          {carton.height_mm !== null && <Figure label="Height" value={`${carton.height_mm} mm`} />}
          {carton.stated_size && (
            <Figure label="Footprint" value={`${carton.stated_size.length_mm} × ${carton.stated_size.width_mm} mm`} />
          )}
        </div>
      ) : (
        <form
          className={s.measure}
          onSubmit={(e) => {
            e.preventDefault();
            void bench.measure({
              carton: carton.id,
              ...(weight.trim() ? { weightKg: weight.trim() } : {}),
              ...(cut ? { heightMm: height.trim() } : {}),
            });
          }}
        >
          <div className={s.measureFields}>
            <div className={s.measureField}>
              <TextField label="Weight" inputMode="decimal" trailing="kg" value={weight} onChange={(e) => setWeight(e.target.value)} />
            </div>
            <div className={s.measureField}>
              <TextField label="Height" inputMode="numeric" trailing="mm" value={height} onChange={(e) => setHeight(e.target.value)} />
            </div>
            {carton.expected && <Expected expected={carton.expected} />}
            {carton.stated_size && (
              <Figure label="Footprint" value={`${carton.stated_size.length_mm} × ${carton.stated_size.width_mm} mm`} />
            )}
          </div>
          <div className={s.cartonActions}>
            {carton.contents.length === 0 && (
              <Button variant="ghost" icon={<Trash2 />} disabled={bench.busy} onClick={() => void bench.discard(carton.id)}>
                Discard
              </Button>
            )}
            <span className={s.spacer} />
            <Button type="submit" disabled={bench.busy}>
              Record
            </Button>
            <Button variant={open ? "primary" : "secondary"} disabled={bench.busy} onClick={() => void bench.seal(carton.id)}>
              Seal
            </Button>
          </div>
        </form>
      )}
    </Card>
  );
}

function Figure({ label, value, big }: { label: string; value: string; big?: boolean }) {
  return (
    <div className={s.figure}>
      <span className={s.figureLabel}>{label}</span>
      <span className={cx(s.figureValue, big && s.big)}>{value}</span>
    </div>
  );
}

/** What this carton has weighed before, and on what basis (D-weight baselines). */
function Expected({ expected }: { expected: ExpectedWeight }) {
  const gap = agreement(expected);
  return (
    <div className={s.figure}>
      <span className={s.figureLabel}>Expected</span>
      <span className={s.figureValue}>{kg(expected.grams)}</span>
      <span className={s.figureNote}>
        {provenance(expected)}
        {gap ? ` · ${gap}` : ""}
      </span>
    </div>
  );
}

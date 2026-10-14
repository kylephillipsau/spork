import { Ruler } from "lucide-react";

import { Badge, Button, Card, useDraggable, useDropTarget } from "@ui/index";
import type { Uuid } from "@domain/types";
import { ItemLine } from "@app/items/ItemProperties";

import { goodsKey, type Board, type BoardLine } from "./board";
import { needsCheck } from "./check";
import type { OrderLine } from "./order";
import s from "./pack-bench.module.css";

/**
 * The left of the board (D245): each line with its units still to place, to
 * drag onto a parcel or choose and move with M. Dropped here, goods come out
 * of a carton onto the bench (settled 2) and planned goods come back to place
 * again; a carton dropped here comes off its pallet.
 */
export function ToPack({ board, look, elsewhere }: { board: Board; look: (item: Uuid) => void; elsewhere: string | null }) {
  const drop = useDropTarget("bench");
  const left = board.lines.reduce((t, b) => t + b.left, 0);
  return (
    <Card
      title="To pack"
      count={left}
      description={[elsewhere, "Drag a line onto a parcel, or choose it and press M to move it."].filter(Boolean).join(". ")}
      padded={false}
    >
      <div {...drop.props} className={s.toPack}>
        <ul className={s.boardLines} aria-label="Lines to pack">
          {board.lines.map((b) => (
            <Row key={b.line.line_id} b={b} order={board.view.lines.find((l) => l.line === b.line.line_id) ?? null} look={look} />
          ))}
        </ul>
        <p className={s.dropHere} data-over={drop.over || undefined}>
          {drop.over ? "Let go to take it out onto the bench" : "Drop goods here to take them out of a carton"}
        </p>
      </div>
    </Card>
  );
}

function Row({ b, order, look }: { b: BoardLine; order: OrderLine | null; look: (item: Uuid) => void }) {
  const drag = useDraggable(goodsKey(b.line.line_id));
  const l = b.line;
  // An each with no size to fit by (D195): placed by hand, or measured first.
  const each = l.packs.find((p) => p.level === "each");
  const unmeasured = b.left > 0 && !!each && !each.size && !each.no_size;
  return (
    <li className={s.boardLine} data-done={b.left === 0 || undefined} {...(b.left > 0 ? drag.props : {})}>
      <ItemLine
        code={l.item_code}
        description={l.description ?? undefined}
        tags={l.tags}
        picture={l.picture}
        onOpen={() => look(l.item_id)}
        note={order ? whereWords(order) : undefined}
        done={b.left === 0}
      />
      <span className={s.lineSide}>
        <span className={s.lineLeft}>{b.left > 0 ? `${b.left} left` : "Placed"}</span>
        {needsCheck(l) ? <Badge tone="warning">Not checked</Badge> : <Badge tone="success">Checked</Badge>}
        {unmeasured && (
          <Button size="sm" variant="ghost" icon={<Ruler />} onClick={() => look(l.item_id)}>
            Measure
          </Button>
        )}
      </span>
    </li>
  );
}

/** "4 in Carton 2, 4 planned in Small Box": where a line's units are. */
function whereWords(l: OrderLine): string {
  const parts = l.where.map((w) => `${w.units} ${w.state === "planned" ? "planned in" : "in"} ${w.parcel}`);
  if (l.short > 0) parts.push(`${l.short} short at the check`);
  if (l.missing > 0) parts.push(`${l.missing} not accounted for`);
  return parts.join(", ");
}

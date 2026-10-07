import { Suspense, lazy } from "react";
import { Check, ChevronLeft, ChevronRight, LogOut, Undo2 } from "lucide-react";

import { Alert, Badge, Button, Card, Page, PageHeader } from "@ui/index";
import { Faint } from "@app/common/cells";
import { Thumb } from "@app/common/Thumb";

import type { ToPickDesk, TripWalk } from "./useToPick";
import s from "./walk-view.module.css";

const Map3D = lazy(() => import("@app/layout/Map3D"));

/**
 * Walking a trip (D230): the stop on screen, its bin highlighted in the 3D
 * warehouse with the route drawn on the floor from the bench and back (A* on
 * the layout, D211), and every stop in walking order. Tapping a stop, or its
 * bin on the map, goes to it; Got it ticks it off and moves on. The picks are
 * recorded in NetSuite as ever (D212): this only keeps your place.
 */
export function WalkView({ desk, walk }: { desk: ToPickDesk; walk: TripWalk }) {
  const { trip } = walk;
  const { map } = desk;
  const stop = trip.stops[walk.at];

  const done = walk.done.includes(walk.at);
  const finished = walk.done.length >= trip.stops.length;
  const total = (i: number) => trip.stops[i]!.takes.reduce((n, t) => n + t.quantity, 0);

  return (
    <Page>
      <PageHeader
        title={walk.label}
        description={`${trip.orders.join(", ")} · ${walk.done.length} of ${trip.stops.length} stops done`}
        actions={
          <Button icon={<LogOut size={16} />} onClick={desk.leaveWalk}>
            Leave the walk
          </Button>
        }
      />
      <div className={s.walk}>
        <section className={s.now} aria-label="This stop">
          {finished && <Alert tone="success">Every stop is done. Back to the packing bench, each order's goods with its ticket.</Alert>}
          {stop && (
            <Card>
              <div className={s.stop}>
                <div className={s.where}>
                  <Faint>
                    Stop {walk.at + 1} of {trip.stops.length}
                  </Faint>
                  <div className={s.bin}>
                    {stop.bin ?? "No bin"}
                    {stop.within_reach === false && <Badge tone="warning">High</Badge>}
                  </div>
                  {stop.off_route && <Faint>Not on the floor plan yet</Faint>}
                </div>
                <div className={s.item}>
                  <Thumb picture={walk.pictures[stop.code] ?? null} alt={stop.code} />
                  <div>
                    <div className={s.code}>{stop.code}</div>
                    {stop.description && <Faint>{stop.description}</Faint>}
                  </div>
                </div>
                <ul className={s.takes}>
                  {stop.takes.map((t) => (
                    <li key={t.order} className={t.gathered ? s.gathered : undefined}>
                      <span className={s.order}>{t.order}</span>
                      <span className={s.customer}>{t.gathered ? "for another group, to the bench" : (walk.customers[t.order] ?? "")}</span>
                      <span className={s.qty}>× {t.quantity}</span>
                    </li>
                  ))}
                </ul>
                <div className={s.actions}>
                  <Button icon={<ChevronLeft size={18} />} disabled={walk.at === 0} onClick={() => desk.goTo(walk.at - 1)}>
                    Back
                  </Button>
                  {done ? (
                    <Button icon={<Undo2 size={18} />} onClick={desk.notGot}>
                      Not got it
                    </Button>
                  ) : (
                    <Button variant="primary" size="lg" iconAfter={<ChevronRight size={18} />} onClick={desk.got}>
                      Got it{stop.takes.length > 1 ? `, all ${total(walk.at)}` : ""}
                    </Button>
                  )}
                </div>
              </div>
            </Card>
          )}
        </section>

        <section className={s.stage} aria-label="The route">
          {map.kind === "ready" ? (
            <Suspense fallback={<div className={s.canvas} />}>
              <Map3D
                plan={map.site.layout.plan}
                places={map.site.layout.places}
                bins={map.site.bins.bins}
                layer="stock"
                chosen={stop?.location_id ?? null}
                choose={(id) => {
                  const i = trip.stops.findIndex((st) => st.location_id === id);
                  if (i >= 0) desk.goTo(i);
                }}
                flight={walk.at + 1}
                route={trip.path.length ? trip.path : null}
              />
            </Suspense>
          ) : map.kind === "failed" ? (
            <Alert tone="danger">{map.message}</Alert>
          ) : (
            <div className={s.canvas} />
          )}
        </section>

        <ol className={s.stops} aria-label="Every stop">
          {trip.stops.map((st, i) => (
            <li key={i}>
              <button type="button" className={s.row} aria-current={i === walk.at ? "step" : undefined} onClick={() => desk.goTo(i)}>
                <span className={s.n}>{walk.done.includes(i) ? <Check size={16} aria-label="Done" /> : i + 1}</span>
                <span className={s.rowBin}>{st.bin ?? "No bin"}</span>
                <span className={s.rowCode}>{st.code}</span>
                <span className={s.rowQty}>{total(i)}</span>
              </button>
            </li>
          ))}
        </ol>
      </div>
    </Page>
  );
}

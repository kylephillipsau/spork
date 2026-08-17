import { useState } from "react";
import {
  Band,
  Chooser,
  Code,
  EmptySlot,
  Face,
  FaceWell,
  Fact,
  Faint,
  Key,
  Notice,
  Panel,
  Pill,
  Readout,
  Record,
  Records,
  Row,
  Soft,
  Spacer,
  Stack,
  Tag,
  grams,
  millimetres,
} from "@design/index";
import type { BookedConsignment, DespatchScreen, WaitingJob } from "@domain/types";
import type { DespatchBench } from "./useDespatch";

/**
 * Stages 6 to 9 of the recorded process, on one screen.
 *
 * Today those stages are: NetSuite pushes a record to MachShip and creates a
 * *pending consignment*; the operator finds it again **by matching the delivery
 * address by eye**; picks a route from memory; re-enters the carton count,
 * which "exists in neither system beforehand"; and prints.
 *
 * Three of those are gone by construction. A consignment is a row with an
 * identifier, so nothing is matched by eye. The count is a fold. The route is a
 * list rather than a thing to remember.
 */
export function Despatch({ bench }: { bench: DespatchBench }) {
  if (bench.status.kind === "loading") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Faint>Loading…</Faint>
        </Face>
      </Panel>
    );
  }

  if (bench.status.kind === "failed") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Notice>{bench.status.message}</Notice>
      </Panel>
    );
  }

  const screen = bench.status.screen;

  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <FaceWell>
            <Row gap={4} align="baseline" wrap>
              <Soft>Despatch</Soft>
              <Spacer />
              <Tag>{screen.site}</Tag>
              <Pill tone={screen.waiting.length > 0 ? "state" : "good"}>
                {screen.waiting.length > 0
                  ? `${screen.waiting.length} to consign`
                  : "Nothing waiting"}
              </Pill>
            </Row>
          </FaceWell>
        </Face>

        <Waiting screen={screen} bench={bench} />

        {screen.booked.map((consignment) => (
          <Booked key={consignment.consignment_id} consignment={consignment} bench={bench} />
        ))}

        <GoneToday screen={screen} />

        {(bench.problem || bench.notice) && (
          <Notice
            kind={bench.problem ? "finding" : "recorded"}
            onDismiss={bench.dismiss}
            notes={bench.booked?.warnings.map((w) => (
              <Faint key={w}>{w}</Faint>
            ))}
          >
            {bench.problem ??
              `Booked ${bench.booked?.packages} carton(s) with ${bench.booked?.carrier ?? "the carrier"}.`}
          </Notice>
        )}

        {bench.booked && bench.booked.lines.length > 0 && (
          <Manifest booked={bench.booked} />
        )}
      </Stack>
    </Panel>
  );
}

/**
 * Sealed and on no consignment, grouped by the job it belongs to.
 *
 * **Grouped rather than listed**, because a consignment is booked against a
 * customer's goods and a flat list of cartons makes the operator reassemble
 * that grouping in their head — which is stage 6's eyeball match, moved rather
 * than removed.
 */
function Waiting({ screen, bench }: { screen: DespatchScreen; bench: DespatchBench }) {
  const [carrier, setCarrier] = useState(screen.carriers[0]?.id ?? "");
  const [service, setService] = useState(screen.carriers[0]?.services[0]?.id ?? "");

  const chosen = screen.carriers.find((c) => c.id === carrier);
  const services = chosen?.services ?? [];

  return (
    <Face pad={false}>
      <Band count={screen.waiting.length}>Waiting to consign</Band>
      <FaceWell>
        <Stack gap={4}>
          {screen.waiting.length === 0 ? (
            <EmptySlot
              label="Dock clear"
              note="All sealed cartons are booked."
            />
          ) : (
            <Records>
              {screen.waiting.map((job) => (
                <Job key={job.fulfilment_id} job={job} bench={bench} carrier={carrier} service={service} />
              ))}
            </Records>
          )}

          {/* The route, as data. Stage 7 has the operator apply a carrier's
              rules from memory; D1 keeps the carrier and the way we reach them
              separate, so the service is a second choice rather than a suffix
              on the first. */}
          <Row gap={4} align="end" wrap>
            <Chooser
              label="Carrier"
              value={carrier}
              onChange={(next) => {
                setCarrier(next);
                const first = screen.carriers.find((c) => c.id === next)?.services[0]?.id ?? "";
                setService(first);
              }}
              options={screen.carriers.map((c) => ({ value: c.id, label: c.name }))}
            />
            <Chooser
              label="Service"
              value={service}
              onChange={setService}
              options={services.map((s) => ({ value: s.id, label: s.name }))}
            />
            <Spacer />
            <Faint>
              {screen.providers.length > 0
                ? `Booked through ${screen.providers.map((p) => p.name).join(", ")}`
                : "No freight provider configured"}
            </Faint>
          </Row>
        </Stack>
      </FaceWell>
    </Face>
  );
}

function Job({
  job,
  bench,
  carrier,
  service,
}: {
  job: WaitingJob;
  bench: DespatchBench;
  carrier: string;
  service: string;
}) {
  const unweighed = job.cartons.filter((c) => !c.weighed).length;

  return (
    <Record
      name={<Code>{job.reference}</Code>}
      tags={
        <>
          <Soft>{job.customer}</Soft>
          <Code>{job.order_reference}</Code>
        </>
      }
      facts={
        <>
          <Fact
            value={job.gross_weight_g === null ? "—" : grams(job.gross_weight_g)}
            label="gross"
            {...(job.gross_weight_g === null ? {} : { unit: "kg" })}
          />
          <Fact
            value={job.cartons.length}
            label={`carton${job.cartons.length === 1 ? "" : "s"}`}
          />
        </>
      }
      meta={
        /* **Said before it leaves rather than after.** A carton nobody weighed
           is a carton the carrier weighs, and their figure arrives attached to
           their invoice — which is the comparison the whole measurement side
           of this system exists to make possible. */
        unweighed > 0 ? <Pill tone="state">{unweighed} never weighed</Pill> : undefined
      }
      action={
        /* Not lit. Consigning is preparation and reversible up to the moment
           the goods move; despatching is not, and the lit key on this screen
           is the one that commits them to leaving. Three violet keys said
           three primary actions and meant none. */
        <Key
          size="small"
          disabled={bench.busy || !carrier}
          onClick={() =>
            void bench.consign({
              packages: job.cartons.map((c) => c.id),
              ...(carrier ? { carrier } : {}),
              ...(service ? { service } : {}),
            })
          }
        >
          Consign {job.cartons.length}
        </Key>
      }
    />
  );
}

function Booked({
  consignment,
  bench,
}: {
  consignment: BookedConsignment;
  bench: DespatchBench;
}) {
  const gone = consignment.package_count - consignment.awaiting_despatch;

  return (
    <Face pad={false} as="section">
      <Band count={consignment.package_count}>
        <Row gap={3} align="baseline">
          <span>{consignment.carrier_name ?? "No carrier named"}</span>
          {consignment.carrier_service_name && <Faint>{consignment.carrier_service_name}</Faint>}
          {consignment.awaiting_despatch === 0 && <Pill tone="state">Gone</Pill>}
        </Row>
      </Band>
      <FaceWell>
        <Stack gap={4}>
          {/* **A section row, not a Record.** The consignment is the heading
              this list of cartons sits under, and its key acts on all of them
              — a different level from the per-carton key below, which is why
              Record's one-action rule is not being bent here (D167). */}
          <Row gap={4} wrap align="baseline">
            <Fact
              value={consignment.gross_weight_g === null ? "—" : grams(consignment.gross_weight_g)}
              label="gross"
              {...(consignment.gross_weight_g === null ? {} : { unit: "kg" })}
            />
            <Fact value={consignment.package_count} label="cartons" />
            <Fact value={`${gone} / ${consignment.package_count}`} label="gone" />
            {/* The carrier's own word, and null until one has been asked. It is
                not ours to invent — the application holds no INSERT on it. */}
            <Fact value={consignment.status ?? "—"} label="carrier says" />
            <Spacer />
            {consignment.awaiting_despatch > 0 && (
              <Key
                live
                size="small"
                disabled={bench.busy}
                onClick={() => void bench.despatchAll(consignment.packages)}
              >
                Despatch {consignment.awaiting_despatch}
              </Key>
            )}
          </Row>

          <Records>
            {consignment.packages.map((carton) => (
              <Record
                key={carton.id}
                name={<Code>{carton.reference}</Code>}
                tags={<Faint>carton {carton.sequence}</Faint>}
                action={
                  carton.despatched ? (
                    <Pill tone="state">Gone</Pill>
                  ) : (
                    <Key
                      size="small"
                      disabled={bench.busy}
                      onClick={() => void bench.despatchCarton(carton)}
                    >
                      Despatch
                    </Key>
                  )
                }
              />
            ))}
          </Records>
        </Stack>
      </FaceWell>
    </Face>
  );
}

function GoneToday({ screen }: { screen: DespatchScreen }) {
  return (
    <Face pad={false}>
      <Band count={screen.gone_today.length}>Despatched today</Band>
      <FaceWell>
        {screen.gone_today.length === 0 ? (
          <Faint>Nothing has left this site today.</Faint>
        ) : (
          <Records>
            {screen.gone_today.map((c) => (
              <Record
                key={c.consignment_id}
                name={<span>{c.carrier_name ?? "No carrier named"}</span>}
                facts={
                  <Fact
                    value={c.package_count}
                    label={`carton${c.package_count === 1 ? "" : "s"}`}
                  />
                }
              />
            ))}
          </Records>
        )}
      </FaceWell>
    </Face>
  );
}

/**
 * What the carrier is handed.
 *
 * **This is the output of consigning, and until now the screen kept only the
 * number of rows.** A carrier line is one row per kind of package: the box, the
 * carrier's own code for it, how many, and the weight and dimensions somebody
 * confirmed at the bench. The count is a fold over what is physically in the
 * cartons, so it is a number that exists in neither system before this moment.
 *
 * `uniform` is the honest part. When several cartons of one type disagree about
 * their size, the line carries the count and refuses to state a size, because
 * one made-up figure covering four different boxes is what a carrier invoice
 * later disagrees with.
 *
 * The weight on a line is the sum for that line and the dimensions are one
 * carton's, because the server totals `gross_weight_g` and takes `min` of each
 * side. Two numbers of different grain on one row is how a manifest is read
 * wrongly, so each says which it is.
 *
 * Nothing here is sent anywhere. No carrier has been asked, which is why the
 * consignment has no status, and saying so is better than implying otherwise.
 */
function Manifest({ booked }: { booked: NonNullable<DespatchBench["booked"]> }) {
  return (
    <Face pad={false}>
      <Band count={booked.lines.length}>Consignment manifest</Band>
      <FaceWell>
        <Stack gap={3}>
          <Row gap={3} align="baseline" wrap>
            {booked.service && <Code>{booked.service}</Code>}
            <Spacer />
            {/* **The one instrument on this panel** — the figure the carrier
                bills against, and the reason a manifest gets read at all. The
                per-line weights below are facts about their lines (D167). */}
            {booked.total_gross_weight_g !== null && (
              <Readout label="Gross" value={grams(booked.total_gross_weight_g)} unit="kg" />
            )}
          </Row>

          <Records>
            {booked.lines.map((line, i) => (
              <Record
                key={`${line.package_type ?? "line"}-${i}`}
                name={<Code>{line.carrier_package_code ?? "—"}</Code>}
                tags={<Faint>{line.package_type ?? "unstated"}</Faint>}
                facts={
                  <>
                    <Fact
                      value={line.gross_weight_g === null ? "—" : grams(line.gross_weight_g)}
                      label="gross"
                      {...(line.gross_weight_g === null ? {} : { unit: "kg" })}
                    />
                    <Fact
                      value={line.package_count}
                      label={line.package_count === 1 ? "carton" : "cartons"}
                    />
                  </>
                }
                meta={
                  <>
                    {/* L×W×H reads as one measurement and wraps as one. Four
                        separate readouts let a height land on its own line. */}
                    {line.length_mm !== null && (
                      <Faint>
                        {millimetres(line.length_mm)} ×{" "}
                        {line.width_mm === null ? "—" : millimetres(line.width_mm)} ×{" "}
                        {line.height_mm === null ? "—" : millimetres(line.height_mm)} mm{" "}
                        {line.uniform ? "each" : "smallest"}
                      </Faint>
                    )}
                    {!line.uniform && <Pill tone="state">sizes differ</Pill>}
                  </>
                }
              />
            ))}
          </Records>

          <Faint>
            Recorded here only. No carrier has been contacted, so there is no
            status yet.
          </Faint>
        </Stack>
      </FaceWell>
    </Face>
  );
}

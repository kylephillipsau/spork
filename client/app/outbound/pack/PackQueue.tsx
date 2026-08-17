import {
  Band,
  EmptySlot,
  Face,
  FaceWell,
  Fact,
  Faint,
  Field,
  Key,
  Link,
  Notice,
  Panel,
  Pill,
  Record,
  Records,
  Row,
  Stack,
} from "@design/index";
import { href } from "@app/routing/location";
import type { PackJob } from "@domain/types";
import { STAGES, STAGE_LABELS, type QueueBench } from "./useQueue";

/**
 * What there is to pack, at this site.
 *
 * # It opens on the work, not on a search box
 *
 * One of the few things the pages this replaces got right, and worth saying
 * again: the operator has this open all day and the common case is *what is
 * next*, not *find me one*. The search narrows a list that is already there.
 *
 * # The groups are computed and the server computes them
 *
 * S44 forbids a stored progress column, because any label it held would be a
 * function of coverage quantities that can disagree with it. `packing::stage`
 * is that function, and the server-rendered page calls the same one — so the
 * two screens cannot disagree about whether a commitment is ready.
 *
 * A group with nothing in it is not drawn. Four empty headings is a screen
 * telling you four times that there is nothing to do.
 */
export function PackQueue({ bench }: { bench: QueueBench }) {
  return (
    <Stack gap={3}>
      <Panel elevation="raised" frame="bezel" as="section">
        <Face>
          <FaceWell>
            <Row gap={4} align="end" wrap>
              <Field
                label="Reference, order or customer"
                numeric={false}
                value={bench.term}
                onChange={bench.type}
                onSubmit={() => void bench.search()}
              />
              <Key onClick={() => void bench.search()}>Narrow</Key>
            </Row>
          </FaceWell>
        </Face>
      </Panel>

      {bench.state.kind === "loading" && (
        <Panel elevation="raised" frame="bezel">
          <Face>
            <Faint>Loading…</Faint>
          </Face>
        </Panel>
      )}

      {bench.state.kind === "failed" && (
        <Panel elevation="raised" frame="bezel">
          <Notice>{bench.state.message}</Notice>
        </Panel>
      )}

      {bench.state.kind === "ready" &&
        (bench.state.jobs.length === 0 ? (
          <Panel elevation="raised" frame="bezel">
            <Face>
              <Stack gap={3}>
                <EmptySlot label="Nothing to pack at this site" />
                {bench.term.trim() && <Faint>Nothing matches that search.</Faint>}
              </Stack>
            </Face>
          </Panel>
        ) : (
          <Panel elevation="lifted" frame="bezel" as="section">
            <Stack gap={3}>
              {STAGES.map((stage) => {
                const jobs = bench.state.kind === "ready"
                  ? bench.state.jobs.filter((j) => j.stage === stage)
                  : [];
                if (jobs.length === 0) return null;
                return (
                  <Face key={stage} pad={false}>
                    <Band>{STAGE_LABELS[stage]}</Band>
                    <FaceWell>
                      <Records>
                        {jobs.map((job) => (
                          <Job key={job.fulfilment_id} job={job} />
                        ))}
                      </Records>
                    </FaceWell>
                  </Face>
                );
              })}
            </Stack>
          </Panel>
        ))}
    </Stack>
  );
}

function Job({ job }: { job: PackJob }) {
  return (
    <Record
      name={
        <Link href={href(`/pack/${job.fulfilment_id}`)}>
          {job.reference ?? job.order_reference ?? "no reference"}
        </Link>
      }
      tags={<Faint>{job.customer}</Faint>}
      facts={
        <>
          {/* **Quantities, never a status**, per S44 — and a partly picked job
              says how far rather than merely failing a gate. */}
          <Fact value={`${job.picked}/${job.committed}`} label="picked" />
          <Fact value={job.lines} label={job.lines === 1 ? "line" : "lines"} />
          {job.cartons > 0 && (
            <Fact value={job.cartons} label={job.cartons === 1 ? "carton" : "cartons"} />
          )}
        </>
      }
      meta={
        /* Phrased by the server (D114): "overdue" is an opinion about a date,
           and opinions do not get formatted on the client. */
        job.due ? (
          <Pill tone={job.due === "overdue" ? "state" : "good"}>{job.due}</Pill>
        ) : undefined
      }
    />
  );
}

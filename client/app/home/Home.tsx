import {
  Band,
  Face,
  FaceWell,
  Faint,
  Fact,
  Lamp,
  Link,
  Panel,
  Record,
  Records,
  Row,
  Stack,
} from "@design/index";
import { href } from "@app/routing/location";
import type { HomeBench } from "./useHome";

/**
 * What is waiting for you, at this site, now.
 *
 * The front door. It used to be a fixture — `/` redirected to `/ui/`, which
 * drew a pack bench full of invented data — which is most of why the whole
 * thing read as a demo.
 *
 * # Grouped by work, not by table
 *
 * D110: *"the entity-oriented menu is the disease. Orders. Fulfilments.
 * Packages. Items. It is a rendering of the schema, and it requires the operator
 * to know which noun holds the thing they want."* So the groups are Outbound
 * and Integrity, and each row is a job rather than a table.
 *
 * # Zero hides
 *
 * D112, and it is the rule the whole screen turns on: *"a badge showing nothing
 * to do is a badge people stop reading."* A group with nothing waiting is drawn
 * as a sentence saying so, not as a row with a nought beside it.
 *
 * # Nothing waiting and no site chosen are opposite instructions
 *
 * They must not look the same. One means go home; the other means the software
 * does not yet know where you are standing.
 */
export function Home({ bench }: { bench: HomeBench }) {
  if (bench.state.kind === "loading") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Faint>Loading…</Faint>
        </Face>
      </Panel>
    );
  }

  if (bench.state.kind === "failed") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Row gap={3} wrap>
            <Lamp kind="finding" />
            <span>{bench.state.message}</span>
          </Row>
        </Face>
      </Panel>
    );
  }

  const w = bench.state.work;

  if (w.no_site) {
    return (
      <Panel elevation="lifted" frame="bezel">
        <Stack gap={3}>
          <Face>
            <Row gap={3} wrap>
              <Lamp kind="finding" />
              <span>No warehouse set.</span>
            </Row>
          </Face>
          <Face>
            <Stack gap={3}>
              <Row gap={3}>
                <Link href={href("/where")}>Say where you are working</Link>
              </Row>
            </Stack>
          </Face>
        </Stack>
      </Panel>
    );
  }

  const outbound = [
    { label: "Pack", count: w.pack, path: "/pack", noun: "left to pick" },
    { label: "Pick", count: w.pick, path: "/picking", noun: "lines to walk" },
    { label: "Despatch", count: w.despatch, path: "/despatch", noun: "sealed, not booked" },
  ];
  const integrity = [
    { label: "Findings", count: w.findings, path: "/findings", noun: "open" },
  ];
  const quiet = [...outbound, ...integrity].every((r) => r.count === 0);

  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        {quiet ? (
          <Face>
            {/* The drawn absence. D127: absence is drawn rather than left as a
                gap, and a queue that is genuinely empty is the system working
                rather than a screen that failed to load. */}
            <Row gap={3} wrap>
              <Lamp kind="recorded" />
              <span>Nothing is waiting at this site.</span>
            </Row>
          </Face>
        ) : (
          <>
            <Group title="Outbound" rows={outbound} />
            <Group title="Integrity" rows={integrity} />
          </>
        )}

      </Stack>
    </Panel>
  );
}

function Group({
  title,
  rows,
}: {
  title: string;
  rows: { label: string; count: number; path: string; noun: string }[];
}) {
  const waiting = rows.filter((r) => r.count > 0);
  if (waiting.length === 0) return null;
  return (
    <Face pad={false}>
      <Band>{title}</Band>
      <FaceWell>
        <Records>
          {waiting.map((r) => (
            <Record
              key={r.label}
              name={<Link href={href(r.path)}>{r.label}</Link>}
              facts={<Fact value={r.count} label={r.noun} />}
            />
          ))}
        </Records>
      </FaceWell>
    </Face>
  );
}

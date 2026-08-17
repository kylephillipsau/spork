import {
  Band,
  Code,
  Face,
  FaceWell,
  Faint,
  Lamp,
  Panel,
  Pill,
  Row,
  Soft,
  Spacer,
  Stack,
  Table,
  Num,
  NumHead,
} from "@design/index";
import type { WorkspaceSite } from "@domain/types";
import type { WorkspaceBench } from "./useWorkspace";

/**
 * The organisation and its warehouses.
 *
 * Bin counts are the reason this is a table rather than a list: a site with
 * none reads identically to a loaded one until something counts them, and the
 * import creates a site as soon as a warehouse appears in the export.
 */

function when(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

function SiteRow({ site }: { site: WorkspaceSite }) {
  return (
    <tr>
      <td>
        <Code>{site.code}</Code>
        {site.current && <Pill tone="good">here</Pill>}
      </td>
      <td>
        {site.name}
        {!site.active && <Pill tone="quiet">inactive</Pill>}
      </td>
      <td>
        <Faint>{site.timezone}</Faint>
      </td>
      <Num>{site.locations.toLocaleString()}</Num>
      <Num>{site.sequenced.toLocaleString()}</Num>
    </tr>
  );
}

export function Workspace({ bench }: { bench: WorkspaceBench }) {
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

  const { organisation, sites } = bench.state.workspace;

  return (
    <Stack gap={3}>
      <Panel elevation="raised" frame="bezel">
        <Face pad={false}>
          <Band>Organisation</Band>
          <FaceWell>
            <Stack gap={3}>
              <Row gap={3} align="baseline" wrap>
                <Soft>{organisation.name}</Soft>
                <Code>{organisation.slug}</Code>
                <Spacer />
                <Faint>set up {when(organisation.created_at)}</Faint>
              </Row>
              <Row gap={3} align="baseline" wrap>
                <Faint>id</Faint>
                <Code>{organisation.id}</Code>
              </Row>
            </Stack>
          </FaceWell>
        </Face>
      </Panel>

      <Panel elevation="raised" frame="bezel">
        <Face pad={false}>
          <Band count={sites.length}>Warehouses</Band>
          <FaceWell>
            {sites.length === 0 ? (
              <Faint>None.</Faint>
            ) : (
              <Table
                min="wide"
                head={
                  <tr>
                    <th>Code</th>
                    <th>Name</th>
                    <th>Timezone</th>
                    <NumHead>Bins</NumHead>
                    <NumHead>Sequenced</NumHead>
                  </tr>
                }
              >
                {sites.map((s) => (
                  <SiteRow key={s.id} site={s} />
                ))}
              </Table>
            )}
          </FaceWell>
        </Face>
      </Panel>
    </Stack>
  );
}

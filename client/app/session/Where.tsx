import {
  Band,
  Code,
  Face,
  FaceWell,
  Faint,
  Key,
  Lamp,
  Notice,
  Panel,
  Pill,
  Record,
  Records,
  Row,
  Soft,
  Spacer,
  Stack,
} from "@design/index";
import type { WhereBench } from "./useWhere";

/**
 * Where are you working.
 *
 * One question, asked once, on the way in. It exists because every act a person
 * records names the site they recorded it at (D11), every badge counts work
 * *at this site* (D112), and until now no browser session had one — the sign-in
 * page has always posted an email and a password and nothing else.
 *
 * **A tenant with one warehouse never sees this**, which is the ordinary case
 * and the reason the screen is not part of signing in. A question with one
 * possible answer is a screen that exists to be dismissed.
 */
export function Where({ bench }: { bench: WhereBench }) {
  if (bench.state.kind === "asking") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Face>
          <Faint>Loading warehouses…</Faint>
        </Face>
      </Panel>
    );
  }

  if (bench.state.kind === "failed") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Notice>{bench.state.message}</Notice>
      </Panel>
    );
  }

  if (bench.state.kind === "nowhere") {
    return (
      <Panel elevation="raised" frame="bezel">
        <Stack gap={3}>
          <Face>
            <Row gap={3} wrap>
              <Lamp kind="finding" />
              <span>Your account is not attached to a warehouse.</span>
            </Row>
          </Face>
          <Face>
            {/* **Not a retry.** Nothing the operator can do from here changes
                it, so the screen says who can rather than offering a button
                that will fail the same way. */}
            <Faint>
An administrator must attach one before there is work to show.
            </Faint>
          </Face>
        </Stack>
      </Panel>
    );
  }

  if (bench.state.kind === "settled") {
    const s = bench.state.site;
    return (
      <Panel elevation="lifted" frame="bezel">
        <Stack gap={3}>
          <Face>
            <Row gap={3} wrap>
              <Lamp kind="recorded" />
              <span>Working at</span>
              <Code>{s.code}</Code>
              <span>{s.name}</span>
              <Spacer />
              <Pill tone="good">set</Pill>
            </Row>
          </Face>
          <Face>
            <Faint>
Your work is recorded against this warehouse.
            </Faint>
          </Face>
        </Stack>
      </Panel>
    );
  }

  const sites = bench.state.sites;
  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face>
          <FaceWell>
            <Row gap={3} align="baseline" wrap>
              <Soft>Where are you working?</Soft>
              <Spacer />
              <Pill tone="state">{`${sites.length} warehouses`}</Pill>
            </Row>
          </FaceWell>
        </Face>

        <Face pad={false}>
          <Band>Choose a site</Band>
          <FaceWell>
            <Records>
              {sites.map((s) => (
                <Record
                  key={s.id}
                  name={<Code>{s.code}</Code>}
                  tags={<span>{s.name}</span>}
                  action={
                    <Key
                      size="small"
                      disabled={bench.busy}
                      onClick={() => void bench.choose(s.id)}
                    >
                      Work here
                    </Key>
                  }
                />
              ))}
            </Records>
          </FaceWell>
        </Face>

        {bench.problem && (
          <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>
        )}

        <Face>
          <Faint>
Change it by signing in again.
          </Faint>
        </Face>
      </Stack>
    </Panel>
  );
}

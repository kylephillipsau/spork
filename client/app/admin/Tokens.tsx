import {
  Band,
  Code,
  Face,
  FaceWell,
  Faint,
  Field,
  Key,
  Lamp,
  Panel,
  Pill,
  Record,
  Records,
  Row,
  Soft,
  Spacer,
  Stack,
} from "@design/index";
import type { ApiToken } from "@domain/types";
import { standing } from "./useTokens";
import type { TokensBench } from "./useTokens";

/**
 * Import tokens (D158).
 *
 * # The secret is the whole design problem
 *
 * A token exists in readable form in exactly one HTTP response. Everything
 * else on this screen is a list of labels and dates, and the only moment that
 * matters is the one after minting — so the secret gets its own panel, stays
 * until it is dismissed rather than until the next render, and says the one
 * thing that is genuinely irreversible about it.
 *
 * That sentence is the only explanatory text here. Everything else is a label,
 * a date or a state, because this is a screen for somebody who already knows
 * what they came to do.
 */

function when(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

function Standing({ token }: { token: ApiToken }) {
  const state = standing(token);
  if (state === "revoked") return <Pill tone="quiet">withdrawn</Pill>;
  if (state === "expired") return <Pill tone="quiet">expired</Pill>;
  return <Pill tone="good">live</Pill>;
}

export function Tokens({ bench }: { bench: TokensBench }) {
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

  const tokens = bench.state.tokens;

  return (
    <Stack gap={3}>
      {/* **Shown once, and the screen says so.** Not a warning about being
          careful — a statement of what the server can and cannot do, which is
          the one thing the operator cannot work out from the layout. */}
      {bench.minted && (
        <Panel elevation="lifted" frame="bezel">
          <Face>
            <Stack gap={3}>
              <Row gap={3} wrap>
                <Lamp kind="recorded" />
                <span>{bench.minted.label}</span>
                <Spacer />
                <Faint>expires {when(bench.minted.expires_at)}</Faint>
              </Row>
              <Code>{bench.minted.token}</Code>
              <Row gap={3} align="center">
                <Soft>Copy it now. It is not shown again.</Soft>
                <Spacer />
                <Key size="small" onClick={bench.dismiss}>
                  Done
                </Key>
              </Row>
            </Stack>
          </Face>
        </Panel>
      )}

      <Panel elevation="raised" frame="bezel">
        <Face pad={false}>
          <Band>New token</Band>
          <FaceWell>
            <Stack gap={3}>
              <Row gap={3} wrap align="end">
                <Field
                  label="What it is for"
                  value={bench.draft.label}
                  numeric={false}
                  onChange={(next) => bench.type("label", next)}
                  onSubmit={() => void bench.mint()}
                />
                <Field
                  label="Days"
                  width="measure"
                  value={bench.draft.days}
                  onChange={(next) => bench.type("days", next)}
                  onSubmit={() => void bench.mint()}
                />
                <Spacer />
                <Key
                  live
                  disabled={bench.busy || !bench.draft.label.trim()}
                  onClick={() => void bench.mint()}
                >
                  Mint
                </Key>
              </Row>
              {bench.problem && (
                <Row gap={3} wrap>
                  <Lamp kind="finding" />
                  <span>{bench.problem}</span>
                </Row>
              )}
            </Stack>
          </FaceWell>
        </Face>
      </Panel>

      <Panel elevation="raised" frame="bezel">
        <Face pad={false}>
          <Band>Tokens</Band>
          <FaceWell>
            {tokens.length === 0 ? (
              <Faint>None.</Faint>
            ) : (
              <Records>
                {tokens.map((t) => (
                  <Record
                    key={t.id}
                    lead={<Standing token={t} />}
                    name={<span>{t.label}</span>}
                    meta={
                      <>
                        <Faint>
                          {t.last_used_at ? `used ${when(t.last_used_at)}` : "never used"}
                        </Faint>
                        <Faint>expires {when(t.expires_at)}</Faint>
                      </>
                    }
                    action={
                      standing(t) === "live" ? (
                        <Key
                          size="small"
                          disabled={bench.busy}
                          onClick={() => void bench.revoke(t.id)}
                        >
                          Withdraw
                        </Key>
                      ) : undefined
                    }
                  />
                ))}
              </Records>
            )}
          </FaceWell>
        </Face>
      </Panel>
    </Stack>
  );
}

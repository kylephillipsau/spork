import { useState } from "react";
import {
  Band,
  Boot,
  Code,
  Dim,
  EvidencePair,
  Face,
  FaceWell,
  Faint,
  Finding,
  Grid,
  Key,
  Lamp,
  LightRoom,
  Nylon,
  Panel,
  Pill,
  Readout,
  Row,
  Spacer,
  Stack,
  Tag,
  Well,
} from "@design/index";
import styles from "./gallery.module.css";

/**
 * The design package, rendered from a fixture with no network.
 *
 * That is the point of it: both densities and both faces have to be
 * reviewable without a running warehouse, which is also what makes the laws
 * checkable in CI rather than in a screenshot.
 */
export function Gallery() {
  const [density, setDensity] = useState<"floor" | "desk">("desk");

  return (
    <LightRoom density={density}>
      <div className={styles.page}>
        <Panel elevation="lifted" frame="bezel" as="header">
          <div className={styles.pad}>
            <Row gap={4} wrap>
              <Tag dyed>Nylonite · design</Tag>
              <Tag>Draft 5</Tag>
              <Spacer />
              <Key size="small" onClick={() => setDensity("floor")}>
                Floor
              </Key>
              <Key size="small" live={density === "desk"} onClick={() => setDensity("desk")}>
                Desk
              </Key>
            </Row>
          </div>
        </Panel>

        {/* ── the six layers ─────────────────────────────────────── */}
        <Panel elevation="raised" frame="bezel" as="section">
          <div className={styles.pad}>
            <Stack gap={4}>
              <span className={styles.legend}>L1 · L2 · L4 · L5 — the chassis</span>
              <Row gap={4} wrap>
                <Key live>Seal and consign</Key>
                <Key>Print packing list</Key>
                <Key size="small">Investigate</Key>
                <Key disabled>Void</Key>
                <Spacer />
                <Row gap={3}>
                  <Lamp kind="finding" />
                  <Lamp kind="active" />
                  <Lamp kind="recorded" />
                  <Lamp kind="off" />
                </Row>
              </Row>
              <Well>
                <div className={styles.pad}>
                  <span className={styles.legend}>
                    A machined pocket. The chamfer inverts: light beneath, shadow above.
                  </span>
                </div>
              </Well>
            </Stack>
          </div>
        </Panel>

        {/* ── L3, the instrument face ────────────────────────────── */}
        <Panel elevation="raised" frame="bezel" as="section">
          <div className={styles.pad}>
            <span className={styles.legend}>L3 — the face. The chassis is metal, the data is paper.</span>
          </div>
          <Face>
            <Stack gap={4}>
              <FaceWell>
                <Row gap={3} align="baseline" wrap>
                  <Code>IF400187</Code>
                  <Faint>Harbourline Provisions Pty Ltd</Faint>
                  <Spacer />
                  <Pill>Picked 12 / 12</Pill>
                </Row>
              </FaceWell>

              <Band count={3}>Cartons</Band>

              <Grid columns={2} gap={4}>
                <FaceWell>
                  <Stack gap={3}>
                    <Row gap={5} wrap align="start">
                      <Readout label="Weight" value="412.500" unit="kg" size="large" />
                      <Readout label="Height" value="1840" unit="mm" size="large" />
                    </Row>
                    <Dim from="1165" to="1165" label="L × W, mm" />
                  </Stack>
                </FaceWell>

                <Finding kind="Weight disagreement">
                  <Stack gap={3}>
                    <EvidencePair
                      expected="386.100"
                      observed="412.500"
                      expectedLabel="On file"
                    />
                    <Faint>
                      +26.400 kg, 6.8% — above the tenth and above the five-gram floor. Both
                      readings stay on file and this carries the pair.
                    </Faint>
                  </Stack>
                </Finding>
              </Grid>
            </Stack>
          </Face>
        </Panel>

        {/* ── L1b, nylon ─────────────────────────────────────────── */}
        <Grid columns={2} gap={4}>
          <Panel elevation="raised" frame="bezel">
            <div className={styles.pad}>
              <Stack gap={4}>
                <span className={styles.legend}>
                  L1b — ripstop. Metal encloses; nylon protects and labels.
                </span>
                <Nylon elevation="raised">
                  <div className={styles.pad}>
                    <Row gap={3} wrap>
                      <Tag dyed>Melbourne</Tag>
                      <Tag>MEL-BENCH-2</Tag>
                    </Row>
                  </div>
                </Nylon>
              </Stack>
            </div>
          </Panel>

          <Boot>
            <Panel elevation="flush" frame="bezel">
              <Face>
                <Stack gap={3}>
                  <Readout label="Go to" value="K.32.01" size="large" />
                  <FaceWell>
                    <Code>GLV-NIT-BLU-M</Code>
                  </FaceWell>
                  <Row gap={5}>
                    <Readout label="Take" value="8" unit="ea" size="large" />
                    <Readout label="On hand" value="124" />
                  </Row>
                  <Key live block>
                    Confirm pick
                  </Key>
                </Stack>
              </Face>
            </Panel>
          </Boot>
        </Grid>
      </div>
    </LightRoom>
  );
}

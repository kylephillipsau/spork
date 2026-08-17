import {
  Band,
  Chooser,
  Code,
  Face,
  FaceWell,
  Faint,
  Field,
  Key,
  Notice,
  Num,
  NumHead,
  Panel,
  Pill,
  Row,
  Soft,
  Spacer,
  Stack,
  Table,
} from "@design/index";
import type { ImportReport, ItemImportReport } from "@domain/types";
import type { ImportBench } from "./useImport";
import styles from "./import.module.css";

/**
 * Loading a bin list.
 *
 * Dry run first, then apply — the same two steps the endpoint offers, drawn as
 * two keys. The dry run performs the writes and rolls them back, so the numbers
 * below are what applying will do rather than an estimate of it.
 */

function Report({ report, applied }: { report: ImportReport; applied: boolean }) {
  const { survey, loaded } = report;
  return (
    <Stack gap={3}>
      <Panel elevation={applied ? "lifted" : "raised"} frame="bezel">
        <Face pad={false}>
          <Band count={survey.bins}>{applied ? "Loaded" : "Would load"}</Band>
          <FaceWell>
            <Stack gap={4}>
              <Table
                min="wide"
                head={
                  <tr>
                    <th>Warehouse</th>
                    <NumHead>Bins</NumHead>
                    <NumHead>Typed</NumHead>
                    <NumHead>Untyped</NumHead>
                    <th>Clock</th>
                  </tr>
                }
              >
                {survey.sites.map((s) => (
                  <tr key={s.warehouse}>
                    <td>
                      {s.warehouse}
                      {s.skipped && <Pill tone="quiet">skipped</Pill>}
                    </td>
                    <Num>{s.bins.toLocaleString()}</Num>
                    <Num>{s.typed.toLocaleString()}</Num>
                    <Num>{s.untyped.toLocaleString()}</Num>
                    <td>
                      <Faint>{s.note}</Faint>
                    </td>
                  </tr>
                ))}
              </Table>

              <Row gap={4} wrap align="baseline">
                <Faint>walking order</Faint>
                <Soft>{survey.sequenced.toLocaleString()} sequenced</Soft>
                <Faint>{survey.zeroed} say 0</Faint>
                <Faint>{survey.disagree.toLocaleString()} disagree with WMS Bin Sequence</Faint>
              </Row>

              <Row gap={4} wrap align="baseline">
                <Soft>
                  {loaded.sites_created} sites created, {loaded.sites_matched} matched
                </Soft>
                <Spacer />
                <Soft>{loaded.bins_created.toLocaleString()} created</Soft>
                <Soft>{loaded.bins_corrected.toLocaleString()} corrected</Soft>
                <Faint>{loaded.bins_left_out.toLocaleString()} left out</Faint>
              </Row>
            </Stack>
          </FaceWell>
        </Face>
      </Panel>
    </Stack>
  );
}

function ItemsReport({ report, applied }: { report: ItemImportReport; applied: boolean }) {
  const { survey, loaded } = report;
  return (
    <Panel elevation={applied ? "lifted" : "raised"} frame="bezel">
      <Face pad={false}>
        <Band count={survey.items}>{applied ? "Loaded" : "Would load"}</Band>
        <FaceWell>
          <Stack gap={3}>
            <Row gap={4} wrap align="baseline">
              <Soft>{loaded.items_created.toLocaleString()} created</Soft>
              <Faint>{loaded.items_present.toLocaleString()} already on file</Faint>
            </Row>
            <Row gap={4} wrap align="baseline">
              {survey.duplicated > 0 && (
                <Faint>
                  {survey.duplicated} code{survey.duplicated === 1 ? "" : "s"} appear more than
                  once; the first wins
                </Faint>
              )}
              {survey.unnamed > 0 && (
                <Faint>{survey.unnamed} have no description and are named by their code</Faint>
              )}
            </Row>
          </Stack>
        </FaceWell>
      </Face>
    </Panel>
  );
}

export function Import({ bench }: { bench: ImportBench }) {
  const busy = bench.state.kind === "working";
  const reported = bench.state.kind === "reported" ? bench.state : null;

  return (
    <Stack gap={3}>
      <Panel elevation="raised" frame="bezel">
        <Face pad={false}>
          <Band>Bin list</Band>
          <FaceWell>
            <Stack gap={3}>
              <Row gap={3} wrap align="end">
                <Chooser
                  label="What this file is"
                  value={bench.which}
                  onChange={(v) => bench.pick(v as "bins" | "items")}
                  options={[
                    { value: "bins", label: "Bin list" },
                    { value: "items", label: "Item master" },
                  ]}
                />
              </Row>

              <Row gap={3} wrap align="center">
                <label className={styles.file}>
                  <span className={styles.hidden}>Choose a bin list CSV</span>
                  <span aria-hidden="true">{bench.file ? "Change file" : "Choose file"}</span>
                  <input
                    type="file"
                    accept=".csv,text/csv"
                    disabled={busy}
                    onChange={(e) => {
                      const f = e.currentTarget.files?.[0] ?? null;
                      e.currentTarget.value = "";
                      bench.choose(f);
                    }}
                  />
                </label>
                {bench.file ? (
                  <Code>{bench.file.name}</Code>
                ) : (
                  <Faint>Download as CSV.</Faint>
                )}
              </Row>

              {bench.which === "bins" && (
              <Row gap={3} wrap align="end">
                <Field
                  label="Assume kind"
                  numeric={false}
                  value={bench.options.assumeKind}
                  onChange={(v) => bench.set("assumeKind", v)}
                />
                <Faint>Bins with no type are skipped unless this is set.</Faint>
              </Row>
              )}

              <Row gap={3} wrap align="center">
                <Key
                  size="small"
                  disabled={busy || !bench.file}
                  onClick={() => void bench.run(false)}
                >
                  Dry run
                </Key>
                <Key
                  live
                  disabled={busy || !reported || reported.applied}
                  onClick={() => void bench.run(true)}
                >
                  Apply
                </Key>
                <Spacer />
                {busy && (
                  <Faint>
                    {bench.state.kind === "working" && bench.state.what === "apply"
                      ? "Loading…"
                      : "Reading…"}
                  </Faint>
                )}
              </Row>
            </Stack>
          </FaceWell>
        </Face>
      </Panel>

      {bench.state.kind === "failed" && (
        <Panel elevation="raised" frame="bezel">
          <Notice onDismiss={bench.dismiss}>{bench.state.message}</Notice>
        </Panel>
      )}

      {reported &&
        (reported.which === "items" ? (
          <ItemsReport report={reported.report} applied={reported.applied} />
        ) : (
          <Report report={reported.report} applied={reported.applied} />
        ))}
    </Stack>
  );
}

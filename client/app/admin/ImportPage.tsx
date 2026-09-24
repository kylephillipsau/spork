import { FileSpreadsheet, FlaskConical, Upload } from "lucide-react";

import { Badge, Button, Card, DataTable, PageHeader, Select, TextField, cx, type Column } from "@ui/index";
import type { ImportReport, ItemImportReport, SiteSurvey } from "@domain/types";
import { Faint } from "@app/common/cells";

import type { ImportBench, Which } from "./useImport";
import { Alert } from "./Alert";
import s from "./settings.module.css";
import imp from "./import-page.module.css";

/**
 * Loading NetSuite exports (D158, D171). Always a dry run first: it is the
 * load rolled back, so what it reports is what applying does.
 */
export function ImportPage({ bench }: { bench: ImportBench }) {
  const st = bench.state;
  const busy = st.kind === "working";
  const reported = st.kind === "reported" ? st : null;

  return (
    <div className={s.page}>
      <PageHeader title="Import" description="Load exports from NetSuite. Check the file with a dry run, then apply it." />

      <Card title="File">
        <div className={s.stack}>
          <div className={s.formRow}>
            <div className={s.narrow} style={{ flexBasis: 200 }}>
              <Select
                label="What this file is"
                value={bench.which}
                onValueChange={(v) => bench.pick(v as Which)}
                options={[
                  { value: "bins", label: "Bin list" },
                  { value: "items", label: "Item master" },
                ]}
              />
            </div>
            <div className={imp.picker}>
              <span className={imp.pickerLabel}>CSV file</span>
              <div className={imp.pickerRow}>
                <label className={cx(imp.choose, busy && imp.disabled)}>
                  <FileSpreadsheet aria-hidden />
                  <span>{bench.file ? "Change file" : "Choose file"}</span>
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
                <span className={bench.file ? imp.fileName : s.muted}>
                  {bench.file ? bench.file.name : "No file chosen. Export it from NetSuite as CSV."}
                </span>
              </div>
            </div>
          </div>

          {bench.which === "bins" && (
            <div className={s.formRow}>
              <div className={s.grow} style={{ maxWidth: 360 }}>
                <TextField
                  label="Type for bins without one"
                  placeholder="e.g. Pick"
                  hint="Bins with no type in the file are skipped unless this is set."
                  value={bench.options.assumeKind}
                  onChange={(e) => bench.set("assumeKind", e.target.value)}
                  disabled={busy}
                />
              </div>
            </div>
          )}

          <div className={imp.actions}>
            <Button
              icon={<FlaskConical />}
              loading={busy && st.kind === "working" && st.what === "dry"}
              disabled={busy || !bench.file}
              onClick={() => void bench.run(false)}
            >
              Dry run
            </Button>
            <Button
              variant="primary"
              icon={<Upload />}
              loading={busy && st.kind === "working" && st.what === "apply"}
              disabled={busy || !reported || reported.applied}
              onClick={() => void bench.run(true)}
            >
              Apply
            </Button>
            {!reported && bench.file && !busy && <span className={s.muted}>Run a dry run to check the file before applying it.</span>}
          </div>
        </div>
      </Card>

      {st.kind === "failed" && (
        <Alert tone="danger" onDismiss={bench.dismiss}>
          {st.message}
        </Alert>
      )}

      {reported &&
        (reported.which === "items" ? (
          <ItemsResult report={reported.report} applied={reported.applied} />
        ) : (
          <BinsResult report={reported.report} applied={reported.applied} />
        ))}
    </div>
  );
}

function ResultTitle({ applied, what }: { applied: boolean; what: string }) {
  return (
    <span className={imp.resultTitle}>
      {applied ? `${what} imported` : `Dry run: ${what.toLowerCase()}`}
      <Badge tone={applied ? "success" : "info"} dot>
        {applied ? "Applied" : "Nothing written yet"}
      </Badge>
    </span>
  );
}

function Stat({ label, value, tone }: { label: string; value: number; tone?: "muted" | undefined }) {
  return (
    <div className={imp.stat}>
      <span className={imp.statLabel}>{label}</span>
      <span className={cx(imp.statValue, tone === "muted" && imp.statMuted)}>{value.toLocaleString()}</span>
    </div>
  );
}

const SITE_COLUMNS: Column<SiteSurvey>[] = [
  {
    key: "warehouse",
    header: "Warehouse",
    cell: (x) => (
      <>
        {x.warehouse} {x.skipped && <Badge>Skipped</Badge>}
      </>
    ),
    grow: true,
  },
  { key: "bins", header: "Bins", cell: (x) => x.bins.toLocaleString(), align: "right", width: "90px" },
  { key: "typed", header: "Typed", cell: (x) => x.typed.toLocaleString(), align: "right", width: "90px" },
  { key: "untyped", header: "Untyped", cell: (x) => (x.untyped ? x.untyped.toLocaleString() : <Faint>0</Faint>), align: "right", width: "90px" },
  { key: "note", header: "Clock", cell: (x) => <Faint>{x.note}</Faint>, width: "220px" },
];

function BinsResult({ report, applied }: { report: ImportReport; applied: boolean }) {
  const { survey, loaded } = report;
  return (
    <Card title={<ResultTitle applied={applied} what="Bin list" />} padded={false}>
      <div className={imp.stats}>
        <Stat label="Bins in file" value={survey.bins} />
        <Stat label="Bins created" value={loaded.bins_created} />
        <Stat label="Bins corrected" value={loaded.bins_corrected} />
        <Stat label="Left out" value={loaded.bins_left_out} tone="muted" />
        <Stat label="Sites created" value={loaded.sites_created} />
        <Stat label="Sites matched" value={loaded.sites_matched} tone="muted" />
      </div>
      <DataTable aria-label="Bins by warehouse" columns={SITE_COLUMNS} rows={survey.sites} rowKey={(x) => x.warehouse} />
      <p className={imp.note}>
        Walk order: {survey.sequenced.toLocaleString()} bins have a position
        {survey.zeroed ? `, ${survey.zeroed} are 0` : ""}
        {survey.disagree ? `, ${survey.disagree.toLocaleString()} disagree with the WMS bin sequence` : ""}.
        {report.arrival?.replay ? " This exact file was loaded before." : ""}
      </p>
    </Card>
  );
}

function ItemsResult({ report, applied }: { report: ItemImportReport; applied: boolean }) {
  const { survey, loaded } = report;
  const notes = [
    survey.duplicated > 0 ? `${survey.duplicated} codes appear more than once; the first wins.` : null,
    survey.unnamed > 0 ? `${survey.unnamed} have no description and are named by their code.` : null,
    report.arrival?.replay ? "This exact file was loaded before." : null,
  ].filter(Boolean);
  return (
    <Card title={<ResultTitle applied={applied} what="Item master" />} padded={false}>
      <div className={imp.stats}>
        <Stat label="Items in file" value={survey.items} />
        <Stat label="Created" value={loaded.items_created} />
        <Stat label="Already on file" value={loaded.items_present} tone="muted" />
      </div>
      {notes.length > 0 && <p className={imp.note}>{notes.join(" ")}</p>}
    </Card>
  );
}

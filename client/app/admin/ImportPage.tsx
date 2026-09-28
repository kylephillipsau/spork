import { FileSpreadsheet, FlaskConical, Upload } from "lucide-react";

import { Alert, Badge, Button, Card, cx, DataTable, Page, PageHeader, Select, Stat, StatGrid, TextField, type Column } from "@ui/index";
import type { FloorImportReport, ImportReport, ItemImportReport, RackImportReport, SitePlaced, SiteSurvey } from "@domain/types";
import { Faint } from "@app/common/cells";

import type { ImportBench, Which } from "./useImport";
import s from "./settings.module.css";
import imp from "./import-page.module.css";

/**
 * Loading NetSuite exports (D158, D171) and the warehouse's layout (D173).
 * Always a dry run first: it is the load rolled back, so what it reports is
 * what applying does.
 */

/** What the empty file picker says, per kind of file. */
const WHERE_FROM: Record<Which, string> = {
  bins: "No file chosen. Export it from NetSuite as CSV.",
  items: "No file chosen. Export it from NetSuite as CSV.",
  racks: "No file chosen. One row per rack, as CSV.",
  floor: "No file chosen. One row per area, as CSV.",
};
export function ImportPage({ bench }: { bench: ImportBench }) {
  const st = bench.state;
  const busy = st.kind === "working";
  const reported = st.kind === "reported" ? st : null;

  return (
    <Page>
      <PageHeader title="Import" description="Load exports from NetSuite, and the warehouse layout. Check the file with a dry run, then apply it." />

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
                  { value: "racks", label: "Racks" },
                  { value: "floor", label: "Floor areas" },
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
                  {bench.file ? bench.file.name : WHERE_FROM[bench.which]}
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

      {reported?.which === "items" && <ItemsResult report={reported.report} applied={reported.applied} />}
      {reported?.which === "bins" && <BinsResult report={reported.report} applied={reported.applied} />}
      {reported?.which === "racks" && <RacksResult report={reported.report} applied={reported.applied} />}
      {reported?.which === "floor" && <FloorResult report={reported.report} applied={reported.applied} />}
    </Page>
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
      <StatGrid>
        <Stat label="Bins in file" value={survey.bins.toLocaleString()} />
        <Stat label="Bins created" value={loaded.bins_created.toLocaleString()} />
        <Stat label="Bins corrected" value={loaded.bins_corrected.toLocaleString()} />
        <Stat label="Left out" value={loaded.bins_left_out.toLocaleString()} tone="muted" />
        <Stat label="Sites created" value={loaded.sites_created.toLocaleString()} />
        <Stat label="Sites matched" value={loaded.sites_matched.toLocaleString()} tone="muted" />
      </StatGrid>
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
      <StatGrid>
        <Stat label="Items in file" value={survey.items.toLocaleString()} />
        <Stat label="Created" value={loaded.items_created.toLocaleString()} />
        <Stat label="Already on file" value={loaded.items_present.toLocaleString()} tone="muted" />
      </StatGrid>
      {notes.length > 0 && <p className={imp.note}>{notes.join(" ")}</p>}
    </Card>
  );
}

const count = (n: number) => (n ? n.toLocaleString() : <Faint>0</Faint>);

const PLACED_COLUMNS: Column<SitePlaced>[] = [
  { key: "site", header: "Site", cell: (x) => x.site, grow: true },
  { key: "racks", header: "Racks", cell: (x) => count(x.racks), align: "right", width: "80px" },
  { key: "slots", header: "Slots", cell: (x) => count(x.slots), align: "right", width: "90px" },
  { key: "placed", header: "Placed", cell: (x) => count(x.bins_placed), align: "right", width: "90px" },
  { key: "moved", header: "Moved", cell: (x) => count(x.bins_moved), align: "right", width: "90px" },
  { key: "kept", header: "Measured, kept", cell: (x) => count(x.bins_kept_measured), align: "right", width: "130px" },
  { key: "unplaced", header: "Unplaced", cell: (x) => count(x.bins_unplaced), align: "right", width: "100px" },
];

/** A few codes and how many more, so a long list reads as a number. */
function sample(codes: string[], total: number): string {
  const more = total - codes.length;
  return codes.join(", ") + (more > 0 ? ` and ${more.toLocaleString()} more` : "");
}

function RacksResult({ report, applied }: { report: RackImportReport; applied: boolean }) {
  const { loaded } = report;
  const notes = loaded.sites.flatMap((x) => [
    x.slots_without_bin > 0
      ? `${x.site}: ${x.slots_without_bin.toLocaleString()} slots name no bin on file (${sample(x.slots_without_bin_sample, x.slots_without_bin)}). Racks place bins; they do not create them.`
      : null,
    x.bins_uncovered > 0
      ? `${x.site}: ${x.bins_uncovered.toLocaleString()} bins are in a racked aisle but no rack names them (${sample(x.bins_uncovered_sample, x.bins_uncovered)}). Check their codes against the template.`
      : null,
  ]).filter((n): n is string => n !== null);
  return (
    <Card title={<ResultTitle applied={applied} what="Racks" />} padded={false}>
      <StatGrid>
        <Stat label="Racks created" value={loaded.racks_created.toLocaleString()} />
        <Stat label="Racks changed" value={loaded.racks_changed.toLocaleString()} />
        <Stat label="Unchanged" value={loaded.racks_unchanged.toLocaleString()} tone="muted" />
      </StatGrid>
      <DataTable aria-label="Bins by site" columns={PLACED_COLUMNS} rows={loaded.sites} rowKey={(x) => x.site} />
      {notes.map((n) => (
        <p key={n} className={imp.note}>
          {n}
        </p>
      ))}
      {report.arrival?.replay && <p className={imp.note}>This exact file was loaded before.</p>}
    </Card>
  );
}

function FloorResult({ report, applied }: { report: FloorImportReport; applied: boolean }) {
  const { loaded } = report;
  return (
    <Card title={<ResultTitle applied={applied} what="Floor areas" />} padded={false}>
      <StatGrid>
        <Stat label="Created" value={loaded.areas_created.toLocaleString()} />
        <Stat label="Changed" value={loaded.areas_changed.toLocaleString()} />
        <Stat label="Unchanged" value={loaded.areas_unchanged.toLocaleString()} tone="muted" />
      </StatGrid>
      {report.arrival?.replay && <p className={imp.note}>This exact file was loaded before.</p>}
    </Card>
  );
}

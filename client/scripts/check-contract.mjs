#!/usr/bin/env node
/**
 * THE HAND-MIRRORED TYPES, WATCHED (D113).
 *
 * D113 says types are generated from the API, because a hand-kept copy of a
 * Rust struct is a copy that drifts. `domain/types.ts` is currently that copy,
 * and this is the cheap half of the guarantee until a generator exists: it
 * reads `crates/server/src/bench.rs` and fails when the two disagree about
 * which fields exist.
 *
 * It does not check types, only names — a `#[derive(Serialize)]` field added on
 * the server and forgotten here is the drift that actually happens, and a
 * renamed one is the drift that actually breaks. Both are name-shaped.
 *
 * `BenchScreen` is flattened over `Bench` on the wire (`#[serde(flatten)]`), so
 * the TypeScript interface is compared against the union of the two.
 */

import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const TS = join(here, "..", "domain", "types.ts");
/**
 * **Every module, found rather than listed.**
 *
 * This was eleven paths typed out, with a comment saying that adding a module
 * here was part of adding the module. It was not: `tokens.rs` and
 * `importing.rs` arrived with six wire types between them and neither was
 * added, so the gate reported "client and server agree" about types it had
 * never read. A list you have to remember is a list that is eventually wrong,
 * and the failure is silent in the direction that matters — it passes.
 */
function rustFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory()
      ? rustFiles(join(dir, e.name))
      : e.name.endsWith(".rs")
        ? [join(dir, e.name)]
        : [],
  );
}

// **Recursive, and it was not.** The first version of this globbed one
// directory deep, which was every module on the day it was written. `picking`
// and `importing` became directories a week later and took their wire types out
// of the gate's sight with them — the same silence the hardcoded array had,
// arrived at from the other end. A directory read that stops at the first
// directory is a list again.
const RS = rustFiles(join(here, "..", "..", "crates", "server", "src"));

/**
 * Interfaces in types.ts, as name → field set.
 *
 * **`extends` is resolved rather than ignored.** A response that flattens a row
 * over one extra field is one field of its own plus everything the row has, and
 * the honest TypeScript for that is `extends`. Read without resolving it, this
 * reported the interface as missing entirely; the alternative — spelling out
 * twenty-two inherited fields at the call site — is the duplication this whole
 * check exists to catch, written by hand and guaranteed to drift.
 */
function interfaces(source) {
  // **Block comments go first, as they do on the Rust side.** The body regex
  // below stops at the first `}`, and a doc comment containing one — a path
  // like `/images/{digest}`, a code sample — truncates the interface at that
  // point, so its remaining fields are reported as missing from a file that
  // has them. `structs()` has always stripped comments before matching; this
  // half did not, and the asymmetry took a real interface to surface.
  source = source.replace(/\/\*[\s\S]*?\*\//g, "");
  const own = new Map();
  const parents = new Map();
  // `(?:<[^>]*>)?` because a generic interface is still an interface, and
  // `CeremonyBegun<Options>` was skipped entirely — silently, which is the only
  // way this file ever goes wrong.
  const re = /export interface (\w+)(?:<[^>]*>)?(?:\s+extends\s+([\w,\s]+?))?\s*\{([^}]*)\}/g;
  let m;
  while ((m = re.exec(source)) !== null) {
    const fields = new Set();
    for (const line of m[3].split("\n")) {
      // **`readonly` is a modifier and was being read as the field name**, so
      // every field of an interface written that way was dropped and the
      // interface looked empty. Six of them shipped like that.
      const field = /^\s*(?:readonly\s+)?(\w+)\s*[?]?\s*:/.exec(line.replace(/\/\/.*$/, ""));
      if (field) fields.add(field[1]);
    }
    own.set(m[1], fields);
    parents.set(m[1], m[2] ? m[2].split(",").map((p) => p.trim()).filter(Boolean) : []);
  }

  // Flatten the inheritance. Depth-limited rather than cycle-detected: a cycle
  // is a TypeScript error the typecheck gate already refuses, and a bound is
  // shorter than a visited set.
  const out = new Map();
  for (const [name] of own) {
    const fields = new Set();
    const walk = (n, depth) => {
      if (depth > 8 || !own.has(n)) return;
      for (const f of own.get(n)) fields.add(f);
      for (const p of parents.get(n) ?? []) walk(p, depth + 1);
    };
    walk(name, 0);
    out.set(name, fields);
  }
  return out;
}

/** `pub struct`s across the server, as name → field set, comments stripped. */
function structs(source) {
  const clean = source.replace(/\/\/\/?.*$/gm, "").replace(/\/\*[\s\S]*?\*\//g, "");
  const out = new Map();
// The same generic tolerance the TypeScript side needed, for the same
  // reason: `pub struct Ceremony<T>` was skipped without a word.
  const re = /pub struct (\w+)(?:<[^>]*>)?\s*\{([^}]*)\}/g;
  let m;
  while ((m = re.exec(clean)) !== null) {
    const fields = new Set();
    // A flattened field is a shape rather than a field: `#[serde(flatten)] pub
    // bench: Bench` puts Bench's own fields on the wire and `bench` itself
    // nowhere. The attribute sits on its own line, so the skip has to carry to
    // the next one — skipping only the attribute line reports `bench` as a
    // wire field that the client is missing, which it is not.
    let flattened = false;
    for (const line of m[2].split("\n")) {
      if (/serde\(flatten\)/.test(line)) {
        flattened = true;
        continue;
      }
      const field = /^\s*pub\s+(\w+)\s*:/.exec(line);
      if (!field) continue;
      if (flattened) {
        flattened = false;
        continue;
      }
      fields.add(field[1]);
    }
    out.set(m[1], fields);
  }
  return out;
}

const ts = interfaces(readFileSync(TS, "utf8"));
const rs = new Map(RS.flatMap((f) => [...structs(readFileSync(f, "utf8"))]));

/** TypeScript interface → the Rust struct(s) it mirrors. */
const PAIRS = [
  ["Cell", ["Cell"]],
  ["BenchLine", ["BenchLine"]],
  ["Preset", ["Preset"]],
  ["StatedSize", ["StatedSize"]],
  ["PackedRow", ["PackedRow"]],
  ["CartonSummary", ["CartonSummary"]],
  ["WeightBaseline", ["WeightBaseline"]],
  // flattened on the wire, like BenchScreen
  ["ExpectedWeight", ["ExpectedWeight", "WeightBaseline"]],
  // flattened on the wire
  ["BenchScreen", ["BenchScreen", "Bench"]],
  // despatch
  ["WaitingCarton", ["WaitingCarton"]],
  ["WaitingJob", ["WaitingJob"]],
  ["BookedCarton", ["BookedCarton"]],
  ["CartonLine", ["CartonLine"]],
  ["BookedConsignment", ["BookedConsignment"]],
  ["GoneConsignment", ["GoneConsignment"]],
  ["Carrier", ["Carrier"]],
  ["CarrierService", ["CarrierService"]],
  ["Provider", ["Provider"]],
  ["DespatchScreen", ["DespatchScreen"]],
  // capture
  ["CaptureSubject", ["CaptureSubject"]],
  ["CaptureScreen", ["CaptureScreen"]],
  ["RecordObservationResponse", ["RecordObservationResponse"]],
  // binding a barcode (D164)
  ["BoundBarcode", ["BoundBarcode"]],
  ["BindBarcodeRequest", ["BindBarcodeRequest"]],
  // the locator
  ["Subject", ["Subject"]],
  ["Resolution", ["Resolution"]],
  // findings
  ["DiscrepancyRow", ["DiscrepancyRow"]],
  // flattened on the wire, like BenchScreen
  ["FindingActionResponse", ["InvestigateDiscrepancyResponse", "DiscrepancyRow"]],
  // **Paired late, and they were on the wire the whole time.** Ten of these
  // had a Rust struct of the same name sitting in a module the gate already
  // read; they were simply never named here, so nothing compared them. The
  // list below is the gate's second allow-list, and `everyInterfaceIsPaired`
  // at the foot of this file is what stops it silently shrinking again.
  ["RecordEvidenceResponse", ["RecordEvidenceResponse"]],
  ["SignOnRequest", ["SignOnRequest"]],
  ["SignOnResponse", ["SignOnResponse"]],
  ["TenantChoice", ["TenantChoice"]],
  ["PackJob", ["PackJob"]],
  ["FulfilmentSummary", ["FulfilmentSummary"]],
  ["OrderMatch", ["OrderMatch"]],
  ["WorkWaiting", ["WorkWaiting"]],
  ["SiteRow", ["SiteRow"]],
  ["ChooseSiteRequest", ["ChooseSiteRequest"]],
  // Named differently on each side, which is why they slipped: the server says
  // what the row is, the client says what the thing is.
  // The import report (D158): the survey comes from `importing`, the envelope
  // from the handler.
  ["SiteSurvey", ["SiteSurvey"]],
  ["Survey", ["Survey"]],
  ["Loaded", ["Loaded"]],
  ["FileArrival", ["FileArrival"]],
  ["ImportReport", ["ImportReport"]],
  ["ItemSurvey", ["ItemSurvey"]],
  ["ItemsLoaded", ["ItemsLoaded"]],
  ["ItemImportReport", ["ItemImportReport"]],
  ["WarehouseRows", ["WarehouseRows"]],
  ["StockSurvey", ["StockSurvey"]],
  ["StockLoaded", ["StockLoaded"]],
  ["StockImportReport", ["StockImportReport"]],
  ["Organisation", ["Organisation"]],
  ["WorkspaceSite", ["WorkspaceSite"]],
  ["Workspace", ["Workspace"]],
  ["ApiToken", ["TokenRow"]],
  ["MintTokenRequest", ["MintRequest"]],
  ["MintedApiToken", ["Minted"]],
  ["Passkey", ["PasskeyRow"]],
  ["CeremonyBegun", ["Ceremony"]],
  // weighing
  ["ToWeigh", ["ToWeigh"]],
  ["WeighingRecorded", ["WeighingRecorded"]],
  // picking
  ["Picture", ["Picture"]],
  ["PickLine", ["PickLine"]],
  ["PickListScreen", ["PickListScreen"]],
  ["Progress", ["Progress"]],
  // flattened on the wire, like BenchScreen
  ["ProjectionProgress", ["ProjectionProgress", "Progress"]],
  ["RecordPickResponse", ["RecordPickResponse"]],
  // receiving
  ["PackLevel", ["PackLevel"]],
  ["Owner", ["Owner"]],
  ["ExpectedLine", ["ExpectedLine"]],
  ["ReceivingScreen", ["ReceivingScreen"]],
  ["RecordReceiptResponse", ["RecordReceiptResponse"]],
  // put-away
  ["Home", ["Home"]],
  ["PutawayCell", ["PutawayCell"]],
  ["PutawayScreen", ["PutawayScreen"]],
  // setting a deployment up
  ["SetupStatus", ["SetupStatus"]],
  ["SetupRequest", ["SetupRequest"]],
  ["SetupDone", ["SetupDone"]],
  // changing your own password
  ["ChangePasswordRequest", ["ChangePasswordRequest"]],
  ["PasswordChanged", ["PasswordChanged"]],
  ["CurrentSession", ["CurrentSession"]],
  // written by the API rather than read from a screen module
  ["CarrierLine", ["CarrierLine"]],
  ["ConsignmentResponse", ["ConsignmentResponse"]],
];

const problems = [];

for (const [name, sources] of PAIRS) {
  const mirrored = ts.get(name);
  if (!mirrored) {
    problems.push(`domain/types.ts has no interface ${name}`);
    continue;
  }

  const expected = new Set();
  for (const source of sources) {
    const fields = rs.get(source);
    if (!fields) {
      problems.push(`no pub struct ${source} anywhere in crates/server/src`);
      continue;
    }
    for (const field of fields) expected.add(field);
  }

  for (const field of expected) {
    if (!mirrored.has(field)) {
      problems.push(`${name}.${field} is on the wire but missing from domain/types.ts`);
    }
  }
  for (const field of mirrored) {
    if (!expected.has(field)) {
      problems.push(`${name}.${field} is in domain/types.ts but no longer on the wire`);
    }
  }
}

/**
 * **Every interface is paired, or the pairing list has quietly shrunk.**
 *
 * The two failures this file has had are the same failure: an allow-list that
 * has to be remembered. The module list was one, and `tokens.rs` was never
 * added to it. `PAIRS` is the other, and fifteen interfaces — sign-on, the
 * pack queue, the work counts, the site chooser — were never named in it, so
 * nothing ever compared them while the gate reported that everything agreed.
 *
 * The module list is gone, replaced by a directory read. This is what does the
 * same job for `PAIRS`: a wire type added to `types.ts` and not paired is a
 * failure, not a silence. A type that genuinely has no server struct behind it
 * belongs in `CLIENT_ONLY`, named, with a reason — there are none today, and
 * that is worth keeping true.
 */
const CLIENT_ONLY = new Set([]);
const paired = new Set(PAIRS.map(([name]) => name));
for (const name of ts.keys()) {
  if (!paired.has(name) && !CLIENT_ONLY.has(name)) {
    problems.push(
      `${name} is in domain/types.ts and paired with nothing — add it to PAIRS, ` +
        "or to CLIENT_ONLY if no server struct backs it",
    );
  }
}

if (problems.length) {
  console.error("the client's types and the server's disagree:\n");
  for (const p of problems) console.error(`  ✗ ${p}`);
  console.error(
    "\nUntil types are generated (D113), domain/types.ts is a hand-kept copy\n" +
      "and this is what keeps it honest. Update it to match the server.",
  );
  process.exit(1);
}

const total = PAIRS.reduce((n, [name]) => n + (ts.get(name)?.size ?? 0), 0);
console.log(`contract ok — ${PAIRS.length} types, ${total} fields, client and server agree`);

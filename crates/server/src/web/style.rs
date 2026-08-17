//! The stylesheet, served from the binary.
//!
//! Embedded rather than sat on disk because one binary with no asset pipeline is
//! the reason server-rendered pages were chosen at all.
//!
//! # The constraint that shapes it
//!
//! **This is not a desk application.** An operator is standing at a pack bench,
//! possibly gloved, reading at arm's length under warehouse lighting, several
//! hundred times a day. So the base size is 17px rather than 14, targets are at
//! least 44px, and contrast is high everywhere. Every other choice below is
//! downstream of that.
//!
//! # The palette is the business's own materials
//!
//! Pale concrete for the ground, because that is the floor. **Nitrile** — the
//! blue-violet of the gloves this company sells — as the accent. **Kraft** for
//! the carton motif, because a carton is cardboard.
//!
//! And **amber is reserved for findings**. Nothing else in the interface may use
//! it. Architecture: *"Disagreement is the most valuable thing the system
//! produces."* One colour, one meaning, so a screen with amber on it is a screen
//! with something to chase.
//!
//! # Codes look like codes
//!
//! Every string a human reads off a label or says down a phone — an item code, a
//! lot code, an SSCC, an order reference — is monospace with tabular figures.
//! Prose is not. The rule encodes something true about which strings are
//! identifiers, and it holds on every screen.

pub const CSS: &str = r#"
:root {
  --concrete: #EDEEEA;
  --slab: #FFFFFF;
  --ink: #14161A;
  --ink-soft: #4A4F57;
  --ink-faint: #7B818A;
  --rule: #D3D6D0;
  --rule-soft: #E3E5E0;
  --nitrile: #3D3A8C;
  --nitrile-soft: #E7E6F2;
  --kraft: #A9784B;
  --kraft-soft: #F0E4D6;
  --amber: #A64B00;
  --amber-soft: #FBE9D8;
  --good: #2F5D45;
  --mono: ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas, monospace;
  --sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
}

@media (prefers-color-scheme: dark) {
  :root {
    --concrete: #16181B;
    --slab: #1E2126;
    --ink: #ECEEF0;
    --ink-soft: #A9AFB7;
    --ink-faint: #767D86;
    --rule: #2E333A;
    --rule-soft: #262A30;
    --nitrile: #A6A2F0;
    --nitrile-soft: #23233F;
    --kraft: #C79B6E;
    --kraft-soft: #2C2318;
    --amber: #E08A45;
    --amber-soft: #33210F;
    --good: #7FBB9A;
  }
}

* { box-sizing: border-box; }

body {
  margin: 0;
  background: var(--concrete);
  color: var(--ink);
  font-family: var(--sans);
  /* Arm's length, not desk distance. */
  font-size: 17px;
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
}

/* Every identifier a person reads aloud or off a label. */
code, .code { font-family: var(--mono); font-variant-numeric: tabular-nums; font-size: 0.94em; }
.num { font-family: var(--mono); font-variant-numeric: tabular-nums; }

/* ---- chrome ---- */
header.bar {
  display: flex; align-items: center; gap: 1rem;
  padding: 0.7rem 1.25rem;
  background: var(--ink); color: var(--concrete);
  position: sticky; top: 0; z-index: 10;
}
header.bar .mark { font-weight: 700; letter-spacing: -0.02em; font-size: 1.05rem; }
header.bar .who { margin-left: auto; display: flex; align-items: center; gap: 0.9rem;
                  font-size: 0.9rem; color: #B9BEC4; }
header.bar a { color: var(--concrete); }
main { max-width: 62rem; margin: 0 auto; padding: 1.5rem 1.25rem 5rem; }

h1 { font-size: 1.7rem; font-weight: 700; letter-spacing: -0.02em; margin: 0 0 0.3rem; }
h2 { font-size: 1.15rem; font-weight: 650; margin: 2rem 0 0.75rem; }
.lede { color: var(--ink-soft); margin: 0 0 1.5rem; }

/* ---- forms: gloved hands ---- */
label { display: block; font-size: 0.82rem; font-weight: 600; letter-spacing: 0.04em;
        text-transform: uppercase; color: var(--ink-soft); margin-bottom: 0.35rem; }
input[type=text], input[type=email], input[type=password], input[type=search], select {
  width: 100%; min-height: 48px; padding: 0.6rem 0.8rem;
  font-family: inherit; font-size: 1.05rem; color: var(--ink);
  background: var(--slab); border: 1.5px solid var(--rule); border-radius: 3px;
}
input:focus-visible, select:focus-visible, button:focus-visible, a:focus-visible {
  outline: 3px solid var(--nitrile); outline-offset: 2px;
}
button, .button {
  min-height: 48px; padding: 0.6rem 1.4rem;
  font-family: inherit; font-size: 1.02rem; font-weight: 650;
  color: #fff; background: var(--nitrile);
  border: none; border-radius: 3px; cursor: pointer;
  display: inline-flex; align-items: center; justify-content: center; gap: 0.5rem;
  text-decoration: none;
}
button.quiet, .button.quiet { background: transparent; color: var(--nitrile);
                              border: 1.5px solid var(--rule); }
button[disabled] { opacity: 0.45; cursor: not-allowed; }
.field { margin-bottom: 1.1rem; }

/* ---- the docket: the one signature element ---- */
.docket {
  background: var(--slab); border: 1.5px solid var(--rule); border-radius: 3px;
  overflow: hidden; margin-bottom: 1.25rem;
}
.docket > .band {
  display: flex; align-items: baseline; gap: 0.9rem; flex-wrap: wrap;
  padding: 0.7rem 1rem;
  background: var(--kraft-soft); border-bottom: 1.5px solid var(--rule);
}
.docket > .band .seq {
  font-family: var(--mono); font-size: 1.25rem; font-weight: 700; color: var(--kraft);
}
.docket > .band .of { color: var(--ink-soft); font-size: 0.9rem; }
.docket > .band .right { margin-left: auto; font-size: 0.9rem; color: var(--ink-soft); }
.docket .body { padding: 0.25rem 1rem 0.75rem; }

/* Ruled lines, like a real docket. */
table { width: 100%; border-collapse: collapse; }
th { text-align: left; font-size: 0.74rem; letter-spacing: 0.07em; text-transform: uppercase;
     color: var(--ink-faint); font-weight: 600; padding: 0.6rem 0.6rem 0.4rem 0;
     border-bottom: 1.5px solid var(--ink); }
td { padding: 0.65rem 0.6rem 0.65rem 0; border-bottom: 1px solid var(--rule-soft);
     vertical-align: baseline; }
tr:last-child td { border-bottom: none; }
td.r, th.r { text-align: right; padding-right: 0; }

/* Weight and size, stamped. */
.stamp { display: flex; flex-wrap: wrap; gap: 0 2rem; padding: 0.7rem 1rem;
         border-top: 1.5px dashed var(--rule); background: var(--slab); }
.stamp div { display: flex; flex-direction: column; }
.stamp .k { font-size: 0.72rem; letter-spacing: 0.07em; text-transform: uppercase;
            color: var(--ink-faint); }
.stamp .v { font-family: var(--mono); font-variant-numeric: tabular-nums;
            font-size: 1.15rem; font-weight: 650; }

/* ---- findings: the only amber in the interface ---- */
.finding { border-left: 4px solid var(--amber); background: var(--amber-soft);
           padding: 0.7rem 1rem; margin: 0 0 0.8rem; color: var(--ink); }
.finding .kind { font-family: var(--mono); font-size: 0.78rem; letter-spacing: 0.05em;
                 text-transform: uppercase; color: var(--amber); font-weight: 700; }

.note { border-left: 4px solid var(--rule); background: var(--slab);
        padding: 0.7rem 1rem; color: var(--ink-soft); }
.pill { display: inline-block; font-family: var(--mono); font-size: 0.74rem;
        letter-spacing: 0.05em; text-transform: uppercase; padding: 0.14rem 0.45rem;
        border-radius: 2px; background: var(--nitrile-soft); color: var(--nitrile); }
.pill.ready { background: #DEF0E6; color: var(--good); }
@media (prefers-color-scheme: dark) { .pill.ready { background: #16281F; } }

.rows { list-style: none; padding: 0; margin: 0; }
.rows li { border-bottom: 1px solid var(--rule-soft); }
.rows a { display: flex; align-items: center; gap: 1rem; padding: 0.9rem 0.2rem;
          text-decoration: none; color: inherit; min-height: 48px; }
.rows a:hover { background: var(--nitrile-soft); }
.rows .grow { flex: 1; }
.muted { color: var(--ink-soft); font-size: 0.9rem; }
.err { color: var(--amber); font-weight: 600; margin: 0 0 1rem; min-height: 1.4rem; }

/* ---- print: stage 5 is an A4 sheet folded onto a pallet ---- */
@media print {
  :root { --concrete: #fff; --slab: #fff; --ink: #000; --rule: #000; --rule-soft: #999; }
  header.bar, .noprint { display: none !important; }
  body { font-size: 11pt; }
  main { max-width: none; padding: 0; }
  .docket { border: 1.5pt solid #000; break-inside: avoid; page-break-inside: avoid; }
  .docket > .band { background: #fff; border-bottom: 1.5pt solid #000; }
  a { text-decoration: none; color: #000; }
  @page { size: A4; margin: 14mm; }
}

@media (prefers-reduced-motion: reduce) { * { animation: none !important; transition: none !important; } }
"#;

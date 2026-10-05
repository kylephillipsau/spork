//! The weights and dimensions capture sheet, as a PDF (D217).
//!
//! **The sheet a list replaces, drawn as it was drawn.** The printed sheet
//! handed over with a list (D179) is A4 landscape in Helvetica: a title and
//! the page, a line saying how fresh NetSuite's data is, a grey band of column
//! names, then twelve-millimetre rows, fourteen to a page, every other one
//! shaded to the boxes. Each row names the item, where it is and what it is
//! sold in, and has four boxes: length, width and height in centimetres and
//! weight in kilograms. Every measure here is that sheet's, in millimetres,
//! and text is fitted as it fitted it: a column's text stops two millimetres
//! short of the next column; a code or a bin shrinks to 9 pt and then is cut
//! with "…"; a description wraps to two lines and the second is cut.
//!
//! **The boxes hold what Spork has recorded** for the unit the item is sold
//! in, to one decimal, and are empty where nothing is: the sheet is the
//! record, and the work left on it.
//!
//! Written by hand rather than with a library: the two fonts are the ones
//! every reader has, so nothing is embedded, and the file is a few objects.

use crate::pdf_metrics::{HELVETICA, HELVETICA_BOLD};

/// Points in a millimetre.
const MM: f64 = 72.0 / 25.4;
const PAGE_W: f64 = 841.889_8;
const PAGE_H: f64 = 595.275_6;
/// Rows on a page.
pub const PER_PAGE: usize = 14;

/// Where each column starts, and where its text must stop, in millimetres.
const SEQ: (f64, f64) = (9.0, 20.0);
const BIN: (f64, f64) = (20.0, 42.0);
const CODE: (f64, f64) = (42.0, 76.0);
const DESCRIPTION: (f64, f64) = (76.0, 138.0);
const SUPPLIER: (f64, f64) = (138.0, 172.0);
const UNIT: (f64, f64) = (172.0, 187.0);
const SOH: (f64, f64) = (187.0, 200.0);
/// The four boxes: where the column's name starts, the box's left and width.
const BOXES: [(&str, f64, f64, f64); 4] = [
    ("Length cm", 200.0, 200.5, 19.0),
    ("Width cm", 222.0, 222.5, 19.0),
    ("Height cm", 244.0, 244.5, 19.0),
    ("Weight kg", 266.0, 266.5, 21.0),
];
const LEFT: f64 = 8.0;
const RIGHT: f64 = 289.0;
/// The column names' band, and the first row's top.
const BAND_BOTTOM: f64 = 180.0;
const BAND_H: f64 = 10.0;
const ROW_H: f64 = 12.0;
/// How far a column's text stops short of the next column.
const GAP: f64 = 2.0;

/// One item on the sheet.
pub struct SheetRow {
    pub seq: usize,
    pub bin: String,
    pub code: String,
    pub description: String,
    pub supplier_part: String,
    pub unit: String,
    pub soh: String,
    /// Length, width, height (cm) and weight (kg), as written in the boxes.
    pub boxes: [Option<String>; 4],
}

pub struct Sheet {
    pub title: String,
    pub subtitle: String,
    pub rows: Vec<SheetRow>,
}

/// The sheet, as the bytes of a PDF.
pub fn render(sheet: &Sheet) -> Vec<u8> {
    let pages: Vec<&[SheetRow]> = if sheet.rows.is_empty() {
        vec![&sheet.rows[..]]
    } else {
        sheet.rows.chunks(PER_PAGE).collect()
    };
    let of = pages.len();
    let streams: Vec<Vec<u8>> = pages
        .iter()
        .enumerate()
        .map(|(n, rows)| page(sheet, rows, n + 1, of))
        .collect();
    write_pdf(&sheet.title, &streams)
}

// ---------------------------------------------------------------------------
// Text, measured
// ---------------------------------------------------------------------------

/// A string in WinAnsi, the encoding the two fonts are used in. What it has
/// no code for is a question mark.
fn winansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c as u32 {
            0x20..=0x7E | 0xA0..=0xFF => c as u32 as u8,
            _ => match c {
                '€' => 0x80,
                '‚' => 0x82,
                'ƒ' => 0x83,
                '„' => 0x84,
                '…' => 0x85,
                '†' => 0x86,
                '‡' => 0x87,
                'ˆ' => 0x88,
                '‰' => 0x89,
                'Š' => 0x8A,
                '‹' => 0x8B,
                'Œ' => 0x8C,
                'Ž' => 0x8E,
                '‘' => 0x91,
                '’' => 0x92,
                '“' => 0x93,
                '”' => 0x94,
                '•' => 0x95,
                '–' => 0x96,
                '—' => 0x97,
                '˜' => 0x98,
                '™' => 0x99,
                'š' => 0x9A,
                '›' => 0x9B,
                'œ' => 0x9C,
                'ž' => 0x9E,
                'Ÿ' => 0x9F,
                '\t' | '\n' | '\r' => b' ',
                _ => b'?',
            },
        })
        .collect()
}

const ELLIPSIS: u8 = 0x85;

fn width(text: &[u8], bold: bool, size: f64) -> f64 {
    let table = if bold { &HELVETICA_BOLD } else { &HELVETICA };
    text.iter().map(|&b| table[b as usize] as f64).sum::<f64>() * size / 1000.0
}

/// Cut to fit, ending "…", when it doesn't fit as it is.
fn cut(text: &[u8], bold: bool, size: f64, room: f64) -> Vec<u8> {
    if width(text, bold, size) <= room {
        return text.to_vec();
    }
    let mut kept = text.to_vec();
    while !kept.is_empty() && width(&[&kept[..], &[ELLIPSIS]].concat(), bold, size) > room {
        kept.pop();
    }
    while kept.last() == Some(&b' ') {
        kept.pop();
    }
    kept.push(ELLIPSIS);
    kept
}

/// A code or a bin: bold 10 pt, shrunk a quarter point at a time to 9 to
/// fit, and cut there if it still doesn't.
fn shrunk(text: &[u8], room: f64) -> (f64, Vec<u8>) {
    let mut size = 10.0;
    while size > 9.0 && width(text, true, size) > room {
        size -= 0.25;
    }
    (size, cut(text, true, size, room))
}

/// A description: 9 pt, wrapped at spaces, two lines at most, the second cut.
fn wrapped(text: &[u8], room: f64) -> Vec<Vec<u8>> {
    let size = 9.0;
    let mut lines: Vec<Vec<u8>> = vec![];
    let mut line: Vec<u8> = vec![];
    for word in text.split(|&b| b == b' ').filter(|w| !w.is_empty()) {
        let tried = if line.is_empty() {
            word.to_vec()
        } else {
            [&line[..], b" ", word].concat()
        };
        if width(&tried, false, size) <= room || line.is_empty() {
            line = tried;
        } else {
            lines.push(std::mem::take(&mut line));
            line = word.to_vec();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > 2 {
        lines.truncate(2);
        let second = lines.pop().unwrap_or_default();
        let mut kept = second;
        while !kept.is_empty() && width(&[&kept[..], &[ELLIPSIS]].concat(), false, size) > room {
            kept.pop();
        }
        while kept.last() == Some(&b' ') {
            kept.pop();
        }
        kept.push(ELLIPSIS);
        lines.push(kept);
    }
    // A word wider than the column on its own is cut where it stands.
    lines
        .into_iter()
        .map(|l| cut(&l, false, size, room))
        .collect()
}

// ---------------------------------------------------------------------------
// A page
// ---------------------------------------------------------------------------

/// A number as a PDF wants it: four places at most, no trailing zeros.
fn n(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

/// A string as a PDF literal: escaped, and anything past ASCII in octal.
fn literal(text: &[u8]) -> String {
    let mut s = String::with_capacity(text.len() + 2);
    s.push('(');
    for &b in text {
        match b {
            b'\\' | b'(' | b')' => {
                s.push('\\');
                s.push(b as char);
            }
            0x20..=0x7E => s.push(b as char),
            _ => s.push_str(&format!("\\{b:03o}")),
        }
    }
    s.push(')');
    s
}

struct Ink(String);

impl Ink {
    fn text(&mut self, font: &str, size: f64, x: f64, y: f64, text: &[u8]) {
        self.0.push_str(&format!(
            "BT /{font} {} Tf 1 0 0 1 {} {} Tm {} Tj ET\n",
            n(size),
            n(x),
            n(y),
            literal(text)
        ));
    }
    fn fill(&mut self, grey: f64, x: f64, y: f64, w: f64, h: f64) {
        self.0.push_str(&format!(
            "{} g {} {} {} {} re f 0 g\n",
            n(grey),
            n(x),
            n(y),
            n(w),
            n(h)
        ));
    }
    fn rule(&mut self, x0: f64, x1: f64, y: f64) {
        self.0.push_str(&format!(
            "0.25 w 0 G {} {} m {} {} l S\n",
            n(x0),
            n(y),
            n(x1),
            n(y)
        ));
    }
    fn boxed(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.0.push_str(&format!(
            "1 g 0.5 w 0 G {} {} {} {} re B 0 g\n",
            n(x),
            n(y),
            n(w),
            n(h)
        ));
    }
}

fn page(sheet: &Sheet, rows: &[SheetRow], at: usize, of: usize) -> Vec<u8> {
    let y = |mm: f64| mm * MM;
    let room = |col: (f64, f64)| (col.1 - col.0 - GAP) * MM;
    let mut ink = Ink(String::new());

    // The title, the page, and how fresh NetSuite's data is.
    ink.text("F2", 13.0, LEFT * MM, 559.559_1, &winansi(&sheet.title));
    let page = winansi(&format!("Page {at} of {of}"));
    ink.text(
        "F2",
        13.0,
        RIGHT * MM - width(&page, true, 13.0),
        559.559_1,
        &page,
    );
    ink.text("F1", 9.0, LEFT * MM, 542.834_6, &winansi(&sheet.subtitle));

    // The column names, on their band.
    ink.fill(
        0.8,
        LEFT * MM,
        y(BAND_BOTTOM),
        (RIGHT - LEFT) * MM,
        BAND_H * MM,
    );
    let heads = [
        ("Seq", SEQ.0),
        ("Bin", BIN.0),
        ("Item Code", CODE.0),
        ("Description", DESCRIPTION.0),
        ("Supplier Part No.", SUPPLIER.0),
        ("Unit", UNIT.0),
        ("SOH", SOH.0),
    ];
    for (name, x) in heads
        .iter()
        .map(|(n, x)| (*n, *x))
        .chain(BOXES.iter().map(|b| (b.0, b.1)))
    {
        ink.text("F2", 10.0, x * MM, 519.874, &winansi(name));
    }
    ink.rule(LEFT * MM, RIGHT * MM, y(BAND_BOTTOM));

    for (i, row) in rows.iter().enumerate() {
        let top = y(BAND_BOTTOM - ROW_H * i as f64);
        let bottom = top - ROW_H * MM;
        // Every other row shaded, as far as the boxes.
        if i % 2 == 1 {
            ink.fill(0.95, LEFT * MM, bottom, 191.0 * MM, ROW_H * MM);
        }
        let line = top - 20.409_4;
        ink.text(
            "F2",
            10.0,
            SEQ.0 * MM,
            line,
            &cut(&winansi(&row.seq.to_string()), true, 10.0, room(SEQ)),
        );
        let (size, bin) = shrunk(&winansi(&row.bin), room(BIN));
        ink.text("F2", size, BIN.0 * MM, line, &bin);
        let (size, code) = shrunk(&winansi(&row.code), room(CODE));
        ink.text("F2", size, CODE.0 * MM, line, &code);
        match &wrapped(&winansi(&row.description), room(DESCRIPTION))[..] {
            [one] => ink.text("F1", 9.0, DESCRIPTION.0 * MM, line, one),
            [first, second] => {
                ink.text("F1", 9.0, DESCRIPTION.0 * MM, top - 11.622_1, first);
                ink.text("F1", 9.0, DESCRIPTION.0 * MM, top - 21.826_8, second);
            }
            _ => {}
        }
        ink.text(
            "F1",
            9.0,
            SUPPLIER.0 * MM,
            line,
            &cut(&winansi(&row.supplier_part), false, 9.0, room(SUPPLIER)),
        );
        ink.text(
            "F1",
            9.0,
            UNIT.0 * MM,
            line,
            &cut(&winansi(&row.unit), false, 9.0, room(UNIT)),
        );
        ink.text(
            "F1",
            9.0,
            SOH.0 * MM,
            line,
            &cut(&winansi(&row.soh), false, 9.0, room(SOH)),
        );
        // The boxes, and what is recorded in them, centred.
        for ((_, _, x, w), said) in BOXES.iter().zip(&row.boxes) {
            let (bx, by, bw, bh) = (x * MM, bottom + 1.8 * MM, w * MM, 8.4 * MM);
            ink.boxed(bx, by, bw, bh);
            if let Some(said) = said {
                let text = cut(&winansi(said), false, 10.0, bw - 2.0);
                ink.text(
                    "F1",
                    10.0,
                    bx + (bw - width(&text, false, 10.0)) / 2.0,
                    by + (bh - 7.2) / 2.0,
                    &text,
                );
            }
        }
        ink.rule(LEFT * MM, RIGHT * MM, bottom);
    }
    ink.0.into_bytes()
}

// ---------------------------------------------------------------------------
// The file
// ---------------------------------------------------------------------------

/// The objects, then the table of where each starts. Nothing is compressed:
/// a sheet of thirty items is a few tens of kilobytes as it is.
fn write_pdf(title: &str, pages: &[Vec<u8>]) -> Vec<u8> {
    // 1 catalogue, 2 pages, 3 and 4 the fonts, 5 info, then each page and its
    // contents.
    let first = 6;
    let kids: Vec<String> = (0..pages.len())
        .map(|i| format!("{} 0 R", first + 2 * i))
        .collect();
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!(
            "<< /Type /Pages /Kids [ {} ] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec(),
        format!(
            "<< /Producer (Spork) /Title {} >>",
            literal(&winansi(title))
        )
        .into_bytes(),
    ];
    for (i, content) in pages.iter().enumerate() {
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [ 0 0 {} {} ] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
                n(PAGE_W),
                n(PAGE_H),
                first + 2 * i + 1
            )
            .into_bytes(),
        );
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(content);
        stream.extend_from_slice(b"\nendstream");
        objects.push(stream);
    }
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut at = vec![];
    for (i, body) in objects.iter().enumerate() {
        at.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in at {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(col: (f64, f64)) -> f64 {
        (col.1 - col.0 - GAP) * MM
    }

    /// The paper sheet's own fitting, item by item (Foodcare, 1 Oct 2026).
    #[test]
    fn text_is_fitted_where_the_paper_sheet_fitted_it() {
        assert_eq!(
            shrunk(b"JAR-8018", room(CODE)),
            (10.0, b"JAR-8018".to_vec())
        );
        assert_eq!(
            shrunk(b"Giveaways-Umbrella", room(CODE)),
            (9.25, b"Giveaways-Umbrella".to_vec())
        );
        assert_eq!(
            shrunk(b"Giveaways-Picnic-Blanket", room(CODE)),
            (9.0, [&b"Giveaways-Picnic-"[..], &[ELLIPSIS]].concat())
        );
        assert_eq!(
            wrapped(
                &winansi("Kitchen Bin Bags Small -18L - Roll 50/ ctn/ 20 rolls - White"),
                room(DESCRIPTION)
            ),
            vec![
                b"Kitchen Bin Bags Small -18L - Roll 50/ ctn/".to_vec(),
                b"20 rolls - White".to_vec()
            ]
        );
        assert_eq!(
            wrapped(
                &winansi("Sugarcane Round Plate - 9in - ctn 500 - White"),
                room(DESCRIPTION)
            ),
            vec![
                b"Sugarcane Round Plate - 9in - ctn 500 -".to_vec(),
                b"White".to_vec()
            ]
        );
        let long = wrapped(
            &winansi("Foodcare Bin Liners 810 x 950 - 82L Starseal-individual Fold - 200 per carton - Black heavy duty"),
            room(DESCRIPTION),
        );
        assert_eq!(long.len(), 2);
        assert_eq!(
            long[1],
            [
                &b"Starseal-individual Fold - 200 per carton"[..],
                &[ELLIPSIS]
            ]
            .concat()
        );
        assert_eq!(
            cut(b"FP1008FNOR004012345", false, 9.0, room(SUPPLIER)),
            [&b"FP1008FNOR0040"[..], &[ELLIPSIS]].concat()
        );
        assert_eq!(
            winansi("Picnic Blanket –150x 150mm × ‘soft’"),
            b"Picnic Blanket \x96150x 150mm \xD7 \x91soft\x92".to_vec()
        );
    }

    #[test]
    fn a_page_holds_fourteen_and_the_file_says_how_many_pages() {
        let row = |seq| SheetRow {
            seq,
            bin: "F-20-01".into(),
            code: "JAR-8018".into(),
            description: "Roll Towel Single Ply 80m Roll 16/ctn".into(),
            supplier_part: "AC-8018".into(),
            unit: "CTN".into(),
            soh: "19".into(),
            boxes: [
                Some("33.0".into()),
                Some("40.0".into()),
                Some("48.5".into()),
                Some("6.6".into()),
            ],
        };
        let sheet = Sheet {
            title: "WEIGHTS & DIMENSIONS CAPTURE — MELBOURNE".into(),
            subtitle: "NetSuite data refreshed".into(),
            rows: (1..=30).map(row).collect(),
        };
        let pdf = render(&sheet);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Count 3"), "thirty rows are three pages");
        assert!(text.contains("(Page 3 of 3)"));
        assert!(
            text.contains("(WEIGHTS & DIMENSIONS CAPTURE \\227 MELBOURNE)"),
            "the dash, in WinAnsi"
        );
        assert!(
            text.contains("(48.5)") && text.contains("(6.6)"),
            "what is recorded is in its box"
        );
        assert!(text.trim_end().ends_with("%%EOF"));
        // Every object starts where the table says it does.
        let xref = text.rfind("xref\n").unwrap();
        for (i, line) in text[xref..]
            .lines()
            .skip(3)
            .take_while(|l| l.ends_with(" n "))
            .enumerate()
        {
            let at: usize = line[..10].parse().unwrap();
            assert!(
                text[at..].starts_with(&format!("{} 0 obj", i + 1)),
                "object {}",
                i + 1
            );
        }
    }
}

//! Where the racks stand, and where that puts every bin. D173.
//!
//! A rack is described in a few numbers: where its front-left corner is on the
//! site plan, which way it faces, how wide each bay is, how high each level
//! starts, how many bins share a bay at each level, and what the bins are
//! called. [`expand`] turns that into every slot's code and box.
//!
//! Pure, and tested without a database, for [`crate::bins`]' reason: the
//! judgement is the difficulty, and the importer that writes the result is
//! [`crate::importing::layout`].
//!
//! # The frame
//!
//! One per site, in millimetres: `x` to the right on the plan, `y` up the page,
//! `z` up from the floor. A slot's box is its corner nearest the origin and its
//! extent along each axis, which is what `location`'s six columns now mean.
//!
//! A rack's own frame runs `u` along its front, left to right as you face it
//! from the aisle, and `v` from its front into its depth. At rotation 0 those
//! are `x` and `y`; each quarter turn counter-clockwise turns both. Only quarter
//! turns, so every box stays aligned with the plan.
//!
//! # Names
//!
//! A template is literal text with fields in braces: `{aisle}`, `{bay}`,
//! `{level}` and `{position}`. A numeric field may be zero-padded (`{bay:02}`)
//! or lettered (`{level:A}`, where 1 is `A` and 27 is `AA`), because bin codes
//! in the wild do both. `A-{bay:02}-{level}` names bay 3, level 1 `A-03-1`.

use std::collections::HashMap;

use serde::Serialize;

use crate::bins;

/// One rack, as its row says. The same fields as the `rack` table.
#[derive(Clone, Debug, PartialEq)]
pub struct Rack {
    pub code: String,
    pub aisle: String,
    pub x_mm: i32,
    pub y_mm: i32,
    pub rotation: i16,
    pub depth_mm: i32,
    pub height_mm: i32,
    pub upright_mm: i32,
    pub first_bay: i32,
    pub bay_step: i32,
    pub bay_widths_mm: Vec<i32>,
    pub first_level: i32,
    pub level_z_mm: Vec<i32>,
    pub positions: Vec<i32>,
    pub code_template: String,
}

/// An axis-aligned box on the site plan: a corner and an extent along each
/// axis, exactly as `location` stores one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Box3 {
    pub x_mm: i32,
    pub y_mm: i32,
    pub z_mm: i32,
    pub length_mm: i32,
    pub width_mm: i32,
    pub height_mm: i32,
}

impl Box3 {
    /// Whether two boxes share any volume. Touching faces do not: two bins side
    /// by side in one bay meet along a face and are not in each other's way.
    pub fn overlaps(&self, o: &Box3) -> bool {
        self.x_mm < o.x_mm + o.length_mm
            && o.x_mm < self.x_mm + self.length_mm
            && self.y_mm < o.y_mm + o.width_mm
            && o.y_mm < self.y_mm + self.width_mm
            && self.z_mm < o.z_mm + o.height_mm
            && o.z_mm < self.z_mm + self.height_mm
    }
}

/// One slot of one rack: what it is called and where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub code: String,
    pub bay: i32,
    pub level: i32,
    pub position: i32,
    pub r#box: Box3,
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Aisle,
    Bay,
    Level,
    Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Plain,
    Padded(usize),
    Letters,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    Field(Field, Style),
}

/// A parsed `code_template`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template(Vec<Piece>);

impl Template {
    pub fn parse(s: &str) -> Result<Template, String> {
        let mut pieces = vec![];
        let mut text = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    let mut inner = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some('{') | None => {
                                return Err(format!("`{s}` opens a field it does not close"))
                            }
                            Some(ch) => inner.push(ch),
                        }
                    }
                    if !text.is_empty() {
                        pieces.push(Piece::Text(std::mem::take(&mut text)));
                    }
                    let (name, spec) = match inner.split_once(':') {
                        Some((n, sp)) => (n.trim(), Some(sp.trim())),
                        None => (inner.trim(), None),
                    };
                    let field = match name {
                        "aisle" => Field::Aisle,
                        "bay" => Field::Bay,
                        "level" => Field::Level,
                        "position" => Field::Position,
                        other => {
                            return Err(format!(
                                "`{s}` names a field `{other}`; a template knows aisle, bay, \
                                 level and position"
                            ))
                        }
                    };
                    let style = match spec {
                        None => Style::Plain,
                        Some(_) if field == Field::Aisle => {
                            return Err(format!(
                                "`{s}` formats the aisle, which is text and is written as it is"
                            ))
                        }
                        Some("A") => Style::Letters,
                        Some(sp)
                            if sp.len() >= 2
                                && sp.starts_with('0')
                                && sp[1..].chars().all(|c| c.is_ascii_digit()) =>
                        {
                            Style::Padded(sp[1..].parse().unwrap_or(0))
                        }
                        Some(sp) => {
                            return Err(format!(
                                "`{s}` formats a field as `{sp}`; say `02` to pad to two \
                                 digits or `A` for letters"
                            ))
                        }
                    };
                    pieces.push(Piece::Field(field, style));
                }
                '}' => return Err(format!("`{s}` closes a field it never opened")),
                c => text.push(c),
            }
        }
        if !text.is_empty() {
            pieces.push(Piece::Text(text));
        }
        let t = Template(pieces);
        for (f, name) in [(Field::Bay, "bay"), (Field::Level, "level")] {
            if !t.has(f) {
                return Err(format!(
                    "`{s}` has no {{{name}}}, so two slots of one rack would share a code"
                ));
            }
        }
        Ok(t)
    }

    fn has(&self, f: Field) -> bool {
        self.0.iter().any(|p| matches!(p, Piece::Field(g, _) if *g == f))
    }

    pub fn render(&self, aisle: &str, bay: i32, level: i32, position: i32) -> Result<String, String> {
        let mut out = String::new();
        for p in &self.0 {
            match p {
                Piece::Text(t) => out.push_str(t),
                Piece::Field(Field::Aisle, _) => out.push_str(aisle),
                Piece::Field(f, style) => {
                    let n = match f {
                        Field::Bay => bay,
                        Field::Level => level,
                        Field::Position => position,
                        Field::Aisle => unreachable!(),
                    };
                    match style {
                        Style::Plain => out.push_str(&n.to_string()),
                        Style::Padded(w) => out.push_str(&format!("{n:0w$}", w = *w)),
                        Style::Letters => out.push_str(&letters(n)?),
                    }
                }
            }
        }
        Ok(out)
    }
}

/// 1 is `A`, 26 is `Z`, 27 is `AA`: the spreadsheet column numbering, which is
/// what a lettered level runs out into if a rack is ever that tall.
fn letters(n: i32) -> Result<String, String> {
    if n < 1 {
        return Err(format!("{n} has no letter: lettering starts at 1, which is A"));
    }
    let mut n = n as u32;
    let mut s = vec![];
    while n > 0 {
        n -= 1;
        s.push(char::from(b'A' + (n % 26) as u8));
        n /= 26;
    }
    Ok(s.into_iter().rev().collect())
}

// ---------------------------------------------------------------------------
// Expansion
// ---------------------------------------------------------------------------

impl Rack {
    /// How long the rack is along its front: every bay and every upright,
    /// including one at each end.
    pub fn length_mm(&self) -> i32 {
        self.upright_mm + self.bay_widths_mm.iter().map(|w| w + self.upright_mm).sum::<i32>()
    }

    /// The whole rack's box, for drawing it and for telling two racks apart.
    pub fn footprint(&self) -> Box3 {
        self.place(0, self.length_mm(), 0, self.height_mm)
    }

    /// A box in the rack's frame (`u0..u1` along the front, the full depth,
    /// `z0..z1` up) put onto the site plan.
    fn place(&self, u0: i32, u1: i32, z0: i32, z1: i32) -> Box3 {
        let (ux, uy, vx, vy) = match self.rotation {
            0 => (1, 0, 0, 1),
            90 => (0, 1, -1, 0),
            180 => (-1, 0, 0, -1),
            _ => (0, -1, 1, 0),
        };
        let corner = |u: i32, v: i32| (self.x_mm + u * ux + v * vx, self.y_mm + u * uy + v * vy);
        let (ax, ay) = corner(u0, 0);
        let (bx, by) = corner(u1, self.depth_mm);
        Box3 {
            x_mm: ax.min(bx),
            y_mm: ay.min(by),
            z_mm: z0,
            length_mm: (ax - bx).abs(),
            width_mm: (ay - by).abs(),
            height_mm: z1 - z0,
        }
    }

    /// Everything wrong with this row, as one sentence each. Empty when it can
    /// be expanded.
    ///
    /// The table's CHECKs hold most of this too; saying it here first turns a
    /// constraint name into a sentence about the rack, and holds what a CHECK
    /// cannot: that the levels ascend, and that the codes read back as this
    /// rack's aisle.
    pub fn problems(&self) -> Vec<String> {
        let mut p = vec![];
        let r = &self.code;
        if self.code.trim().is_empty() {
            p.push("a rack with no code cannot be told from the next one".into());
        }
        if self.aisle.trim().is_empty() {
            p.push(format!("rack {r} names no aisle"));
        }
        if ![0, 90, 180, 270].contains(&self.rotation) {
            p.push(format!(
                "rack {r} is turned {} degrees; racks turn in quarter turns (0, 90, 180, 270)",
                self.rotation
            ));
        }
        if self.depth_mm <= 0 || self.height_mm <= 0 {
            p.push(format!("rack {r} needs a depth and a height above zero"));
        }
        if self.upright_mm < 0 {
            p.push(format!("rack {r} has an upright narrower than nothing"));
        }
        if self.bay_widths_mm.is_empty() {
            p.push(format!("rack {r} has no bays"));
        }
        if self.bay_widths_mm.iter().any(|w| *w <= 0) {
            p.push(format!("rack {r} has a bay no wider than zero"));
        }
        if self.bay_step == 0 {
            p.push(format!("rack {r} numbers every bay the same (bay_step 0)"));
        }
        let last_bay = self.first_bay + (self.bay_widths_mm.len() as i32 - 1) * self.bay_step;
        if self.first_bay < 0 || last_bay < 0 {
            p.push(format!("rack {r} numbers a bay below zero"));
        }
        if self.first_level < 0 {
            p.push(format!("rack {r} numbers a level below zero"));
        }
        if self.level_z_mm.is_empty() {
            p.push(format!("rack {r} has no levels"));
        } else {
            if self.level_z_mm[0] < 0 {
                p.push(format!("rack {r} has a level below the floor"));
            }
            if self.level_z_mm.windows(2).any(|w| w[1] <= w[0]) {
                p.push(format!(
                    "rack {r}'s level heights do not rise ({:?}); give them lowest first",
                    self.level_z_mm
                ));
            }
            if *self.level_z_mm.last().unwrap() >= self.height_mm {
                p.push(format!("rack {r}'s top level starts at or above its height"));
            }
        }
        if self.positions.len() != self.level_z_mm.len() {
            p.push(format!(
                "rack {r} gives {} position counts for {} levels",
                self.positions.len(),
                self.level_z_mm.len()
            ));
        }
        if self.positions.iter().any(|n| *n <= 0) {
            p.push(format!("rack {r} has a level with no positions"));
        }
        match Template::parse(&self.code_template) {
            Err(e) => p.push(format!("rack {r}: {e}")),
            Ok(t) => {
                if self.positions.iter().any(|n| *n > 1) && !t.has(Field::Position) {
                    p.push(format!(
                        "rack {r} puts more than one bin in a bay on some level, and \
                         `{}` has no {{position}} to tell them apart",
                        self.code_template
                    ));
                }
            }
        }
        p
    }

    /// Every slot, bay by bay along the front, then level by level up, then
    /// position by position along the bay.
    ///
    /// Refuses a rack with [`Rack::problems`], and one whose codes do not read
    /// back as its own aisle: J77 finds a bin a rack should place by the aisle
    /// its code decomposes to, so a rack whose codes decompose elsewhere would
    /// place bins the check then looks for somewhere else.
    pub fn expand(&self) -> Result<Vec<Slot>, String> {
        let problems = self.problems();
        if !problems.is_empty() {
            return Err(problems.join("; "));
        }
        let t = Template::parse(&self.code_template)?;
        let mut slots = vec![];
        let mut u = self.upright_mm;
        for (i, w) in self.bay_widths_mm.iter().enumerate() {
            let bay = self.first_bay + i as i32 * self.bay_step;
            for (j, z0) in self.level_z_mm.iter().enumerate() {
                let z1 = self.level_z_mm.get(j + 1).copied().unwrap_or(self.height_mm);
                let level = self.first_level + j as i32;
                let n = self.positions[j];
                for k in 0..n {
                    // Split with the remainder spread, so the positions tile the
                    // bay exactly whatever it divides into.
                    let a = u + w * k / n;
                    let b = u + w * (k + 1) / n;
                    let position = k + 1;
                    let code = t.render(&self.aisle, bay, level, position)?;
                    let parsed = bins::decompose(&code);
                    if parsed.aisle.as_deref() != Some(self.aisle.as_str()) {
                        return Err(format!(
                            "rack {} names a bin `{code}`, which reads as aisle {}, not {}",
                            self.code,
                            parsed.aisle.as_deref().unwrap_or("(none)"),
                            self.aisle
                        ));
                    }
                    slots.push(Slot { code, bay, level, position, r#box: self.place(a, b, *z0, z1) });
                }
            }
            u += w + self.upright_mm;
        }
        Ok(slots)
    }
}

/// Expand every rack of one site together, refusing a code two slots share.
///
/// Across racks as well as within one: two racks naming one bin would each
/// place it, and whichever was written last would win without anybody deciding.
pub fn expand_site(racks: &[Rack]) -> Result<Vec<(usize, Slot)>, String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out = vec![];
    for (i, r) in racks.iter().enumerate() {
        for s in r.expand()? {
            if let Some(j) = seen.insert(s.code.clone(), i) {
                return Err(if i == j {
                    format!("rack {} names `{}` twice", r.code, s.code)
                } else {
                    format!("racks {} and {} both name `{}`", racks[j].code, r.code, s.code)
                });
            }
            out.push((i, s));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Floor outlines
// ---------------------------------------------------------------------------

/// Read `x,y x,y ...` into points.
pub fn parse_outline(s: &str) -> Result<Vec<(i32, i32)>, String> {
    let mut pts = vec![];
    for pair in s.split(|c: char| c.is_whitespace() || c == ';').filter(|p| !p.is_empty()) {
        let Some((x, y)) = pair.split_once(',') else {
            return Err(format!("`{pair}` is not a point; write each as x,y"));
        };
        let n = |v: &str| v.trim().parse::<i32>().map_err(|_| format!("`{pair}` is not a point in millimetres"));
        pts.push((n(x)?, n(y)?));
    }
    Ok(pts)
}

/// What is wrong with an outline, or nothing.
///
/// An outline is a simple polygon: at least three corners, some area, and no
/// edge crossing or touching another except its neighbours at their shared
/// corner. One that crosses itself has no inside anybody agrees on, and "is
/// this spot in the staging lane" is the question it exists to answer.
pub fn outline_problem(pts: &[(i32, i32)]) -> Option<String> {
    let mut pts = pts.to_vec();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    if pts.len() < 3 {
        return Some("an outline needs at least three corners".into());
    }
    let n = pts.len();
    for i in 0..n {
        for j in i + 1..n {
            // Neighbouring edges share a corner and nothing else is asked of them,
            // beyond not folding back over each other, which the area and the
            // crossing test between the others catch.
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let (c, d) = (pts[j], pts[(j + 1) % n]);
            if segments_meet(a, b, c, d) {
                return Some(format!(
                    "the edge from {},{} to {},{} meets the edge from {},{} to {},{}",
                    a.0, a.1, b.0, b.1, c.0, c.1, d.0, d.1
                ));
            }
        }
    }
    // After the crossings: a bowtie's two halves cancel, and "its edges cross"
    // says more than "it has no area".
    let area2: i64 = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.0 as i64 * b.1 as i64 - b.0 as i64 * a.1 as i64
        })
        .sum();
    if area2 == 0 {
        return Some("the outline encloses no area".into());
    }
    None
}

fn orient(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> i64 {
    let v = (b.0 as i64 - a.0 as i64) * (c.1 as i64 - a.1 as i64)
        - (b.1 as i64 - a.1 as i64) * (c.0 as i64 - a.0 as i64);
    v.signum()
}

fn on_segment(a: (i32, i32), b: (i32, i32), p: (i32, i32)) -> bool {
    p.0 >= a.0.min(b.0) && p.0 <= a.0.max(b.0) && p.1 >= a.1.min(b.1) && p.1 <= a.1.max(b.1)
}

/// Whether two segments share any point, touching included.
fn segments_meet(a: (i32, i32), b: (i32, i32), c: (i32, i32), d: (i32, i32)) -> bool {
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    if o1 != o2 && o3 != o4 && o1 * o2 <= 0 && o3 * o4 <= 0 {
        return true;
    }
    (o1 == 0 && on_segment(a, b, c))
        || (o2 == 0 && on_segment(a, b, d))
        || (o3 == 0 && on_segment(c, d, a))
        || (o4 == 0 && on_segment(c, d, b))
}

/// A list of millimetres as a person writes one: `1200 1200 900`, or
/// `10x2700 1800` for ten bays of 2700 and one of 1800. Commas and semicolons
/// separate as well as spaces do.
pub fn parse_list(s: &str) -> Result<Vec<i32>, String> {
    let mut out = vec![];
    for tok in s
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|t| !t.is_empty())
    {
        match tok.split_once(['x', 'X', '*']) {
            Some((n, v)) => {
                let n: usize = n.parse().map_err(|_| format!("`{tok}` is not a count times a value"))?;
                let v: i32 = v.parse().map_err(|_| format!("`{tok}` is not a count times a value"))?;
                if n == 0 || n > 1000 {
                    return Err(format!("`{tok}` repeats {n} times; say between 1 and 1000"));
                }
                out.extend(std::iter::repeat_n(v, n));
            }
            None => out.push(tok.parse().map_err(|_| format!("`{tok}` is not a number"))?),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rack() -> Rack {
        Rack {
            code: "A-L".into(),
            aisle: "A".into(),
            x_mm: 1000,
            y_mm: 2000,
            rotation: 0,
            depth_mm: 1100,
            height_mm: 6000,
            upright_mm: 100,
            first_bay: 1,
            bay_step: 2,
            bay_widths_mm: vec![2700, 2700, 1800],
            first_level: 1,
            level_z_mm: vec![0, 1500, 3000],
            positions: vec![3, 1, 1],
            code_template: "{aisle}-{bay:02}-{level}-{position}".into(),
        }
    }

    #[test]
    fn a_template_says_what_a_slot_is_called() {
        let t = Template::parse("{aisle}.{bay:02}.{level:A}").unwrap();
        assert_eq!(t.render("K", 7, 2, 1).unwrap(), "K.07.B");
        assert_eq!(t.render("K", 123, 28, 1).unwrap(), "K.123.AB");
        assert!(Template::parse("{aisle}-{bay}").unwrap_err().contains("{level}"));
        assert!(Template::parse("{aisle}-{bay}-{lvl}").is_err());
        assert!(Template::parse("{aisle:02}-{bay}-{level}").is_err());
        assert!(Template::parse("{aisle}-{bay}-{level").is_err());
        assert!(Template::parse("{bay:2}-{level}").is_err(), "pad with a leading zero");
    }

    #[test]
    fn slots_run_bay_by_bay_and_tile_each_bay_exactly() {
        let slots = rack().expand().unwrap();
        // Three bays of (3 + 1 + 1) slots.
        assert_eq!(slots.len(), 15);
        let codes: Vec<&str> = slots.iter().take(6).map(|s| s.code.as_str()).collect();
        assert_eq!(codes, ["A-01-1-1", "A-01-1-2", "A-01-1-3", "A-01-2-1", "A-01-3-1", "A-03-1-1"]);

        // Bay 1 starts after the first upright, and its three positions meet
        // exactly across its 2700.
        let b1: Vec<Box3> = slots[..3].iter().map(|s| s.r#box).collect();
        assert_eq!(b1[0].x_mm, 1100);
        assert_eq!(b1.iter().map(|b| b.length_mm).sum::<i32>(), 2700);
        assert_eq!(b1[0].x_mm + b1[0].length_mm, b1[1].x_mm);
        assert_eq!(b1[1].x_mm + b1[1].length_mm, b1[2].x_mm);
        assert!(b1.iter().all(|b| b.y_mm == 2000 && b.width_mm == 1100 && b.z_mm == 0 && b.height_mm == 1500));

        // The top level reaches the rack's height; bay 5 follows bay 3's upright.
        let top = &slots[4].r#box;
        assert_eq!((top.z_mm, top.height_mm), (3000, 3000));
        let b5 = slots.iter().find(|s| s.code == "A-05-1-1").unwrap();
        assert_eq!(b5.r#box.x_mm, 1000 + 100 + 2700 + 100 + 2700 + 100);
        assert_eq!(rack().length_mm(), 100 + 2800 + 2800 + 1900);
    }

    #[test]
    fn a_turned_rack_keeps_every_slot_inside_its_footprint_and_apart() {
        for rotation in [0, 90, 180, 270] {
            let r = Rack { rotation, ..rack() };
            let fp = r.footprint();
            let slots = r.expand().unwrap();
            for s in &slots {
                let b = s.r#box;
                assert!(b.length_mm > 0 && b.width_mm > 0 && b.height_mm > 0);
                assert!(
                    b.x_mm >= fp.x_mm && b.x_mm + b.length_mm <= fp.x_mm + fp.length_mm
                        && b.y_mm >= fp.y_mm && b.y_mm + b.width_mm <= fp.y_mm + fp.width_mm,
                    "{rotation}: {} outside the rack",
                    s.code
                );
            }
            for (i, a) in slots.iter().enumerate() {
                for b in &slots[i + 1..] {
                    assert!(!a.r#box.overlaps(&b.r#box), "{rotation}: {} and {} overlap", a.code, b.code);
                }
            }
        }
        // A quarter turn about the origin: the rack now runs up the page, with
        // its depth off to the left.
        let fp = Rack { rotation: 90, ..rack() }.footprint();
        assert_eq!((fp.x_mm, fp.y_mm, fp.length_mm, fp.width_mm), (1000 - 1100, 2000, 1100, rack().length_mm()));
        let first = &Rack { rotation: 180, ..rack() }.expand().unwrap()[0];
        assert_eq!(first.r#box.x_mm + first.r#box.length_mm, 1000 - 100, "180 runs leftwards from the origin");
    }

    #[test]
    fn a_rack_that_contradicts_itself_says_how() {
        let bad = |f: fn(&mut Rack)| {
            let mut r = rack();
            f(&mut r);
            r.expand().unwrap_err()
        };
        assert!(bad(|r| r.rotation = 45).contains("quarter turns"));
        assert!(bad(|r| r.level_z_mm = vec![0, 3000, 1500]).contains("do not rise"));
        assert!(bad(|r| r.positions = vec![1, 1]).contains("3 levels"));
        assert!(bad(|r| r.code_template = "{aisle}-{bay}-{level}".into()).contains("{position}"));
        assert!(bad(|r| r.bay_step = 0).contains("bay_step 0"));
        assert!(bad(|r| { r.first_bay = 3; r.bay_step = -2 }).contains("below zero"));
        // The template spells a different aisle from the rack's own.
        assert!(bad(|r| r.code_template = "B-{bay}-{level}-{position}".into()).contains("aisle B, not A"));
    }

    #[test]
    fn two_racks_cannot_name_one_bin() {
        let a = rack();
        let b = Rack { code: "A-R".into(), first_bay: 5, ..rack() };
        let e = expand_site(&[a.clone(), b]).unwrap_err();
        assert!(e.contains("A-L and A-R") && e.contains("A-05-1-1"), "{e}");
        let b = Rack { code: "A-R".into(), first_bay: 2, ..rack() };
        assert_eq!(expand_site(&[a, b]).unwrap().len(), 30);
    }

    #[test]
    fn lists_are_written_the_way_a_survey_writes_them() {
        assert_eq!(parse_list("10x2700 1800").unwrap().len(), 11);
        assert_eq!(parse_list("0, 1500;3000").unwrap(), vec![0, 1500, 3000]);
        assert!(parse_list("2700 wide").is_err());
        assert!(parse_list("0x2700").is_err());
    }

    #[test]
    fn an_outline_must_have_an_inside() {
        let sq = parse_outline("0,0 1000,0 1000,500 0,500").unwrap();
        assert_eq!(outline_problem(&sq), None);
        let closed = parse_outline("0,0 1000,0 1000,500 0,500 0,0").unwrap();
        assert_eq!(outline_problem(&closed), None, "repeating the first corner is fine");
        let bowtie = parse_outline("0,0 1000,500 1000,0 0,500").unwrap();
        assert!(outline_problem(&bowtie).unwrap().contains("meets"));
        let flat = parse_outline("0,0 500,0 1000,0").unwrap();
        assert!(outline_problem(&flat).unwrap().contains("no area"));
        assert!(outline_problem(&sq[..2]).is_some());
        // An L-shaped staging lane is not a crossing.
        let l = parse_outline("0,0 2000,0 2000,500 500,500 500,2000 0,2000").unwrap();
        assert_eq!(outline_problem(&l), None);
        assert!(parse_outline("0,0 10 5,5").is_err());
    }
}

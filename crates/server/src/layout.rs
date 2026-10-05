//! Where things are: places, their grids, and the names on their cells. D173.
//!
//! A site's layout is boxes drawn inside other boxes. A place is positioned in
//! cells of its parent, turned by some angle, and marked solid or walk-through;
//! it may hold a grid of bays, levels, rows and positions, and every bin sits in
//! one cell of one place. Nothing is measured.
//!
//! This module is the judgement, and pure: naming patterns, the labels a rack
//! carries, composing a place's position on the site from its chain of parents,
//! and drafting a first layout from a bin list. [`crate::places`] reads and
//! writes the rows.
//!
//! # Exact and approximate
//!
//! A cell is exact: bay 3 is between bays 2 and 4 because the labels say so. A
//! drawn position is an estimate, and it is only ever stored relative to its
//! parent. [`compose`] turns a chain of those into a position on the site when
//! somebody reads it, so there is one place the arithmetic happens and no copy
//! of it to drift.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

// ---------------------------------------------------------------------------
// Cells and grids
// ---------------------------------------------------------------------------

/// A cell of a place's grid, each coordinate counted from 1.
///
/// `bay` is the column along the place, counted from the front's left as the
/// front is faced, on either side and whichever end its numbering starts; `side`
/// is 1 for the front and 2 for the back of a rack with two. So the cell behind
/// another is the same column on the other side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct GridCell {
    pub bay: i32,
    pub level: i32,
    pub row: i32,
    pub position: i32,
    pub side: i32,
}

/// A place's grid and how its labels are numbered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub bays: i32,
    pub levels: i32,
    pub rows: i32,
    /// One count per level. Empty means one position everywhere.
    pub positions: Vec<i32>,
    pub first_bay: i32,
    pub bay_step: i32,
    pub first_level: i32,
    /// 1, or 2 for a rack with a face on each side, numbered round it.
    pub sides: i32,
    /// Its first label is at the right end of its front, and the labels run
    /// leftwards (D220). Otherwise they start at the left.
    pub from_right: bool,
}

impl Grid {
    pub fn single() -> Grid {
        Grid {
            bays: 1,
            levels: 1,
            rows: 1,
            positions: vec![],
            first_bay: 1,
            bay_step: 1,
            first_level: 1,
            sides: 1,
            from_right: false,
        }
    }

    /// How many bins share a bay at this level (counted from 1).
    pub fn positions_at(&self, level: i32) -> i32 {
        self.positions.get((level - 1) as usize).copied().unwrap_or(1)
    }

    pub fn contains(&self, c: GridCell) -> bool {
        c.side >= 1
            && c.side <= self.sides
            && c.bay >= 1
            && c.bay <= self.bays
            && c.level >= 1
            && c.level <= self.levels
            && c.row >= 1
            && c.row <= self.rows
            && c.position >= 1
            && c.position <= self.positions_at(c.level)
    }

    /// Every cell: the front, then the back, each bay by bay, then level by
    /// level up, then row by row back, then position by position along the bay.
    pub fn cells(&self) -> Vec<GridCell> {
        let mut out = vec![];
        for side in 1..=self.sides {
            for bay in 1..=self.bays {
                for level in 1..=self.levels {
                    for row in 1..=self.rows {
                        for position in 1..=self.positions_at(level) {
                            out.push(GridCell { bay, level, row, position, side });
                        }
                    }
                }
            }
        }
        out
    }

    /// The number on the label of this column's bay on this side.
    ///
    /// The front is numbered from the end its numbering starts at: the left,
    /// or the right (D220). The back is numbered round: on from the front's
    /// last, the other way along, so behind the front's first bay is the last
    /// of all (E-36 behind E-01).
    pub fn label_number(&self, side: i32, bay: i32) -> i32 {
        // How far along the front's numbering this column is, from 1.
        let along = if self.from_right { self.bays + 1 - bay } else { bay };
        if side == 2 {
            self.first_bay + (2 * self.bays - along) * self.bay_step
        } else {
            self.first_bay + (along - 1) * self.bay_step
        }
    }

    pub fn level_number(&self, level: i32) -> i32 {
        self.first_level + level - 1
    }

    /// Everything wrong with this grid, one sentence each.
    pub fn problems(&self) -> Vec<String> {
        let mut p = vec![];
        if self.bays < 1 || self.levels < 1 || self.rows < 1 {
            p.push("a grid needs at least one bay, one level and one row".into());
        }
        if !self.positions.is_empty() && self.positions.len() != self.levels as usize {
            p.push(format!(
                "{} position counts for {} levels",
                self.positions.len(),
                self.levels
            ));
        }
        if self.positions.iter().any(|n| *n < 1) {
            p.push("a level with no positions".into());
        }
        if self.bay_step == 0 {
            p.push("every bay would have the same number (a step of 0)".into());
        }
        if self.sides != 1 && self.sides != 2 {
            p.push("a grid has one side or two".into());
        }
        if self.first_bay < 0 || self.first_bay + (self.bays.max(1) * self.sides.clamp(1, 2) - 1) * self.bay_step < 0 {
            p.push("a bay numbered below zero".into());
        }
        if self.first_level < 0 {
            p.push("a level numbered below zero".into());
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Naming patterns
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Bay,
    Level,
    Row,
    Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Plain,
    Padded(usize),
    Letters,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    Field(Field, Style),
}

/// A parsed `bin_pattern`: literal text with fields in braces. `C-{bay:02}-{level}`
/// names bay 7, level 3 `C-07-3`; `{level:A}` letters it, 1 being `A`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern(Vec<Piece>);

impl Pattern {
    pub fn parse(s: &str) -> Result<Pattern, String> {
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
                        "bay" => Field::Bay,
                        "level" => Field::Level,
                        "row" => Field::Row,
                        "position" => Field::Position,
                        other => {
                            return Err(format!(
                                "`{s}` names a field `{other}`; a pattern knows bay, level, row \
                                 and position"
                            ))
                        }
                    };
                    let style = match spec {
                        None => Style::Plain,
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
                                "`{s}` formats a field as `{sp}`; say `02` to pad to two digits \
                                 or `A` for letters"
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
        if !pieces.iter().any(|p| matches!(p, Piece::Field(..))) {
            return Err(format!("`{s}` has no fields, so every cell would have the same name"));
        }
        Ok(Pattern(pieces))
    }

    pub fn has(&self, f: Field) -> bool {
        self.0.iter().any(|p| matches!(p, Piece::Field(g, _) if *g == f))
    }

    pub fn style(&self, f: Field) -> Option<Style> {
        self.0.iter().find_map(|p| match p {
            Piece::Field(g, s) if *g == f => Some(*s),
            _ => None,
        })
    }

    /// Why this pattern cannot name every cell of this grid apart, if it
    /// cannot: a field is missing for an axis with more than one cell.
    pub fn gap(&self, grid: &Grid) -> Option<String> {
        let needs = [
            (grid.bays > 1, Field::Bay, "bay"),
            (grid.levels > 1, Field::Level, "level"),
            (grid.rows > 1, Field::Row, "row"),
            (grid.positions.iter().any(|n| *n > 1), Field::Position, "position"),
        ];
        needs
            .iter()
            .find(|(needed, f, _)| *needed && !self.has(*f))
            .map(|(_, _, name)| format!("it has no {{{name}}}, so two cells would share a name"))
    }

    pub fn render(&self, grid: &Grid, c: GridCell) -> Result<String, String> {
        let mut out = String::new();
        for p in &self.0 {
            match p {
                Piece::Text(t) => out.push_str(t),
                Piece::Field(f, style) => {
                    let n = match f {
                        Field::Bay => grid.label_number(c.side, c.bay),
                        Field::Level => grid.level_number(c.level),
                        Field::Row => c.row,
                        Field::Position => c.position,
                    };
                    out.push_str(&format_number(n, *style)?);
                }
            }
        }
        Ok(out)
    }

    /// Every cell and its bin's name, refusing a pattern that cannot tell two
    /// cells apart.
    pub fn names(&self, grid: &Grid) -> Result<Vec<(GridCell, String)>, String> {
        if let Some(g) = self.gap(grid) {
            return Err(g);
        }
        let mut seen = HashSet::new();
        let mut out = vec![];
        for c in grid.cells() {
            let name = self.render(grid, c)?;
            if !seen.insert(name.clone()) {
                return Err(format!("it names two cells `{name}`"));
            }
            out.push((c, name));
        }
        Ok(out)
    }
}

pub fn format_number(n: i32, style: Style) -> Result<String, String> {
    match style {
        Style::Plain => Ok(n.to_string()),
        Style::Padded(w) => Ok(format!("{n:0w$}")),
        Style::Letters => letters(n),
    }
}

/// 1 is `A`, 26 is `Z`, 27 is `AA`.
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

fn from_letters(s: &str) -> Option<i32> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    Some(s.chars().fold(0, |n, c| n * 26 + (c as i32 - 'A' as i32 + 1)))
}

/// The labels along a place's face, as its racks would print them: the bay
/// numbers across the front, left to right as you face it, and the level
/// numbers up.
pub fn labels(grid: &Grid, pattern: Option<&Pattern>) -> (Vec<String>, Vec<String>) {
    let bays = (1..=grid.bays).map(|b| bay_label(grid, pattern, 1, b)).collect();
    let levels = (1..=grid.levels).map(|l| level_label(grid, pattern, l)).collect();
    (bays, levels)
}

/// The labels along the back of a rack with two sides, left to right as you
/// face the back: the column at the front's right end first. Nothing for a
/// place with one side.
pub fn back_labels(grid: &Grid, pattern: Option<&Pattern>) -> Vec<String> {
    if grid.sides < 2 {
        return vec![];
    }
    (1..=grid.bays).rev().map(|b| bay_label(grid, pattern, 2, b)).collect()
}

fn style_of(pattern: Option<&Pattern>, f: Field) -> Style {
    pattern.and_then(|p| p.style(f)).unwrap_or(Style::Plain)
}

fn bay_label(grid: &Grid, pattern: Option<&Pattern>, side: i32, bay: i32) -> String {
    let n = grid.label_number(side, bay);
    format_number(n, style_of(pattern, Field::Bay)).unwrap_or_else(|_| n.to_string())
}

fn level_label(grid: &Grid, pattern: Option<&Pattern>, level: i32) -> String {
    let n = grid.level_number(level);
    format_number(n, style_of(pattern, Field::Level)).unwrap_or_else(|_| n.to_string())
}

/// A cell in the words its labels use: "bay 05, level 3", and on a rack with
/// two sides which one, "back, bay 36, level 01".
pub fn whereabouts(grid: &Grid, pattern: Option<&Pattern>, c: GridCell) -> String {
    let mut parts = vec![];
    if grid.sides > 1 {
        parts.push(if c.side == 2 { "back".to_string() } else { "front".to_string() });
    }
    parts.push(format!("bay {}", bay_label(grid, pattern, c.side, c.bay)));
    if grid.levels > 1 {
        parts.push(format!("level {}", level_label(grid, pattern, c.level)));
    }
    if grid.positions_at(c.level) > 1 {
        parts.push(format!("position {}", c.position));
    }
    if grid.rows > 1 {
        parts.push(if c.row == 1 { "front row".into() } else { format!("row {}", c.row) });
    }
    parts.join(", ")
}

// ---------------------------------------------------------------------------
// Positions on the site
// ---------------------------------------------------------------------------

/// Where a place's corner is on the site, and which way it faces: its own box
/// composed with every parent's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Degrees counter-clockwise, 0 to 360.
    pub turn: f64,
}

impl Frame {
    pub const SITE: Frame = Frame { x: 0.0, y: 0.0, z: 0.0, turn: 0.0 };

    /// A point in this frame's cells, on the site.
    pub fn point(&self, u: f64, v: f64) -> [f64; 2] {
        let (s, c) = self.turn.to_radians().sin_cos();
        [self.x + u * c - v * s, self.y + u * s + v * c]
    }
}

/// A child's frame on the site, from its parent's and its own box.
pub fn compose(parent: &Frame, x: f64, y: f64, z: f64, turn: f64) -> Frame {
    let [wx, wy] = parent.point(x, y);
    Frame { x: wx, y: wy, z: parent.z + z, turn: (parent.turn + turn).rem_euclid(360.0) }
}

/// A place's footprint on the site: its outline, or its rectangle.
pub fn footprint(frame: &Frame, length: f64, depth: f64, outline: Option<&[f64]>) -> Vec<[f64; 2]> {
    match outline {
        Some(o) if o.len() >= 6 => o.chunks(2).map(|p| frame.point(p[0], p[1])).collect(),
        _ => vec![
            frame.point(0.0, 0.0),
            frame.point(length, 0.0),
            frame.point(length, depth),
            frame.point(0.0, depth),
        ],
    }
}

/// Frames for every place, given each one's parent and box, walking from the
/// site down. A place whose chain of parents loops (J78) gets no frame rather
/// than an endless walk.
pub fn frames<K: Copy + Eq + std::hash::Hash>(
    places: &HashMap<K, (Option<K>, f64, f64, f64, f64)>,
) -> HashMap<K, Frame> {
    let mut out: HashMap<K, Frame> = HashMap::new();
    for &start in places.keys() {
        let mut chain = vec![];
        let mut seen = HashSet::new();
        let mut at = Some(start);
        let mut base = Frame::SITE;
        let mut looped = false;
        while let Some(k) = at {
            if let Some(f) = out.get(&k) {
                base = *f;
                break;
            }
            if !seen.insert(k) {
                looped = true;
                break;
            }
            chain.push(k);
            at = places.get(&k).and_then(|p| p.0);
        }
        if looped {
            continue;
        }
        for k in chain.into_iter().rev() {
            let (_, x, y, z, turn) = places[&k];
            base = compose(&base, x, y, z, turn);
            out.insert(k, base);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Outlines
// ---------------------------------------------------------------------------

/// What is wrong with an outline, or nothing. At least three corners, no edge
/// crossing or touching another except its neighbours at their shared corner,
/// and some area.
pub fn outline_problem(flat: &[f64]) -> Option<String> {
    if flat.len() % 2 != 0 {
        return Some("an outline is pairs of numbers".into());
    }
    let mut pts: Vec<(f64, f64)> = flat.chunks(2).map(|p| (p[0], p[1])).collect();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    if pts.len() < 3 {
        return Some("an outline needs at least three corners".into());
    }
    if pts.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return Some("an outline's corners must be numbers".into());
    }
    let n = pts.len();
    for i in 0..n {
        for j in i + 1..n {
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
    // After the crossings: a bowtie's halves cancel, and "its edges cross" says
    // more than "it has no area".
    let area2: f64 = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum();
    if area2.abs() < 1e-9 {
        return Some("the outline encloses no area".into());
    }
    None
}

fn orient(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> i8 {
    let v = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    if v.abs() < 1e-12 {
        0
    } else if v > 0.0 {
        1
    } else {
        -1
    }
}

fn on_segment(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> bool {
    p.0 >= a.0.min(b.0) && p.0 <= a.0.max(b.0) && p.1 >= a.1.min(b.1) && p.1 <= a.1.max(b.1)
}

fn segments_meet(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    if o1 != o2 && o3 != o4 {
        return true;
    }
    (o1 == 0 && on_segment(a, b, c))
        || (o2 == 0 && on_segment(a, b, d))
        || (o3 == 0 && on_segment(c, d, a))
        || (o4 == 0 && on_segment(c, d, b))
}

// ---------------------------------------------------------------------------
// A first draft, from the bin list
// ---------------------------------------------------------------------------

/// One place the draft proposes, and the bins it takes.
#[derive(Clone, Debug, PartialEq)]
pub struct Drafted {
    pub name: String,
    pub solid: bool,
    pub pattern: String,
    pub grid: Grid,
    pub bins: Vec<(String, GridCell)>,
}

/// What a bin list suggests, before anything is drawn.
#[derive(Debug, Default, PartialEq)]
pub struct Draft {
    pub places: Vec<Drafted>,
    /// Codes that follow no pattern the draft could see: `3PL`, `ASSEMBLY-BIN`.
    /// They wait to be placed by hand.
    pub unmatched: Vec<String>,
}

/// A code split at its separators: `C-07-3` is `C`, `07`, `3` with `-`, `-`.
fn split(code: &str) -> (Vec<&str>, Vec<char>) {
    let mut parts = vec![];
    let mut seps = vec![];
    let mut start = 0;
    for (i, ch) in code.char_indices() {
        if matches!(ch, '-' | '.' | '_' | '/' | ' ') {
            parts.push(&code[start..i]);
            seps.push(ch);
            start = i + ch.len_utf8();
        }
    }
    parts.push(&code[start..]);
    (parts, seps)
}

/// A numbered token and how it is written.
fn read_number(t: &str, letters_ok: bool) -> Option<(i32, Style)> {
    if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) && t.len() <= 6 {
        let n: i32 = t.parse().ok()?;
        let style = if t.len() > 1 && t.starts_with('0') { Style::Padded(t.len()) } else { Style::Plain };
        return Some((n, style));
    }
    if letters_ok && t.len() <= 2 {
        return from_letters(t).map(|n| (n, Style::Letters));
    }
    None
}

/// The style most of a group's tokens are written in. A padded width wins over
/// plain when any token needs it: `07` and `10` are both `{bay:02}`.
fn common_style(styles: &[Style]) -> Style {
    if styles.contains(&Style::Letters) {
        return Style::Letters;
    }
    styles
        .iter()
        .filter_map(|s| match s {
            Style::Padded(w) => Some(*w),
            _ => None,
        })
        .max()
        .map(Style::Padded)
        .unwrap_or(Style::Plain)
}

/// Propose one place per family of codes.
///
/// A family is codes that share their first part and their shape: `C-07-3` and
/// `C-12-1` are one, `DOCK-1` to `DOCK-6` another. The first part names it and
/// the parts after it are bay, level and position, in that order, because that
/// is how bin codes are written wherever this has been seen. The grid spans
/// the lowest to the highest number found, stepping by two when every bay is
/// odd or every bay is even.
///
/// **Every bin the draft places, its pattern names back exactly.** A code that
/// does not round-trip is left unmatched rather than put in a cell it only
/// resembles.
pub fn draft(bins: &[(String, bool)]) -> Draft {
    // (first part, separators, how many parts) -> codes, and whether most are
    // racking (solid).
    let mut families: BTreeMap<(String, String, usize), Vec<(&str, bool)>> = BTreeMap::new();
    let mut out = Draft::default();
    for (code, solid) in bins {
        let (parts, seps) = split(code);
        let head = parts[0];
        let numbered = parts.len() >= 2
            && parts.len() <= 4
            && !head.is_empty()
            && read_number(parts[1], false).is_some()
            && parts[2..].iter().all(|p| read_number(p, true).is_some());
        if !numbered {
            out.unmatched.push(code.clone());
            continue;
        }
        families
            .entry((head.to_string(), seps.iter().collect(), parts.len()))
            .or_default()
            .push((code.as_str(), *solid));
    }

    let mut taken: HashMap<String, usize> = HashMap::new();
    for ((head, seps, n), codes) in families {
        let sep: Vec<char> = seps.chars().collect();
        let read: Vec<(&str, Vec<(i32, Style)>)> = codes
            .iter()
            .map(|(c, _)| {
                let (parts, _) = split(c);
                (*c, parts[1..].iter().map(|p| read_number(p, true).unwrap()).collect())
            })
            .collect();
        let axis = |i: usize| -> (Vec<i32>, Style) {
            let nums: Vec<i32> = read.iter().filter_map(|(_, v)| v.get(i).map(|x| x.0)).collect();
            let styles: Vec<Style> = read.iter().filter_map(|(_, v)| v.get(i).map(|x| x.1)).collect();
            (nums, common_style(&styles))
        };

        let (bays, bay_style) = axis(0);
        let (min_b, max_b) = (*bays.iter().min().unwrap(), *bays.iter().max().unwrap());
        let same_parity = bays.iter().all(|b| (b - min_b) % 2 == 0);
        let bay_step = if same_parity && max_b > min_b { 2 } else { 1 };

        let (levels, level_style) = if n >= 3 { axis(1) } else { (vec![1], Style::Plain) };
        let (min_l, max_l) = (*levels.iter().min().unwrap(), *levels.iter().max().unwrap());

        let mut grid = Grid {
            bays: (max_b - min_b) / bay_step + 1,
            levels: max_l - min_l + 1,
            rows: 1,
            positions: vec![],
            first_bay: min_b,
            bay_step,
            first_level: min_l,
            sides: 1,
            from_right: false,
        };
        let mut position_style = Style::Plain;
        if n == 4 {
            let (_, style) = axis(2);
            position_style = style;
            let mut most = vec![1; grid.levels as usize];
            for (_, v) in &read {
                let l = (v[1].0 - min_l) as usize;
                most[l] = most[l].max(v[2].0);
            }
            if most.iter().any(|m| *m > 1) {
                grid.positions = most;
            }
        }

        let mut text = head.clone();
        let fields = [
            (Field::Bay, bay_style),
            (Field::Level, level_style),
            (Field::Position, position_style),
        ];
        for (i, (field, style)) in fields.iter().take(n - 1).enumerate() {
            text.push(sep[i]);
            let name = match field {
                Field::Bay => "bay",
                Field::Level => "level",
                _ => "position",
            };
            let spec = match style {
                Style::Plain => String::new(),
                Style::Padded(w) => format!(":0{w}"),
                Style::Letters => ":A".into(),
            };
            text.push_str(&format!("{{{name}{spec}}}"));
        }
        // Braces in a code would be read as fields; such a code is left alone.
        let Ok(pattern) = Pattern::parse(&text) else {
            out.unmatched.extend(codes.iter().map(|(c, _)| c.to_string()));
            continue;
        };
        // A family spanning bay 1 to bay 90,000 is two codes that look alike,
        // not a rack.
        let cells: i64 = grid.bays as i64
            * grid.rows as i64
            * (1..=grid.levels).map(|l| grid.positions_at(l) as i64).sum::<i64>();
        if cells > MOST_CELLS {
            out.unmatched.extend(codes.iter().map(|(c, _)| c.to_string()));
            continue;
        }
        let by_name: HashMap<String, GridCell> = match pattern.names(&grid) {
            Ok(v) => v.into_iter().map(|(c, s)| (s, c)).collect(),
            Err(_) => {
                out.unmatched.extend(codes.iter().map(|(c, _)| c.to_string()));
                continue;
            }
        };
        let mut placed = vec![];
        for (code, _) in &codes {
            match by_name.get(*code) {
                Some(cell) => placed.push((code.to_string(), *cell)),
                None => out.unmatched.push(code.to_string()),
            }
        }
        if placed.is_empty() {
            continue;
        }
        placed.sort_by_key(|(_, c)| *c);
        let solid = codes.iter().filter(|(_, s)| *s).count() * 2 >= codes.len();
        let base = if solid { format!("Rack {head}") } else { head.clone() };
        let name = distinct(&mut taken, &base);
        out.places.push(Drafted { name, solid, pattern: text, grid, bins: placed });
    }
    out.unmatched.sort();
    out
}

/// The most cells the draft will give one place.
const MOST_CELLS: i64 = 20_000;

/// `Rack C`, then `Rack C (2)` for a second family with the same first part.
pub fn distinct(taken: &mut HashMap<String, usize>, base: &str) -> String {
    let n = taken.entry(base.to_string()).or_insert(0);
    *n += 1;
    if *n == 1 {
        base.to_string()
    } else {
        format!("{base} ({n})")
    }
}

// ---------------------------------------------------------------------------
// Two sides of one rack
// ---------------------------------------------------------------------------

/// The same rack, read as a face on each side numbered round it: half the
/// columns, the front's labels from the first bay and the back's on from the
/// front's last (`Grid::label_number`). A family whose codes run `E-01` to
/// `E-36` is a rack eighteen columns long with `E-36` behind `E-01`.
///
/// With an odd number of bays the front takes the one over, and the back's
/// last label is one no bin has. Nothing when there is only one bay to share,
/// or the grid already has two sides.
pub fn two_sides(grid: &Grid) -> Option<Grid> {
    if grid.bays < 2 || grid.sides != 1 {
        return None;
    }
    Some(Grid { bays: (grid.bays + 1) / 2, sides: 2, ..grid.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(bays: i32, levels: i32) -> Grid {
        Grid { bays, levels, ..Grid::single() }
    }

    #[test]
    fn a_rack_with_two_sides_is_numbered_round_it() {
        // Rack E: E-01 to E-18 along the front and E-19 to E-36 back along the
        // other side, one rack eighteen columns long.
        let p = Pattern::parse("E-{bay:02}-{level:02}").unwrap();
        let e = two_sides(&grid(36, 4)).unwrap();
        assert_eq!((e.bays, e.sides, e.first_bay), (18, 2, 1));
        let at = |bay, side| GridCell { bay, level: 1, row: 1, position: 1, side };
        let named = |c| p.render(&e, c).unwrap();
        // Behind a bay is the same column on the other side.
        assert_eq!((named(at(1, 1)), named(at(1, 2))), ("E-01-01".to_string(), "E-36-01".to_string()));
        assert_eq!((named(at(18, 1)), named(at(18, 2))), ("E-18-01".to_string(), "E-19-01".to_string()));
        // Each side reads left to right, in order, as you face it.
        assert_eq!(labels(&e, Some(&p)).0.first().map(String::as_str), Some("01"));
        let back = back_labels(&e, Some(&p));
        assert_eq!((back.len(), back[0].as_str(), back[17].as_str()), (18, "19", "36"));
        assert!(back_labels(&grid(36, 4), Some(&p)).is_empty(), "one side has no back");
        // Every code on the bin list has a cell, and no two share one.
        let names = p.names(&e).unwrap();
        assert_eq!(names.len(), 36 * 4);
        assert!(names.iter().any(|(c, n)| n == "E-36-04" && *c == GridCell { level: 4, ..at(1, 2) }));
        assert_eq!(whereabouts(&e, Some(&p), at(1, 2)), "back, bay 36, level 01");
        assert_eq!(whereabouts(&e, Some(&p), at(3, 1)), "front, bay 03, level 01");
        assert_eq!(whereabouts(&grid(36, 4), Some(&p), at(3, 1)), "bay 03, level 01", "one side needs no word for it");

        // An odd rack gives the front the bay over; the back's last label is
        // one no bin has.
        let nine = two_sides(&grid(9, 1)).unwrap();
        assert_eq!((nine.bays, nine.label_number(2, 5), nine.label_number(2, 1)), (5, 6, 10));
        // Numbered by twos, the back goes on by twos.
        let odd = two_sides(&Grid { first_bay: 1, bay_step: 2, ..grid(6, 1) }).unwrap();
        assert_eq!((odd.label_number(1, 3), odd.label_number(2, 3), odd.label_number(2, 1)), (5, 7, 11));
        assert!(two_sides(&grid(1, 3)).is_none(), "one bay has one side");
        assert!(two_sides(&e).is_none(), "and two sides are not split again");
        assert!(!e.contains(GridCell { side: 3, ..at(1, 1) }));
        assert!(Grid { sides: 3, ..grid(2, 1) }.problems().iter().any(|p| p.contains("one side or two")));
    }

    #[test]
    fn a_rack_numbered_from_the_right_reads_the_other_way() {
        // Rack C: C-01 at the front's right end, leftwards to C-16, round the
        // left end, and back along the other side to C-32, behind C-01.
        let p = Pattern::parse("C-{bay:02}-{level:02}").unwrap();
        let c = Grid { from_right: true, ..two_sides(&grid(32, 4)).unwrap() };
        let at = |bay, side| GridCell { bay, level: 1, row: 1, position: 1, side };
        let named = |cell| p.render(&c, cell).unwrap();
        assert_eq!((named(at(16, 1)), named(at(16, 2))), ("C-01-01".to_string(), "C-32-01".to_string()));
        assert_eq!((named(at(1, 1)), named(at(1, 2))), ("C-16-01".to_string(), "C-17-01".to_string()));
        // Each side reads as you face it: the front 16 down to 01, and the
        // back, whose left is the front's right end, 32 down to 17.
        let (front, _) = labels(&c, Some(&p));
        assert_eq!((front[0].as_str(), front[15].as_str()), ("16", "01"));
        let back = back_labels(&c, Some(&p));
        assert_eq!((back[0].as_str(), back[15].as_str()), ("32", "17"));
        assert_eq!(whereabouts(&c, Some(&p), at(16, 2)), "back, bay 32, level 01");
        // The same codes as from the left, each in the mirror of its column.
        let left = two_sides(&grid(32, 4)).unwrap();
        let mut ours: Vec<(String, GridCell)> = p.names(&c).unwrap().into_iter().map(|(cell, n)| (n, cell)).collect();
        let mut theirs: Vec<(String, GridCell)> = p.names(&left).unwrap().into_iter().map(|(cell, n)| (n, cell)).collect();
        ours.sort();
        theirs.sort();
        assert_eq!(ours.len(), theirs.len());
        for ((a, ca), (b, cb)) in ours.iter().zip(&theirs) {
            assert_eq!((a, ca.bay), (b, 17 - cb.bay), "{a} is mirrored");
            assert_eq!((ca.side, ca.level), (cb.side, cb.level));
        }

        // One side, by twos: the right end is the first.
        let odd = Grid { first_bay: 1, bay_step: 2, from_right: true, ..grid(3, 1) };
        assert_eq!(labels(&odd, None).0, ["5", "3", "1"]);
        assert!(back_labels(&odd, None).is_empty());
    }

    #[test]
    fn a_pattern_names_every_cell_and_refuses_to_name_two_alike() {
        let p = Pattern::parse("C-{bay:02}-{level:A}").unwrap();
        let g = Grid { first_bay: 1, bay_step: 2, ..grid(3, 2) };
        let names: Vec<String> = p.names(&g).unwrap().into_iter().map(|(_, n)| n).collect();
        assert_eq!(names, ["C-01-A", "C-01-B", "C-03-A", "C-03-B", "C-05-A", "C-05-B"]);
        assert!(Pattern::parse("C-{bay}").unwrap().names(&grid(3, 2)).unwrap_err().contains("{level}"));
        assert!(Pattern::parse("C-{shelf}").is_err());
        assert!(Pattern::parse("C-01").is_err(), "no fields names every cell alike");
        assert!(Pattern::parse("C-{bay:2}").is_err(), "pad with a leading zero");
        assert!(Pattern::parse("DOCK-{bay}").unwrap().names(&grid(6, 1)).is_ok(), "one level needs no {{level}}");
    }

    #[test]
    fn labels_read_as_the_rack_prints_them() {
        let p = Pattern::parse("C-{bay:02}-{level:A}").unwrap();
        let (bays, levels) = labels(&Grid { first_bay: 2, bay_step: 2, ..grid(3, 3) }, Some(&p));
        assert_eq!(bays, ["02", "04", "06"]);
        assert_eq!(levels, ["A", "B", "C"]);
        let (bays, _) = labels(&Grid { first_bay: 5, bay_step: -1, ..grid(3, 1) }, None);
        assert_eq!(bays, ["5", "4", "3"], "counting down");
    }

    #[test]
    fn a_child_moves_with_its_parent_and_turns_with_it() {
        // A building turned a quarter; a rack 10 cells along it, 2 in.
        let building = compose(&Frame::SITE, 100.0, 50.0, 0.0, 90.0);
        let rack = compose(&building, 10.0, 2.0, 0.0, 0.0);
        assert!((rack.x - 98.0).abs() < 1e-9 && (rack.y - 60.0).abs() < 1e-9, "{rack:?}");
        assert_eq!(rack.turn, 90.0);
        let corners = footprint(&rack, 4.0, 1.0, None);
        assert!((corners[1][0] - 98.0).abs() < 1e-9 && (corners[1][1] - 64.0).abs() < 1e-9, "{corners:?}");
        // A mezzanine up in the air carries its children up with it.
        let mezz = compose(&Frame::SITE, 0.0, 0.0, 6.0, 0.0);
        assert_eq!(compose(&mezz, 1.0, 1.0, 1.0, 0.0).z, 7.0);
    }

    #[test]
    fn frames_are_composed_from_the_site_down_and_a_loop_has_none() {
        let mut places = HashMap::new();
        places.insert(1, (None, 10.0, 0.0, 0.0, 0.0));
        places.insert(2, (Some(1), 5.0, 5.0, 0.0, 0.0));
        places.insert(3, (Some(2), 1.0, 0.0, 0.0, 0.0));
        places.insert(8, (Some(9), 0.0, 0.0, 0.0, 0.0));
        places.insert(9, (Some(8), 0.0, 0.0, 0.0, 0.0));
        let f = frames(&places);
        assert_eq!((f[&3].x, f[&3].y), (16.0, 5.0));
        assert!(!f.contains_key(&8) && !f.contains_key(&9));
    }

    #[test]
    fn an_l_shaped_outline_is_an_outline_and_a_bowtie_is_not() {
        let l = [0.0, 0.0, 60.0, 0.0, 60.0, 20.0, 25.0, 20.0, 25.0, 40.0, 0.0, 40.0];
        assert_eq!(outline_problem(&l), None);
        let bowtie = [0.0, 0.0, 10.0, 5.0, 10.0, 0.0, 0.0, 5.0];
        assert!(outline_problem(&bowtie).unwrap().contains("meets"));
        assert!(outline_problem(&[0.0, 0.0, 5.0, 0.0, 10.0, 0.0]).unwrap().contains("no area"));
        assert!(outline_problem(&[0.0, 0.0, 1.0, 1.0]).is_some());
    }

    fn codes(list: &[&str]) -> Vec<(String, bool)> {
        list.iter().map(|c| (c.to_string(), true)).collect()
    }

    #[test]
    fn a_draft_finds_a_rack_in_its_codes() {
        let mut bins: Vec<String> = vec![];
        for bay in [1, 3, 5, 9] {
            for level in 1..=4 {
                bins.push(format!("C-{bay:02}-{level}"));
            }
        }
        let refs: Vec<&str> = bins.iter().map(String::as_str).collect();
        let d = draft(&codes(&refs));
        assert_eq!(d.unmatched, Vec::<String>::new());
        let c = &d.places[0];
        assert_eq!(c.name, "Rack C");
        assert_eq!(c.pattern, "C-{bay:02}-{level}");
        // Odd bays only, and bay 7 missing: the grid still runs 1 to 9 by twos.
        assert_eq!((c.grid.bays, c.grid.first_bay, c.grid.bay_step, c.grid.levels), (5, 1, 2, 4));
        assert_eq!(c.bins.len(), 16);
        let nine = c.bins.iter().find(|(code, _)| code == "C-09-4").unwrap().1;
        assert_eq!(nine, GridCell { bay: 5, level: 4, row: 1, position: 1, side: 1 });
    }

    #[test]
    fn a_draft_keeps_families_apart_and_leaves_what_it_cannot_read() {
        let d = draft(&[
            ("A.1.1".to_string(), true),
            ("A.2.1".to_string(), true),
            ("A.2.2".to_string(), true),
            ("DOCK-1".to_string(), false),
            ("DOCK-2".to_string(), false),
            ("DOCK-6".to_string(), false),
            ("3PL".to_string(), false),
            ("ASSEMBLY-BIN".to_string(), false),
            ("K-1-A".to_string(), true),
            ("K-1-C".to_string(), true),
        ]);
        let by: HashMap<&str, &Drafted> = d.places.iter().map(|p| (p.name.as_str(), p)).collect();
        assert_eq!(by["Rack A"].pattern, "A.{bay}.{level}");
        assert_eq!(by["DOCK"].pattern, "DOCK-{bay}");
        assert!(!by["DOCK"].solid, "docks are floor");
        assert_eq!(by["DOCK"].grid.bays, 6, "1 to 6, with gaps");
        assert_eq!(by["Rack K"].pattern, "K-{bay}-{level:A}");
        assert_eq!(by["Rack K"].grid.levels, 3, "A to C");
        assert_eq!(d.unmatched, ["3PL", "ASSEMBLY-BIN"]);
    }

    #[test]
    fn a_draft_splits_a_bay_into_positions_where_the_codes_do() {
        let d = draft(&codes(&["B-01-1-1", "B-01-1-2", "B-01-1-3", "B-01-2-1", "B-02-2-1"]));
        let b = &d.places[0];
        assert_eq!(b.pattern, "B-{bay:02}-{level}-{position}");
        assert_eq!(b.grid.positions, vec![3, 1]);
        assert_eq!(b.bins.len(), 5);
    }

    #[test]
    fn a_code_the_pattern_would_spell_differently_is_not_forced_into_a_cell() {
        // `7` among `07` and `10`: the pattern pads, so `C-7-1` is not its name.
        let d = draft(&codes(&["C-07-1", "C-10-1", "C-7-1"]));
        assert_eq!(d.unmatched, ["C-7-1"]);
        assert_eq!(d.places[0].bins.len(), 2);
    }
}

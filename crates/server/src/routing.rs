//! Walking distance on the floor, and the order of stops that walks least.
//! D211, docs/picking-plan.md Proposals C and D.
//!
//! **Two questions, two methods.** How far is it from here to there is a
//! shortest path on the floor: A* between two points, or Dijkstra from one to
//! all. In what order should a walk visit its stops is a travelling-salesman
//! problem over those distances: exact for a dozen stops, a good heuristic
//! beyond. A* measures; it doesn't order.
//!
//! **The floor is the layout, gridded.** Every walk-through place is floor,
//! every solid place is in the way, and a point on the floor is walkable when
//! it is inside the first and outside all of the second. The grid is fine
//! enough for a narrow aisle to stay open: a quarter of a metre once the site
//! is measured, half a cell before. A diagonal step never cuts the corner of
//! something solid, so a route goes round the end of a rack, not through it.
//!
//! Pure: no database, so the geometry is tested on its own.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::layout::{Frame, GridCell};

pub type Pt = [f64; 2];

/// The floor as a grid of spots someone can stand on.
#[derive(Debug, Clone)]
pub struct Floor {
    /// The middle of node (0, 0), on the site.
    origin: Pt,
    /// How far apart neighbouring nodes are, in the site's cells.
    step: f64,
    w: usize,
    h: usize,
    open: Vec<bool>,
}

/// The most nodes a floor is gridded into; a bigger site is gridded coarser.
const MOST_NODES: usize = 400_000;

impl Floor {
    /// Grid the floor: walkable inside any of `walkways` and outside all of
    /// `solids`, nodes `step` apart. Nothing to walk on is no floor.
    pub fn build(walkways: &[Vec<Pt>], solids: &[Vec<Pt>], step: f64) -> Option<Floor> {
        let all: Vec<Pt> = walkways.iter().flatten().copied().collect();
        if all.is_empty() || !(step > 0.0) {
            return None;
        }
        let min_x = all.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        let min_y = all.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let max_x = all.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
        let max_y = all.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
        let mut step = step;
        let (w, h) = loop {
            let w = ((max_x - min_x) / step).ceil().max(1.0) as usize;
            let h = ((max_y - min_y) / step).ceil().max(1.0) as usize;
            if w * h <= MOST_NODES {
                break (w, h);
            }
            step *= 1.5;
        };
        let origin = [min_x + step / 2.0, min_y + step / 2.0];
        let mut open = vec![false; w * h];
        for j in 0..h {
            for i in 0..w {
                let p = [origin[0] + i as f64 * step, origin[1] + j as f64 * step];
                open[j * w + i] = walkways.iter().any(|r| inside(r, p)) && !solids.iter().any(|r| inside(r, p));
            }
        }
        Some(Floor { origin, step, w, h, open })
    }

    pub fn step(&self) -> f64 {
        self.step
    }

    /// Where a node is, on the site.
    pub fn at(&self, n: usize) -> Pt {
        let (i, j) = (n % self.w, n / self.w);
        [self.origin[0] + i as f64 * self.step, self.origin[1] + j as f64 * self.step]
    }

    pub fn is_open(&self, n: usize) -> bool {
        self.open.get(n).copied().unwrap_or(false)
    }

    /// The node a point is in, if it is on the grid at all.
    pub fn node_of(&self, p: Pt) -> Option<usize> {
        let fi = ((p[0] - self.origin[0]) / self.step).round();
        let fj = ((p[1] - self.origin[1]) / self.step).round();
        if fi < 0.0 || fj < 0.0 || fi >= self.w as f64 || fj >= self.h as f64 {
            return None;
        }
        Some(fj as usize * self.w + fi as usize)
    }

    /// Where to stand to reach a face: the first walkable node met stepping
    /// out from `p` along `out`, the way the face looks, no further than
    /// `reach`. **Only along the way it looks**: a face against a wall can't be
    /// reached from the aisle behind its rack, and nothing found is the
    /// honest answer.
    pub fn standing(&self, p: Pt, out: Pt, reach: f64) -> Option<usize> {
        let len = (out[0] * out[0] + out[1] * out[1]).sqrt();
        if len == 0.0 {
            return None;
        }
        let d = [out[0] / len, out[1] / len];
        let stride = self.step / 2.0;
        let mut k = 1.0;
        while k * stride <= reach + 1e-9 {
            let q = [p[0] + d[0] * k * stride, p[1] + d[1] * k * stride];
            if let Some(n) = self.node_of(q).filter(|&n| self.open[n]) {
                return Some(n);
            }
            k += 1.0;
        }
        None
    }

    /// The eight steps from a node, with their lengths. A diagonal is only
    /// taken when both squares it passes between are open, so no step clips
    /// the corner of something solid.
    fn steps(&self, n: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        let (i, j) = ((n % self.w) as isize, (n / self.w) as isize);
        let open = move |x: isize, y: isize| -> bool {
            x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h && self.open[y as usize * self.w + x as usize]
        };
        const MOVES: [(isize, isize); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
        MOVES.into_iter().filter_map(move |(dx, dy)| {
            let (x, y) = (i + dx, j + dy);
            if !open(x, y) {
                return None;
            }
            if dx != 0 && dy != 0 && !(open(i + dx, j) && open(i, j + dy)) {
                return None;
            }
            let cost = if dx != 0 && dy != 0 { std::f64::consts::SQRT_2 } else { 1.0 };
            Some((y as usize * self.w + x as usize, cost * self.step))
        })
    }

    /// The octile distance: what a walk would be with nothing in the way.
    fn guess(&self, a: usize, b: usize) -> f64 {
        let dx = ((a % self.w) as f64 - (b % self.w) as f64).abs();
        let dy = ((a / self.w) as f64 - (b / self.w) as f64).abs();
        (dx.max(dy) + (std::f64::consts::SQRT_2 - 1.0) * dx.min(dy)) * self.step
    }

    /// The shortest walk between two nodes, and its path: A*.
    pub fn path(&self, a: usize, b: usize) -> Option<(f64, Vec<usize>)> {
        if !self.is_open(a) || !self.is_open(b) {
            return None;
        }
        let mut best = vec![f64::INFINITY; self.open.len()];
        let mut came = vec![usize::MAX; self.open.len()];
        let mut heap = BinaryHeap::new();
        best[a] = 0.0;
        heap.push(Next { cost: self.guess(a, b), node: a });
        while let Some(Next { node, .. }) = heap.pop() {
            if node == b {
                let mut path = vec![b];
                let mut at = b;
                while at != a {
                    at = came[at];
                    path.push(at);
                }
                path.reverse();
                return Some((best[b], path));
            }
            let here = best[node];
            for (to, len) in self.steps(node) {
                let via = here + len;
                if via + 1e-12 < best[to] {
                    best[to] = via;
                    came[to] = node;
                    heap.push(Next { cost: via + self.guess(to, b), node: to });
                }
            }
        }
        None
    }

    /// The shortest walk from one node to each of `to`: Dijkstra, stopping once
    /// all of them are reached. Infinite for one that can't be.
    pub fn distances(&self, from: usize, to: &[usize]) -> Vec<f64> {
        let mut best = vec![f64::INFINITY; self.open.len()];
        if !self.is_open(from) {
            return vec![f64::INFINITY; to.len()];
        }
        let mut wanted: std::collections::HashSet<usize> = to.iter().copied().collect();
        let mut heap = BinaryHeap::new();
        best[from] = 0.0;
        heap.push(Next { cost: 0.0, node: from });
        while let Some(Next { cost, node }) = heap.pop() {
            if cost > best[node] {
                continue;
            }
            wanted.remove(&node);
            if wanted.is_empty() {
                break;
            }
            for (next, len) in self.steps(node) {
                let via = cost + len;
                if via + 1e-12 < best[next] {
                    best[next] = via;
                    heap.push(Next { cost: via, node: next });
                }
            }
        }
        to.iter().map(|&t| best[t]).collect()
    }

    /// A path as points on the site, with the bends kept and the straight
    /// runs between them left out. A point where the path doubles back is a
    /// bend, and is kept: it is where the stop is.
    pub fn line(&self, path: &[usize]) -> Vec<Pt> {
        let pts: Vec<Pt> = path.iter().map(|&n| self.at(n)).collect();
        let mut out: Vec<Pt> = vec![];
        for (k, p) in pts.iter().enumerate() {
            if k > 0 && k + 1 < pts.len() {
                let (a, b) = (pts[k - 1], pts[k + 1]);
                let cross = (p[0] - a[0]) * (b[1] - p[1]) - (p[1] - a[1]) * (b[0] - p[0]);
                let onward = (p[0] - a[0]) * (b[0] - p[0]) + (p[1] - a[1]) * (b[1] - p[1]);
                if cross.abs() < 1e-9 && onward > 0.0 {
                    continue;
                }
            }
            out.push(*p);
        }
        out
    }
}

#[derive(PartialEq)]
struct Next {
    cost: f64,
    node: usize,
}
impl Eq for Next {}
impl Ord for Next {
    // The heap is a max-heap: the cheapest is the greatest.
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal).then(other.node.cmp(&self.node))
    }
}
impl PartialOrd for Next {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Whether a point is inside a polygon: a ray cast to the right crosses its
/// edges an odd number of times.
pub fn inside(ring: &[Pt], p: Pt) -> bool {
    let mut odd = false;
    let n = ring.len();
    for k in 0..n {
        let (a, b) = (ring[k], ring[(k + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                odd = !odd;
            }
        }
    }
    odd
}

/// The middle of a bin's face at floor level, and which way the face looks:
/// the front's bins open onto the aisle in front, the back's onto the aisle
/// behind. Bins sharing a bay share where to stand, at the middle of the bay.
pub fn face(frame: &Frame, length: f64, depth: f64, bays: i32, cell: GridCell) -> (Pt, Pt) {
    let along = (cell.bay as f64 - 0.5) * length / bays.max(1) as f64;
    let (v, out) = if cell.side == 2 { (depth, 1.0) } else { (0.0, -1.0) };
    let p = frame.point(along, v);
    let o = frame.point(along, v + out);
    (p, [o[0] - p[0], o[1] - p[1]])
}

// ---------------------------------------------------------------------------
// The order of stops
// ---------------------------------------------------------------------------

/// The most stops, besides the start, ordered exactly.
pub const EXACT_UP_TO: usize = 12;

/// The order to visit every stop in that walks least, starting at `start`
/// and, when `closed`, coming back to it. `cost[i][j]` is the walk from `i` to
/// `j`. Returns every index once, `start` first.
pub fn order(cost: &[Vec<f64>], start: usize, closed: bool) -> Vec<usize> {
    let n = cost.len();
    if n <= 2 {
        let mut out = vec![start];
        out.extend((0..n).filter(|&k| k != start));
        return out;
    }
    if n - 1 <= EXACT_UP_TO {
        exact(cost, start, closed)
    } else {
        improved(cost, start, closed)
    }
}

/// How far a walk in this order is.
pub fn length(cost: &[Vec<f64>], route: &[usize], closed: bool) -> f64 {
    let mut sum: f64 = route.windows(2).map(|w| cost[w[0]][w[1]]).sum();
    if closed && route.len() > 1 {
        sum += cost[route[route.len() - 1]][route[0]];
    }
    sum
}

/// Held-Karp: the best walk ending at each stop over each set of stops, built
/// up from the start. Exact, and quick for a dozen stops.
fn exact(cost: &[Vec<f64>], start: usize, closed: bool) -> Vec<usize> {
    let others: Vec<usize> = (0..cost.len()).filter(|&k| k != start).collect();
    let m = others.len();
    let full = (1usize << m) - 1;
    let mut best = vec![f64::INFINITY; (1 << m) * m];
    let mut prev = vec![usize::MAX; (1 << m) * m];
    for j in 0..m {
        best[(1 << j) * m + j] = cost[start][others[j]];
    }
    for set in 1..=full {
        for j in 0..m {
            if set & (1 << j) == 0 {
                continue;
            }
            let here = best[set * m + j];
            if !here.is_finite() {
                continue;
            }
            for k in 0..m {
                if set & (1 << k) != 0 {
                    continue;
                }
                let next = set | (1 << k);
                let via = here + cost[others[j]][others[k]];
                if via < best[next * m + k] {
                    best[next * m + k] = via;
                    prev[next * m + k] = j;
                }
            }
        }
    }
    let end = (0..m)
        .min_by(|&a, &b| {
            let fa = best[full * m + a] + if closed { cost[others[a]][start] } else { 0.0 };
            let fb = best[full * m + b] + if closed { cost[others[b]][start] } else { 0.0 };
            fa.partial_cmp(&fb).unwrap_or(Ordering::Equal)
        })
        .unwrap_or(0);
    let mut route = vec![];
    let (mut set, mut j) = (full, end);
    while j != usize::MAX {
        route.push(others[j]);
        let p = prev[set * m + j];
        set &= !(1 << j);
        j = p;
    }
    route.push(start);
    route.reverse();
    route
}

/// The quick way, for a walk too long to solve exactly: a good first route,
/// polished with 2-opt and Or-opt until nothing shortens it, then shaken and
/// polished again for as long as the budget allows, keeping the best. Within a
/// few percent of the best for the thirty-odd stops a run has.
fn improved(cost: &[Vec<f64>], start: usize, closed: bool) -> Vec<usize> {
    let budget = std::time::Instant::now() + std::time::Duration::from_millis(20);
    let mut best = polish(cost, inserted(cost, start, closed), closed, budget);
    let other = polish(cost, nearest(cost, start), closed, budget);
    if length(cost, &other, closed) < length(cost, &best, closed) {
        best = other;
    }
    // Shake: cut the walk in three places and swap the middle pieces, which
    // no single 2-opt or Or-opt move can undo, then polish again.
    let mut seed = 0x9e37_79b9_7f4a_7c15u64 ^ cost.len() as u64;
    let mut roll = |below: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % below.max(1)
    };
    let n = best.len();
    for _ in 0..400 {
        if n < 5 || std::time::Instant::now() > budget {
            break;
        }
        let mut cuts = [1 + roll(n - 1), 1 + roll(n - 1), 1 + roll(n - 1)];
        cuts.sort();
        if cuts[0] == cuts[1] || cuts[1] == cuts[2] {
            continue;
        }
        let mut shaken = best[..cuts[0]].to_vec();
        shaken.extend_from_slice(&best[cuts[1]..cuts[2]]);
        shaken.extend_from_slice(&best[cuts[0]..cuts[1]]);
        shaken.extend_from_slice(&best[cuts[2]..]);
        let tried = polish(cost, shaken, closed, budget);
        if length(cost, &tried, closed) + 1e-9 < length(cost, &best, closed) {
            best = tried;
        }
    }
    best
}

/// Nearest insertion: the stop nearest the route so far, where it adds least.
fn inserted(cost: &[Vec<f64>], start: usize, closed: bool) -> Vec<usize> {
    let n = cost.len();
    let mut route = vec![start];
    let mut left: Vec<usize> = (0..n).filter(|&k| k != start).collect();
    while !left.is_empty() {
        let (pick, _) = left
            .iter()
            .enumerate()
            .map(|(i, &k)| (i, route.iter().map(|&r| cost[r][k]).fold(f64::INFINITY, f64::min)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
            .unwrap();
        let k = left.swap_remove(pick);
        let mut best = (route.len(), f64::INFINITY);
        for at in 1..=route.len() {
            let before = route[at - 1];
            let added = if at == route.len() {
                cost[before][k] + if closed { cost[k][start] - cost[before][start] } else { 0.0 }
            } else {
                cost[before][k] + cost[k][route[at]] - cost[before][route[at]]
            };
            if added < best.1 {
                best = (at, added);
            }
        }
        route.insert(best.0, k);
    }
    route
}

/// Nearest neighbour: always on to the closest stop not yet visited.
fn nearest(cost: &[Vec<f64>], start: usize) -> Vec<usize> {
    let mut route = vec![start];
    let mut left: Vec<usize> = (0..cost.len()).filter(|&k| k != start).collect();
    while !left.is_empty() {
        let here = *route.last().unwrap();
        let (i, _) = left
            .iter()
            .enumerate()
            .min_by(|a, b| cost[here][*a.1].partial_cmp(&cost[here][*b.1]).unwrap_or(Ordering::Equal))
            .unwrap();
        route.push(left.swap_remove(i));
    }
    route
}

/// 2-opt and Or-opt until neither shortens the walk, or the time is up: a
/// walk of a hundred lines is polished as far as the budget goes, not to the
/// end. The start stays first.
fn polish(cost: &[Vec<f64>], mut route: Vec<usize>, closed: bool, budget: std::time::Instant) -> Vec<usize> {
    for _ in 0..50 {
        if std::time::Instant::now() > budget {
            break;
        }
        let mut better = false;
        // 2-opt: reverse a stretch.
        for i in 1..route.len().saturating_sub(1) {
            if std::time::Instant::now() > budget {
                break;
            }
            for k in i + 1..route.len() {
                let mut tried = route.clone();
                tried[i..=k].reverse();
                if length(cost, &tried, closed) + 1e-9 < length(cost, &route, closed) {
                    route = tried;
                    better = true;
                }
            }
        }
        // Or-opt: move a stretch of one to three stops elsewhere, either way round.
        for len in 1..=3usize {
            let mut i = 1;
            while i + len <= route.len() && std::time::Instant::now() <= budget {
                let now = length(cost, &route, closed);
                let mut rest = route.clone();
                let piece: Vec<usize> = rest.drain(i..i + len).collect();
                let mut found = None;
                'places: for at in 1..=rest.len() {
                    for flipped in [false, true] {
                        if at == i && !flipped {
                            continue;
                        }
                        let mut tried = rest.clone();
                        if flipped {
                            tried.splice(at..at, piece.iter().rev().copied());
                        } else {
                            tried.splice(at..at, piece.iter().copied());
                        }
                        if length(cost, &tried, closed) + 1e-9 < now {
                            found = Some(tried);
                            break 'places;
                        }
                    }
                }
                if let Some(t) = found {
                    route = t;
                    better = true;
                }
                i += 1;
            }
        }
        if !better {
            break;
        }
    }
    route
}

// ---------------------------------------------------------------------------
// A walk, planned
// ---------------------------------------------------------------------------

/// A walk's stops put in order, against the order they were typed in.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// The stops reached, by their index in the typed order, in walking order.
    pub order: Vec<usize>,
    /// The stops that can't be reached from where the walk starts.
    pub unreachable: Vec<usize>,
    /// The walk in this order, and in the typed order over the same stops,
    /// from the same start to the same end. In the site's cells.
    pub walked: f64,
    pub typed: f64,
    /// Where it goes, on the site.
    pub path: Vec<Pt>,
}

/// Plan a walk over `stops`, nodes on the floor in the order somebody typed.
///
/// **From the same place, both ways.** With a `base` (the packing bench), the
/// walk starts there and comes back. Without one it starts at the typed
/// order's first stop and ends wherever its last one is. Either way the typed
/// order is measured from the same start under the same rule, so the
/// difference between the two numbers is the route and nothing else.
pub fn plan(floor: &Floor, base: Option<usize>, stops: &[usize]) -> Planned {
    let start = base.or_else(|| stops.first().copied());
    let Some(start) = start else {
        return Planned { order: vec![], unreachable: vec![], walked: 0.0, typed: 0.0, path: vec![] };
    };
    let closed = base.is_some();
    // Nodes: the start, then each stop.
    let mut nodes = vec![start];
    nodes.extend_from_slice(stops);
    let from_start = floor.distances(start, &nodes);
    let (reached, unreachable): (Vec<usize>, Vec<usize>) = (0..stops.len()).partition(|&k| from_start[k + 1].is_finite());
    // The matrix over the start and the stops it can reach.
    let mine: Vec<usize> = std::iter::once(start).chain(reached.iter().map(|&k| stops[k])).collect();
    let cost: Vec<Vec<f64>> = mine.iter().map(|&a| floor.distances(a, &mine)).collect();
    let route = order(&cost, 0, closed);
    // The typed order is the reached stops as they were typed; without a base
    // the first of them is the start itself.
    let typed_route: Vec<usize> = (0..mine.len()).collect();
    let walked = length(&cost, &route, closed);
    let typed = length(&cost, &typed_route, closed);
    let mut legs: Vec<usize> = route.iter().map(|&i| mine[i]).collect();
    if closed {
        legs.push(start);
    }
    let mut path: Vec<usize> = vec![];
    for w in legs.windows(2) {
        if let Some((_, leg)) = floor.path(w[0], w[1]) {
            if !path.is_empty() {
                path.pop();
            }
            path.extend(leg);
        }
    }
    Planned {
        order: route.into_iter().skip(1).map(|i| reached[i - 1]).collect(),
        unreachable,
        walked,
        typed,
        path: floor.line(&path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, l: f64, d: f64) -> Vec<Pt> {
        vec![[x, y], [x + l, y], [x + l, y + d], [x, y + d]]
    }

    #[test]
    fn an_empty_floor_is_walked_in_a_straight_line() {
        let floor = Floor::build(&[rect(0.0, 0.0, 10.0, 10.0)], &[], 0.5).unwrap();
        let a = floor.node_of([0.25, 0.25]).unwrap();
        let b = floor.node_of([3.25, 1.25]).unwrap();
        let (d, path) = floor.path(a, b).unwrap();
        // Six steps across and two up: two diagonals and four straight.
        let octile = (4.0 + 2.0 * std::f64::consts::SQRT_2) * 0.5;
        assert!((d - octile).abs() < 1e-9, "{d} against {octile}");
        assert_eq!(path.first(), Some(&a));
        assert_eq!(path.last(), Some(&b));
        assert!((floor.distances(a, &[b])[0] - d).abs() < 1e-9, "Dijkstra and A* agree");
    }

    #[test]
    fn a_walk_goes_round_a_rack_not_through_it() {
        // A rack across the middle, open at both ends.
        let rack = rect(2.0, 4.0, 6.0, 1.0);
        let floor = Floor::build(&[rect(0.0, 0.0, 10.0, 10.0)], &[rack.clone()], 0.5).unwrap();
        let a = floor.node_of([5.25, 2.75]).unwrap();
        let b = floor.node_of([5.25, 6.25]).unwrap();
        let (d, path) = floor.path(a, b).unwrap();
        assert!(d > 3.5 + 2.0, "round an end, not 3.5 straight through: {d}");
        for w in path.windows(2) {
            let (p, q) = (floor.at(w[0]), floor.at(w[1]));
            assert!(!inside(&rack, p), "a step inside the rack at {p:?}");
            // A diagonal's midpoint would be in the rack if it cut a corner.
            assert!(!inside(&rack, [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0]), "a corner cut between {p:?} and {q:?}");
        }
    }

    #[test]
    fn a_face_is_reached_from_its_own_aisle_or_not_at_all() {
        // A two-sided rack whose front is against the bottom wall.
        let frame = Frame { x: 1.0, y: 0.0, z: 0.0, turn: 0.0 };
        let rack = rect(1.0, 0.0, 4.0, 2.0);
        let floor = Floor::build(&[rect(0.0, 0.0, 8.0, 6.0)], &[rack], 0.5).unwrap();
        let cell = |side| GridCell { bay: 2, level: 1, row: 1, position: 1, side };
        let (front, out) = face(&frame, 4.0, 2.0, 4, cell(1));
        assert_eq!((front, out), ([2.5, 0.0], [0.0, -1.0]));
        assert_eq!(floor.standing(front, out, 1.0), None, "against the wall: no aisle in front");
        let (back, out) = face(&frame, 4.0, 2.0, 4, cell(2));
        assert_eq!((back, out), ([2.5, 2.0], [0.0, 1.0]));
        let stand = floor.standing(back, out, 1.0).expect("the aisle behind");
        assert!(floor.at(stand)[1] > 2.0, "in the aisle, not in the rack: {:?}", floor.at(stand));
    }

    #[test]
    fn a_turned_rack_faces_where_it_is_turned() {
        let frame = Frame { x: 5.0, y: 2.0, z: 0.0, turn: 90.0 };
        let (p, out) = face(&frame, 4.0, 1.0, 4, GridCell { bay: 1, level: 1, row: 1, position: 1, side: 1 });
        assert!((p[0] - 5.0).abs() < 1e-9 && (p[1] - 2.5).abs() < 1e-9, "{p:?}");
        assert!((out[0] - 1.0).abs() < 1e-9 && out[1].abs() < 1e-9, "out to the right: {out:?}");
    }

    /// Every order, tried: the truth the solvers are held to.
    fn brute(cost: &[Vec<f64>], start: usize, closed: bool) -> f64 {
        fn perms(items: &mut Vec<usize>, k: usize, out: &mut Vec<Vec<usize>>) {
            if k == items.len() {
                out.push(items.clone());
                return;
            }
            for i in k..items.len() {
                items.swap(k, i);
                perms(items, k + 1, out);
                items.swap(k, i);
            }
        }
        let mut others: Vec<usize> = (0..cost.len()).filter(|&k| k != start).collect();
        let mut all = vec![];
        perms(&mut others, 0, &mut all);
        all.into_iter()
            .map(|p| {
                let mut r = vec![start];
                r.extend(p);
                length(cost, &r, closed)
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// Random points on a plane, and the distances between them.
    fn scattered(n: usize, seed: u64) -> Vec<Vec<f64>> {
        let mut s = seed;
        let mut next = || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 33) as f64 / (1u64 << 31) as f64 * 50.0
        };
        let pts: Vec<Pt> = (0..n).map(|_| [next(), next()]).collect();
        pts.iter().map(|a| pts.iter().map(|b| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()).collect()).collect()
    }

    #[test]
    fn the_exact_order_is_the_best_there_is() {
        for seed in 0..40 {
            for closed in [true, false] {
                let cost = scattered(6 + (seed as usize % 3), seed);
                let route = order(&cost, 0, closed);
                assert_eq!(route[0], 0, "it starts where it was asked to");
                let mut seen = route.clone();
                seen.sort();
                assert_eq!(seen, (0..cost.len()).collect::<Vec<_>>(), "every stop once");
                let best = brute(&cost, 0, closed);
                assert!((length(&cost, &route, closed) - best).abs() < 1e-9, "seed {seed}, closed {closed}");
            }
        }
    }

    #[test]
    fn the_quick_order_is_close_to_the_best() {
        for seed in 0..40 {
            for closed in [true, false] {
                let cost = scattered(8, seed + 100);
                let route = improved(&cost, 0, closed);
                assert_eq!(route[0], 0);
                let best = brute(&cost, 0, closed);
                let got = length(&cost, &route, closed);
                assert!(got <= best * 1.05 + 1e-9, "seed {seed}, closed {closed}: {got} against {best}");
            }
        }
        // And it handles a walk too long to solve exactly.
        let cost = scattered(40, 7);
        let route = order(&cost, 3, false);
        assert_eq!(route[0], 3);
        assert_eq!(route.len(), 40);
    }

    #[test]
    fn a_long_walk_is_ordered_within_its_budget() {
        // A hundred stops, as a busy morning's walk might have: every device
        // reads it again after every pick, so it must stay quick.
        let cost = scattered(100, 11);
        let began = std::time::Instant::now();
        let route = order(&cost, 0, true);
        let took = began.elapsed();
        let mut seen = route.clone();
        seen.sort();
        assert_eq!(seen, (0..100).collect::<Vec<_>>(), "every stop once");
        assert!(took < std::time::Duration::from_millis(250), "ordered in {took:?}");
        assert!(length(&cost, &route, true) < length(&cost, &(0..100).collect::<Vec<_>>(), true), "better than as given");
    }

    #[test]
    fn a_walk_is_planned_against_the_order_it_was_typed_in() {
        // Two rows of racks, an aisle between, a bench at the left end.
        let floor = Floor::build(
            &[rect(0.0, 0.0, 20.0, 10.0)],
            &[rect(2.0, 2.0, 16.0, 1.0), rect(2.0, 6.0, 16.0, 1.0)],
            0.5,
        )
        .unwrap();
        let at = |x: f64, y: f64| floor.node_of([x, y]).unwrap();
        let bench = at(0.75, 4.75);
        // Typed to zigzag: far end, near end, far end, near end.
        let stops = [at(16.25, 4.25), at(3.25, 4.25), at(15.25, 5.25), at(4.25, 5.25)];
        let p = plan(&floor, Some(bench), &stops);
        assert_eq!(p.unreachable, Vec::<usize>::new());
        let mut seen = p.order.clone();
        seen.sort();
        assert_eq!(seen, vec![0, 1, 2, 3], "every stop once");
        assert!(p.walked < p.typed * 0.7, "the near end, then the far, beats the zigzag: {} against {}", p.walked, p.typed);
        assert_eq!(floor.node_of(p.path[0]), Some(bench), "from the bench");
        assert_eq!(floor.node_of(*p.path.last().unwrap()), Some(bench), "and back");

        // With no bench: from the first stop typed, both ways, ending anywhere.
        let open = plan(&floor, None, &stops);
        assert_eq!(open.order[0], 0, "it starts where the typed order starts");
        assert!(open.walked <= open.typed + 1e-9);
    }

    #[test]
    fn a_path_that_doubles_back_keeps_the_point_it_turns_at() {
        let floor = Floor::build(&[rect(0.0, 0.0, 10.0, 2.0)], &[], 0.5).unwrap();
        let at = |x: f64| floor.node_of([x, 0.25]).unwrap();
        let p = plan(&floor, Some(at(0.25)), &[at(5.25)]);
        assert_eq!(p.path, vec![[0.25, 0.25], [5.25, 0.25], [0.25, 0.25]], "out to the stop and back");
    }

    #[test]
    fn a_stop_behind_a_wall_is_left_off_the_walk() {
        // A closed room in the corner, with a stop inside it.
        let floor = Floor::build(
            &[rect(0.0, 0.0, 10.0, 10.0)],
            &[rect(6.0, 6.0, 4.0, 0.5), rect(6.0, 6.0, 0.5, 4.0)],
            0.5,
        )
        .unwrap();
        let at = |x: f64, y: f64| floor.node_of([x, y]).unwrap();
        let p = plan(&floor, Some(at(0.25, 0.25)), &[at(3.25, 3.25), at(8.25, 8.25)]);
        assert_eq!((p.order.clone(), p.unreachable.clone()), (vec![0], vec![1]));
    }
}

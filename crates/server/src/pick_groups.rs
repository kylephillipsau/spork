//! Which waiting orders are picked together, and who walks them. D230.
//!
//! Picking on paper, a batch of orders is shared out between the people free
//! to pick. Orders whose shelves are near each other belong on one walk, and
//! so does an item two orders both want. This works out the walks.
//!
//! **Trips, then pickers.** A trip is one walk from the bench and back, with
//! at most so many orders on it (what a trolley takes); a picker's group is
//! the trips they walk. Trips are made by savings, the classic batching
//! heuristic: every order starts as its own trip, and the two trips that walk
//! least together are merged, again and again, while they fit. Unless the
//! trips are capped, the cap is the batch shared evenly between the pickers.
//! Trips then go to pickers longest first, each to whoever has least so far.
//!
//! **A shelf two trips want is walked to once, when asked** (`gather`). An
//! item on orders in different trips is taken by one of them for all of
//! those orders, and sorted to its orders at the bench: the trip that loses
//! least by keeping it, so the others save the most walking.
//!
//! Pure: distances in, plan out. The floor, the bins and the orders are the
//! caller's (`to_pick`); the order of a trip's stops is [`routing::order`]'s.

use std::collections::{BTreeSet, HashMap};

use crate::routing;

/// One thing to take from a shelf for an order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pick {
    /// Which order it is for, by index.
    pub order: usize,
    /// Where a picker stands to take it: a node of the cost matrix, never 0.
    /// `None` when its bin isn't on the layout or can't be reached.
    pub stop: Option<usize>,
    /// Which shelf and product it is. Picks of the same one, for different
    /// orders, are the same walk to the same shelf.
    pub what: usize,
}

/// How the batch is shared out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// People picking, at least one.
    pub pickers: usize,
    /// The most orders one trip takes, when there is a most.
    pub per_trip: Option<usize>,
    /// A shelf wanted by orders on different trips is walked to by one.
    pub gather: bool,
    /// What a stop costs beside walking, in the matrix's units: the time it
    /// takes to find and take something, said as a distance walked in it.
    /// Weighs a trip of many stops against a trip of long walks when trips
    /// are shared between pickers.
    pub stop_cost: f64,
}

/// One walk.
#[derive(Debug, Clone, PartialEq)]
pub struct Trip {
    /// The orders it carries back to the bench, by index, ascending.
    pub orders: Vec<usize>,
    /// Its stops in walking order, each with the picks taken there, by their
    /// index in the caller's picks. A pick for an order not in `orders` is
    /// gathered for another trip's order.
    pub stops: Vec<(usize, Vec<usize>)>,
    /// Picks whose bin isn't on the floor, walked last in the order given.
    pub off_route: Vec<usize>,
    /// From the bench round its stops and back, in the matrix's units.
    pub walked: f64,
}

/// Every picker's trips. A picker with nothing to do has none.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub pickers: Vec<Vec<Trip>>,
}

/// Plan the batch. `cost` is the walk between nodes, node 0 being the bench
/// trips start and end at; a caller with no bench on the floor makes node 0
/// nothing from anywhere, so a trip starts and ends where it likes.
pub fn plan(cost: &[Vec<f64>], orders: usize, picks: &[Pick], settings: &Settings) -> Plan {
    let pickers = settings.pickers.max(1);
    let cap = settings.per_trip.unwrap_or_else(|| orders.div_ceil(pickers)).max(1);
    let mut walks = Walks { cost, memo: HashMap::new() };

    // Which stops each order needs.
    let mut needs: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); orders];
    for p in picks {
        if let Some(s) = p.stop {
            needs[p.order].insert(s);
        }
    }
    let stops_of = |trip: &BTreeSet<usize>| -> BTreeSet<usize> { trip.iter().flat_map(|&o| needs[o].iter().copied()).collect() };

    // Savings: merge the two trips that walk least together, while they fit.
    // Past the point where merging saves nothing, merge only while there are
    // more trips than people to walk them.
    let mut trips: Vec<BTreeSet<usize>> = (0..orders).map(|o| BTreeSet::from([o])).collect();
    loop {
        let mut best: Option<(f64, usize, usize)> = None;
        for a in 0..trips.len() {
            for b in a + 1..trips.len() {
                if trips[a].len() + trips[b].len() > cap {
                    continue;
                }
                let (sa, sb) = (stops_of(&trips[a]), stops_of(&trips[b]));
                let both: BTreeSet<usize> = sa.union(&sb).copied().collect();
                let saved = walks.length(&sa) + walks.length(&sb) - walks.length(&both);
                if saved <= 1e-9 && trips.len() <= pickers {
                    continue;
                }
                if best.is_none_or(|(s, _, _)| saved > s + 1e-9) {
                    best = Some((saved, a, b));
                }
            }
        }
        let Some((_, a, b)) = best else { break };
        let merged: BTreeSet<usize> = trips[a].union(&trips[b]).copied().collect();
        trips.remove(b);
        trips[a] = merged;
    }

    // Who takes each pick: the trip carrying its order, unless gathered.
    let trip_of_order: Vec<usize> = {
        let mut t = vec![0; orders];
        for (i, trip) in trips.iter().enumerate() {
            for &o in trip {
                t[o] = i;
            }
        }
        t
    };
    let mut taker: Vec<usize> = picks.iter().map(|p| trip_of_order[p.order]).collect();
    if settings.gather {
        gather(&mut walks, picks, &mut taker);
    }

    // Each trip's walk, in order.
    let mut made: Vec<Trip> = trips
        .iter()
        .enumerate()
        .map(|(t, orders)| {
            let mine: Vec<usize> = (0..picks.len()).filter(|&i| taker[i] == t).collect();
            let at: BTreeSet<usize> = mine.iter().filter_map(|&i| picks[i].stop).collect();
            let (route, walked) = walks.route(&at);
            Trip {
                orders: orders.iter().copied().collect(),
                stops: route
                    .into_iter()
                    .map(|s| (s, mine.iter().copied().filter(|&i| picks[i].stop == Some(s)).collect()))
                    .collect(),
                off_route: mine.iter().copied().filter(|&i| picks[i].stop.is_none()).collect(),
                walked,
            }
        })
        .collect();

    // Longest first, each to whoever has least so far.
    let work = |t: &Trip| t.walked + settings.stop_cost * (t.stops.len() + t.off_route.len()) as f64;
    made.sort_by(|x, y| work(y).total_cmp(&work(x)).then_with(|| x.orders.cmp(&y.orders)));
    let mut out: Vec<Vec<Trip>> = vec![vec![]; pickers];
    let mut load = vec![0.0f64; pickers];
    for trip in made {
        let p = (0..pickers).min_by(|&a, &b| load[a].total_cmp(&load[b])).unwrap_or(0);
        load[p] += work(&trip);
        out[p].push(trip);
    }
    Plan { pickers: out }
}

/// Each shelf wanted by more than one trip goes to the trip that loses least
/// by keeping it: the one that stops there anyway, or whose walk the stop
/// adds least to. The others' walks drop the stop when nothing else of
/// theirs is there. A shelf off the floor costs nothing anyone can measure,
/// so the first trip wanting it keeps it: still one walk to it, not several.
fn gather(walks: &mut Walks, picks: &[Pick], taker: &mut [usize]) {
    let mut whats: Vec<usize> = picks.iter().map(|p| p.what).collect();
    whats.sort_unstable();
    whats.dedup();
    for what in whats {
        let of: Vec<usize> = (0..picks.len()).filter(|&i| picks[i].what == what).collect();
        let mut wanting: Vec<usize> = of.iter().map(|&i| taker[i]).collect();
        wanting.sort_unstable();
        wanting.dedup();
        if wanting.len() < 2 {
            continue;
        }
        // What each trip's walk is with and without this shelf's picks.
        let stops = |taker: &[usize], t: usize, without: bool| -> BTreeSet<usize> {
            (0..picks.len())
                .filter(|&i| taker[i] == t && !(without && picks[i].what == what))
                .filter_map(|i| picks[i].stop)
                .collect()
        };
        let keeper = *wanting
            .iter()
            .min_by(|&&a, &&b| {
                let loss = |t: usize, w: &mut Walks| w.length(&stops(taker, t, false)) - w.length(&stops(taker, t, true));
                let (la, lb) = (loss(a, walks), loss(b, walks));
                la.total_cmp(&lb).then(a.cmp(&b))
            })
            .unwrap_or(&wanting[0]);
        for i in of {
            taker[i] = keeper;
        }
    }
}

/// Walks round sets of stops from the bench, each worked out once.
struct Walks<'a> {
    cost: &'a [Vec<f64>],
    memo: HashMap<Vec<usize>, (Vec<usize>, f64)>,
}

impl Walks<'_> {
    fn length(&mut self, stops: &BTreeSet<usize>) -> f64 {
        self.route(stops).1
    }

    /// The stops in walking order from the bench and back, and how far that is.
    fn route(&mut self, stops: &BTreeSet<usize>) -> (Vec<usize>, f64) {
        let key: Vec<usize> = stops.iter().copied().collect();
        if let Some(known) = self.memo.get(&key) {
            return known.clone();
        }
        let nodes: Vec<usize> = std::iter::once(0).chain(key.iter().copied()).collect();
        let sub: Vec<Vec<f64>> = nodes.iter().map(|&a| nodes.iter().map(|&b| self.cost[a][b]).collect()).collect();
        let order = routing::order(&sub, 0, true);
        let walked = routing::length(&sub, &order, true);
        let found = (order.into_iter().skip(1).map(|i| nodes[i]).collect(), walked);
        self.memo.insert(key, found.clone());
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bench at 0 and stops along a line, each a step further out: node k
    /// stands k along it, so a walk to k and back is 2k.
    fn line(n: usize) -> Vec<Vec<f64>> {
        (0..n).map(|a| (0..n).map(|b| (a as f64 - b as f64).abs()).collect()).collect()
    }

    fn pick(order: usize, stop: usize, what: usize) -> Pick {
        Pick { order, stop: Some(stop), what }
    }

    fn settings(pickers: usize, per_trip: Option<usize>, gather: bool) -> Settings {
        Settings { pickers, per_trip, gather, stop_cost: 0.0 }
    }

    fn orders_of(plan: &Plan) -> Vec<Vec<Vec<usize>>> {
        plan.pickers.iter().map(|p| p.iter().map(|t| t.orders.clone()).collect()).collect()
    }

    #[test]
    fn one_picker_walks_every_order_in_one_trip() {
        let cost = line(6);
        let picks = [pick(0, 5, 0), pick(1, 2, 1), pick(2, 4, 2)];
        let plan = plan(&cost, 3, &picks, &settings(1, None, false));
        assert_eq!(orders_of(&plan), vec![vec![vec![0, 1, 2]]]);
        let trip = &plan.pickers[0][0];
        assert_eq!(trip.walked, 10.0, "out to 5 and back, past 2 and 4 on the way");
        let stops: Vec<usize> = trip.stops.iter().map(|s| s.0).collect();
        assert!(stops == vec![2, 4, 5] || stops == vec![5, 4, 2], "{stops:?}");
    }

    #[test]
    fn two_pickers_take_the_near_orders_and_the_far_ones() {
        // Two orders by the bench, two at the far end: each picker gets a pair
        // that is near each other, not one near and one far.
        let cost = line(12);
        let picks = [pick(0, 1, 0), pick(1, 2, 1), pick(2, 10, 2), pick(3, 11, 3)];
        let plan = plan(&cost, 4, &picks, &settings(2, None, false));
        let mut groups: Vec<Vec<usize>> = plan.pickers.iter().flat_map(|p| p.iter().map(|t| t.orders.clone())).collect();
        groups.sort();
        assert_eq!(groups, vec![vec![0, 1], vec![2, 3]]);
        assert!(plan.pickers.iter().all(|p| p.len() == 1), "one trip each: {plan:?}");
    }

    #[test]
    fn a_trip_takes_no_more_orders_than_it_may() {
        let cost = line(8);
        let picks: Vec<Pick> = (0..5).map(|o| pick(o, o + 1, o)).collect();
        let plan = plan(&cost, 5, &picks, &settings(1, Some(2), false));
        let trips = &plan.pickers[0];
        assert_eq!(trips.len(), 3, "five orders, two a trip: {plan:?}");
        assert!(trips.iter().all(|t| t.orders.len() <= 2));
        let mut all: Vec<usize> = trips.iter().flat_map(|t| t.orders.clone()).collect();
        all.sort();
        assert_eq!(all, vec![0, 1, 2, 3, 4], "every order once");
    }

    #[test]
    fn pickers_share_the_trips_evenly() {
        // Four trips of one order each, two pickers: two trips each, the
        // longest walks split between them.
        let cost = line(10);
        let picks = [pick(0, 9, 0), pick(1, 8, 1), pick(2, 2, 2), pick(3, 1, 3)];
        let plan = plan(&cost, 4, &picks, &settings(2, Some(1), false));
        let walked: Vec<f64> = plan.pickers.iter().map(|p| p.iter().map(|t| t.walked).sum()).collect();
        assert_eq!(plan.pickers.iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 2]);
        assert_eq!(walked, vec![20.0, 20.0], "9 and 1 to one, 8 and 2 to the other: {plan:?}");
    }

    #[test]
    fn a_shelf_two_trips_want_is_walked_to_once_when_asked() {
        // Orders 0 and 1 are near the bench and go together; order 2 is far
        // out and on its own trip. All three want the shelf at 3.
        let cost = line(10);
        let picks = [pick(0, 1, 0), pick(0, 3, 9), pick(1, 2, 1), pick(1, 3, 9), pick(2, 9, 2), pick(2, 3, 9)];
        let apart = plan(&cost, 3, &picks, &settings(2, Some(2), false));
        let together = plan(&cost, 3, &picks, &settings(2, Some(2), true));
        let takers = |plan: &Plan| -> Vec<Vec<usize>> {
            plan.pickers.iter().flatten().filter(|t| t.stops.iter().any(|s| s.0 == 3)).map(|t| t.orders.clone()).collect()
        };
        assert_eq!(takers(&apart).len(), 2, "each trip walks to the shelf: {apart:?}");
        assert_eq!(takers(&together).len(), 1, "one trip walks to it: {together:?}");
        // Order 2's trip passes the shelf on its way out to 9, so it keeps it
        // for nothing, and the near trip is spared the walk from 2 to 3. Each
        // pick is still taken once.
        let taken: usize = together.pickers.iter().flatten().map(|t| t.stops.iter().map(|s| s.1.len()).sum::<usize>()).sum();
        assert_eq!(taken, picks.len());
    }

    #[test]
    fn gathering_spares_the_trip_that_would_walk_furthest_for_it() {
        // Order 0 is at 1 and wants the shelf at 6; order 1 is at 7 and wants
        // it too. Kept by order 1's trip, it costs nothing extra; kept by order
        // 0's, it doubles its walk. One picker per trip.
        let cost = line(9);
        let picks = [pick(0, 1, 0), pick(0, 6, 5), pick(1, 7, 1), pick(1, 6, 5)];
        let plan = plan(&cost, 2, &picks, &settings(2, Some(1), true));
        let near = plan.pickers.iter().flatten().find(|t| t.orders == vec![0]).unwrap();
        let far = plan.pickers.iter().flatten().find(|t| t.orders == vec![1]).unwrap();
        assert_eq!(near.walked, 2.0, "order 0's trip goes to 1 and back: {plan:?}");
        assert_eq!(far.stops.iter().find(|s| s.0 == 6).map(|s| s.1.len()), Some(2), "and the far trip takes both");
    }

    #[test]
    fn a_shelf_off_the_floor_is_still_walked_to_once() {
        let cost = line(2);
        let picks = [Pick { order: 0, stop: None, what: 7 }, Pick { order: 1, stop: None, what: 7 }];
        let plan = plan(&cost, 2, &picks, &settings(2, Some(1), true));
        let walking: Vec<usize> = plan.pickers.iter().flatten().map(|t| t.off_route.len()).collect();
        assert!(walking.contains(&2) && walking.contains(&0), "one trip takes both: {plan:?}");
    }

    #[test]
    fn a_pick_off_the_floor_rides_with_its_order() {
        let cost = line(4);
        let picks = [pick(0, 2, 0), Pick { order: 0, stop: None, what: 1 }];
        let plan = plan(&cost, 1, &picks, &settings(1, None, true));
        let trip = &plan.pickers[0][0];
        assert_eq!(trip.off_route, vec![1]);
        assert_eq!(trip.stops, vec![(2, vec![0])]);
    }

    #[test]
    fn with_no_bench_a_trip_starts_where_it_likes() {
        // Node 0 is nothing from anywhere: a trip is the walk between its stops.
        let mut cost = line(8);
        for k in 0..8 {
            cost[0][k] = 0.0;
            cost[k][0] = 0.0;
        }
        let picks = [pick(0, 3, 0), pick(0, 7, 1)];
        let plan = plan(&cost, 1, &picks, &settings(1, None, false));
        assert_eq!(plan.pickers[0][0].walked, 4.0, "from 3 to 7");
    }

    #[test]
    fn more_pickers_than_orders_leaves_some_free() {
        let cost = line(4);
        let picks = [pick(0, 1, 0)];
        let plan = plan(&cost, 1, &picks, &settings(3, None, false));
        assert_eq!(plan.pickers.iter().filter(|p| !p.is_empty()).count(), 1);
        assert_eq!(plan.pickers.len(), 3);
    }
}

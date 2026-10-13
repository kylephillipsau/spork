//! The despatch bench, as data.
//!
//! Stages 6 to 9 of the recorded process, which are the ones that never
//! existed here in any form — the pack screen ends at a sealed carton and
//! everything after it still happens in MachShip by eye.
//!
//! What the walkthrough does at those stages is worth restating, because the
//! screen exists to delete most of it: NetSuite pushes a record across and
//! creates a *pending consignment*; the operator finds it again **by matching
//! the delivery address by eye**, since "there is no shared key surfaced in
//! this step"; picks a route from memory; re-enters the carton count, "which
//! is currently re-entered by hand in MachShip and exists in neither system
//! beforehand"; ticks a dangerous-goods declaration; and prints.
//!
//! Three of those five are gone by construction rather than by automation. A
//! consignment here is a row with an identifier, so nothing is matched by eye.
//! The carton count is [`CarrierLine`] — a fold over what is physically in the
//! boxes, which is the number the walkthrough says exists nowhere. And the
//! route is data rather than recall.
//!
//! Nothing in this module writes. The writes are `POST /consignments` and
//! `POST /packages/{id}/despatch`, both of which already existed and neither
//! of which had a screen.
//!
//! [`CarrierLine`]: crate::routes::CarrierLine

use actix_web::web;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::auth::Caller;
use crate::error::ApiError;
use crate::tenancy::TenantScope;
use crate::AppState;

/// Everything the despatch screen needs, in one read.
#[derive(Serialize)]
pub struct DespatchScreen {
    pub site: String,
    /// Sealed cartons that are on no consignment, grouped by the job they
    /// belong to. **Grouped rather than listed**: a consignment is booked
    /// against a customer's goods, and a flat list of cartons makes the
    /// operator reassemble that grouping in their head — which is the same
    /// eyeball-matching stage 6 exists to remove.
    pub waiting: Vec<WaitingJob>,
    /// Booked, and not yet all gone.
    pub booked: Vec<BookedConsignment>,
    /// What left today, at this site.
    pub gone_today: Vec<GoneConsignment>,
    pub carriers: Vec<Carrier>,
    pub providers: Vec<Provider>,
}

#[derive(Serialize)]
pub struct WaitingJob {
    pub fulfilment_id: Uuid,
    pub reference: String,
    pub order_reference: String,
    pub customer: String,
    pub cartons: Vec<WaitingCarton>,
    pub gross_weight_g: Option<i64>,
}

#[derive(Serialize)]
pub struct WaitingCarton {
    pub id: Uuid,
    pub sequence: String,
    pub package_type: Option<String>,
    pub gross_weight_g: Option<i64>,
    /// **A carton nobody weighed is a carton a carrier will weigh for you.**
    /// The reweigh comes back as their measurement and their invoice, which is
    /// the comparison `proposal.md` says the system exists to make possible —
    /// so it is worth saying before it leaves rather than after.
    pub weighed: bool,
}

#[derive(Serialize)]
pub struct BookedConsignment {
    pub consignment_id: Uuid,
    pub carrier_name: Option<String>,
    pub carrier_service_name: Option<String>,
    /// The carrier's word, NULL until one has been asked. Not ours to invent.
    pub status: Option<String>,
    pub despatch_at: Option<DateTime<Utc>>,
    pub package_count: i64,
    pub awaiting_despatch: i64,
    pub gross_weight_g: Option<i64>,
    pub packages: Vec<BookedCarton>,
}

#[derive(Serialize)]
pub struct BookedCarton {
    pub id: Uuid,
    pub sequence: String,
    pub reference: String,
    pub despatched: bool,
    /// **What despatching this carton actually is.**
    ///
    /// `POST /packages/{id}/despatch` takes a fulfilment line and a quantity,
    /// not a carton — because a despatch is a movement of stock out, and stock
    /// moves by line. A screen that shows cartons and cannot say what is in
    /// them would have to fetch each one before it could act, which is the
    /// round trip per box that stage 6 already costs today.
    pub lines: Vec<CartonLine>,
    /// What is in or on it, at any depth, each with its own lines: a pallet
    /// delivered as one leaves with every carton on it (D247).
    pub inside: Vec<BookedCarton>,
}

/// Netted through `stock_movement_effective`, like the packing list: a
/// reversed pick must stop reading as packed and therefore as despatchable.
#[derive(Serialize)]
pub struct CartonLine {
    pub fulfilment_line_id: Uuid,
    pub quantity: i64,
}

#[derive(Serialize)]
pub struct GoneConsignment {
    pub consignment_id: Uuid,
    pub carrier_name: Option<String>,
    pub package_count: i64,
    pub last_despatched_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct Carrier {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub services: Vec<CarrierService>,
}

#[derive(Serialize)]
pub struct CarrierService {
    pub id: Uuid,
    pub name: String,
    pub code: String,
}

/// How the carrier is reached, which D1 keeps separate from who they are: a
/// carrier booked through an intermediary today and directly tomorrow is the
/// same carrier, and the cost history survives the change.
#[derive(Serialize)]
pub struct Provider {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
}

/// The status its events give it now, which is what says where a carton is.
///
/// `package.status` is a fold and lags the act that just happened, so a carton
/// sealed or despatched a second ago still reads by its previous state. The
/// pack bench learned this the same way and for the same reason. Status events
/// only (`package_status_now`, migration 138): a sealed carton put on a pallet
/// is still sealed.
const WINNING: &str = "
    LEFT JOIN LATERAL (SELECT package_status_now(p.id) AS kind) w ON true";

/// The booked package, or the package on one, with this id.
fn holding(packages: &mut [BookedCarton], id: Uuid) -> Option<&mut BookedCarton> {
    packages.iter_mut().find_map(|p| {
        if p.id == id {
            Some(p)
        } else {
            p.inside.iter_mut().find(|i| i.id == id)
        }
    })
}

pub async fn screen(
    state: &web::Data<AppState>,
    who: &Caller,
    site_id: Option<Uuid>,
) -> Result<DespatchScreen, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let site = match site_id {
                    Some(id) => tx
                        .query_opt("SELECT code FROM site WHERE id = $1", &[&id])
                        .await?
                        .map(|r| r.get::<_, String>(0))
                        .unwrap_or_else(|| "no site".into()),
                    None => "every site".into(),
                };

                // ── waiting ──────────────────────────────────────────────
                let rows = tx
                    .query(
                        &format!(
                            "SELECT f.id,
                                    coalesce(f.reference, o.confirmation_number,
                                             o.external_ref, '—'),
                                    coalesce(o.confirmation_number, o.external_ref, '—'),
                                    coalesce(pa.name, 'no customer named'),
                                    p.id, coalesce(p.sequence::text, '—'),
                                    {what}, p.gross_weight_g
                               FROM package p
                               JOIN fulfilment f ON f.id = p.fulfilment_id
                               JOIN \"order\" o ON o.id = f.order_id
                               LEFT JOIN party pa ON pa.id = o.customer_party_id
                               LEFT JOIN package_type pt ON pt.id = p.package_type_id
                               {own_item}
                               LEFT JOIN consignment_package cp ON cp.package_id = p.id
                               {WINNING}
                              WHERE cp.consignment_id IS NULL
                                AND w.kind = 'sealed'
                                -- On a pallet delivered as one, it goes with the
                                -- pallet and is not booked on its own (D247).
                                AND NOT EXISTS (
                                    SELECT 1 FROM package op
                                     WHERE op.id = package_parent_now(p.id)
                                       AND NOT op.breaks_down)
                                AND ($1::uuid IS NULL OR f.site_id = $1)
                              ORDER BY f.reference NULLS LAST, f.id, p.sequence NULLS LAST",
                            what = crate::packages::WHAT,
                            own_item = crate::packages::OWN_ITEM,
                        ),
                        &[&site_id],
                    )
                    .await?;

                let mut waiting: Vec<WaitingJob> = vec![];
                for r in &rows {
                    let fulfilment_id: Uuid = r.get(0);
                    let gross: Option<i64> = r.get(7);
                    let carton = WaitingCarton {
                        id: r.get(4),
                        sequence: r.get(5),
                        package_type: r.get(6),
                        gross_weight_g: gross,
                        weighed: gross.is_some(),
                    };
                    match waiting.last_mut() {
                        Some(j) if j.fulfilment_id == fulfilment_id => {
                            j.gross_weight_g = sum(j.gross_weight_g, gross);
                            j.cartons.push(carton);
                        }
                        _ => waiting.push(WaitingJob {
                            fulfilment_id,
                            reference: r.get(1),
                            order_reference: r.get(2),
                            customer: r.get(3),
                            gross_weight_g: gross,
                            cartons: vec![carton],
                        }),
                    }
                }

                // ── booked ───────────────────────────────────────────────
                let rows = tx
                    .query(
                        &format!(
                            "SELECT c.id, ca.name, cs.name, c.status, c.despatch_at,
                                    p.id, coalesce(p.sequence::text, '—'),
                                    coalesce(f.reference, '—'),
                                    w.kind = 'despatched', p.gross_weight_g
                               FROM consignment c
                               JOIN consignment_package cp ON cp.consignment_id = c.id
                               JOIN package p ON p.id = cp.package_id
                               LEFT JOIN fulfilment f ON f.id = p.fulfilment_id
                               LEFT JOIN carrier ca ON ca.id = c.carrier_id
                               LEFT JOIN carrier_service cs ON cs.id = c.carrier_service_id
                               {WINNING}
                              WHERE ($1::uuid IS NULL OR f.site_id = $1)
                              ORDER BY c.despatch_at NULLS LAST, c.id, p.sequence NULLS LAST"
                        ),
                        &[&site_id],
                    )
                    .await?;

                let mut booked: Vec<BookedConsignment> = vec![];
                for r in &rows {
                    let consignment_id: Uuid = r.get(0);
                    let despatched: bool = r.get::<_, Option<bool>>(8).unwrap_or(false);
                    let gross: Option<i64> = r.get(9);
                    let carton = BookedCarton {
                        id: r.get(5),
                        sequence: r.get(6),
                        reference: r.get(7),
                        despatched,
                        lines: vec![],
                        inside: vec![],
                    };
                    match booked.last_mut() {
                        Some(c) if c.consignment_id == consignment_id => {
                            c.package_count += 1;
                            if !despatched {
                                c.awaiting_despatch += 1;
                            }
                            c.gross_weight_g = sum(c.gross_weight_g, gross);
                            c.packages.push(carton);
                        }
                        _ => booked.push(BookedConsignment {
                            consignment_id,
                            carrier_name: r.get(1),
                            carrier_service_name: r.get(2),
                            status: r.get(3),
                            despatch_at: r.get(4),
                            package_count: 1,
                            awaiting_despatch: i64::from(!despatched),
                            gross_weight_g: gross,
                            packages: vec![carton],
                        }),
                    }
                }

                // What is in or on each booked package now, at any depth: a
                // pallet that goes as one leaves with every carton on it (D247).
                let roots: Vec<Uuid> = booked
                    .iter()
                    .flat_map(|c| c.packages.iter().map(|p| p.id))
                    .collect();
                if !roots.is_empty() {
                    let rows = tx
                        .query(
                            "WITH RECURSIVE tree(root, id, n) AS (
                                 SELECT r, r, 0 FROM unnest($1::uuid[]) r
                                 UNION
                                 SELECT t.root, e.package_id, t.n + 1
                                   FROM tree t
                                   JOIN package_event e
                                     ON e.parent_package_id = t.id AND e.asserts_placement
                                  WHERE package_parent_now(e.package_id) = t.id AND t.n < 4)
                             SELECT DISTINCT t.root, p.id, coalesce(p.sequence::text, '—'),
                                    coalesce(f.reference, '—'),
                                    package_status_now(p.id) IS NOT DISTINCT FROM 'despatched'
                               FROM tree t
                               JOIN package p ON p.id = t.id
                               LEFT JOIN fulfilment f ON f.id = p.fulfilment_id
                              WHERE t.n > 0",
                            &[&roots],
                        )
                        .await?;
                    for r in &rows {
                        let root: Uuid = r.get(0);
                        let carton = BookedCarton {
                            id: r.get(1),
                            sequence: r.get(2),
                            reference: r.get(3),
                            despatched: r.get(4),
                            lines: vec![],
                            inside: vec![],
                        };
                        if let Some(p) = booked
                            .iter_mut()
                            .flat_map(|c| c.packages.iter_mut())
                            .find(|p| p.id == root)
                        {
                            p.inside.push(carton);
                        }
                    }
                }

                // One query for every booked carton's lines, and what is on
                // them, rather than one per carton: the screen is a page, and a
                // page that costs a round trip per box is the thing being
                // replaced.
                let ids: Vec<Uuid> = booked
                    .iter()
                    .flat_map(|c| c.packages.iter())
                    .flat_map(|p| std::iter::once(p.id).chain(p.inside.iter().map(|i| i.id)))
                    .collect();
                if !ids.is_empty() {
                    let lines = tx
                        .query(
                            // What each holds of each line (D246).
                            "SELECT c.package_id, c.fulfilment_line_id, sum(c.quantity)::bigint
                               FROM package_line_content c
                              WHERE c.package_id = ANY($1)
                              GROUP BY c.package_id, c.fulfilment_line_id
                             HAVING sum(c.quantity) <> 0",
                            &[&ids],
                        )
                        .await?;
                    for r in &lines {
                        let package_id: Uuid = r.get(0);
                        let line = CartonLine {
                            fulfilment_line_id: r.get(1),
                            quantity: r.get(2),
                        };
                        // Its own lines, or a carton's on a booked pallet.
                        if let Some(p) = booked
                            .iter_mut()
                            .find_map(|c| holding(&mut c.packages, package_id))
                        {
                            p.lines.push(line);
                        }
                    }
                }

                // **What left today, read from the event rather than a
                // column.** `despatched_at` on the fold would answer faster
                // and is a projection; the log is the thing that happened.
                let gone_today = tx
                    .query(
                        "SELECT c.id, ca.name, count(*)::bigint, max(e.occurred_at)
                           FROM consignment c
                           JOIN consignment_package cp ON cp.consignment_id = c.id
                           JOIN package p ON p.id = cp.package_id
                           JOIN package_event e ON e.package_id = p.id
                                               AND e.kind = 'despatched'
                           LEFT JOIN fulfilment f ON f.id = p.fulfilment_id
                           LEFT JOIN carrier ca ON ca.id = c.carrier_id
                          WHERE e.occurred_at >= date_trunc('day', now())
                            AND ($1::uuid IS NULL OR f.site_id = $1)
                          GROUP BY c.id, ca.name
                          ORDER BY max(e.occurred_at) DESC",
                        &[&site_id],
                    )
                    .await?
                    .iter()
                    .map(|r| GoneConsignment {
                        consignment_id: r.get(0),
                        carrier_name: r.get(1),
                        package_count: r.get(2),
                        last_despatched_at: r.get(3),
                    })
                    .collect();

                // ── the route, as data rather than as recall ─────────────
                let rows = tx
                    .query(
                        "SELECT ca.id, ca.name, ca.code, cs.id, cs.name, cs.code
                           FROM carrier ca
                           LEFT JOIN carrier_service cs ON cs.carrier_id = ca.id
                          ORDER BY ca.name, cs.name",
                        &[],
                    )
                    .await?;
                let mut carriers: Vec<Carrier> = vec![];
                for r in &rows {
                    let id: Uuid = r.get(0);
                    let service = r.get::<_, Option<Uuid>>(3).map(|sid| CarrierService {
                        id: sid,
                        name: r.get(4),
                        code: r.get(5),
                    });
                    match carriers.last_mut() {
                        Some(c) if c.id == id => c.services.extend(service),
                        _ => carriers.push(Carrier {
                            id,
                            name: r.get(1),
                            code: r.get(2),
                            services: service.into_iter().collect(),
                        }),
                    }
                }

                let providers = tx
                    .query("SELECT id, name, kind FROM freight_provider ORDER BY name", &[])
                    .await?
                    .iter()
                    .map(|r| Provider {
                        id: r.get(0),
                        name: r.get(1),
                        kind: r.get(2),
                    })
                    .collect();

                Ok(DespatchScreen {
                    site,
                    waiting,
                    booked,
                    gone_today,
                    carriers,
                    providers,
                })
            })
        })
        .await
}

/// Sum that keeps "nobody weighed this" distinct from zero.
///
/// A carton with no weight makes the job's total unknown rather than lighter,
/// and a total that quietly treats a missing figure as zero is the number a
/// carrier's invoice later disagrees with.
fn sum(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x + y),
        _ => None,
    }
}

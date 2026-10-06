//! Figures and photos recorded on one card, filed on another: moved when they
//! were recorded on the wrong card (D219, D222), copied when another item of
//! the family is the same box (D228).
//!
//! A box of ten respirators was weighed, measured and photographed on its
//! item's carton card, because "carton of 10" read like "10/box" before an
//! item said what it is sold as (D218). What was recorded is true of the box;
//! it is filed against the carton. A move takes it, the whole card's worth, to
//! another level of the same item.
//!
//! **Or to another item (D222).** A kit's part measured on the kit's card,
//! which is no physical thing, is true of the part, an item of its own. The
//! same move takes it there.
//!
//! **Or copied to a sibling (D228).** The colours of one brush come in the
//! same carton, the same size and weight, with the same sides but for a label.
//! A match copies what one of them has on a card to the same card of another
//! of its family, and leaves the first as it was; the side that differs is
//! then photographed again.
//!
//! **Nothing is rewritten.** Observations are facts somebody recorded, and the
//! history of where they were filed is part of them. Filing them on another
//! card:
//!
//! - each event on the card with anything live on it is mirrored on the other
//!   one: same moment, same method, same arrangement, and
//!   `derived_from_event_id` naming the event it came from;
//! - every live figure is copied into its mirrored event; moved, it is then
//!   retracted where it was, by a retraction naming it
//!   (`retracts_observation_id`), so no older figure surfaces on the wrong
//!   card in its place;
//! - every photo not already moved is filed again in its mirrored event, with
//!   its cut; moved, the original is marked moved to it
//!   (`observation_image_move`, D190), so photos to crop and the item's page
//!   both see the new one.
//!
//! The other card is the item's at that level under the case pack in force
//! now (D23): a box is only a definite thing to measure relative to one.

use std::collections::HashMap;

use actix_web::{post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::{caller, observable_for, SubjectRef};
use crate::tenancy::TenantScope;
use crate::AppState;

#[derive(Deserialize, Debug)]
pub struct RefileRequest {
    /// The card it was recorded on, and the one it belongs on: `each`,
    /// `inner` or `carton`.
    pub from: String,
    pub to: String,
    /// The item whose card it belongs on, when that is another item (D222).
    #[serde(default)]
    pub to_item: Option<Uuid>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct Refiled {
    /// Figures copied to the other card (and, moved, retracted where they were).
    pub figures: i64,
    /// Photos filed again on the other card.
    pub photos: i64,
    /// The act was recorded before: nothing was done again.
    pub replay: bool,
}

/// What is on one of an item's own cards (D228): enough to say what copying
/// it would bring.
#[derive(Serialize, Debug)]
pub struct RecordedCard {
    /// `each`, `inner` or `carton`.
    pub level: String,
    pub weighed: bool,
    pub measured: bool,
    /// Faces with a photo: its sides, top and bottom, its label.
    pub faces: i64,
}

/// A figure is live when nothing retracts or corrects it, and it is not
/// itself a retraction. `o` is the observation.
const LIVE: &str = "o.retracts_observation_id IS NULL
                AND NOT EXISTS (SELECT 1 FROM observation r WHERE r.retracts_observation_id = o.id)
                AND NOT EXISTS (SELECT 1 FROM observation c WHERE c.corrects_observation_id = o.id)";

/// A photo is still where it was taken when it has not been moved. `i` is the image.
const UNMOVED: &str =
    "NOT EXISTS (SELECT 1 FROM observation_image_move m WHERE m.observation_image_id = i.id)";

/// One of an item's own cards: `$1` the item, `$2` the level. `s` is the observable.
const CARD: &str = "s.item_id = $1 AND s.packaging_level = $2::text::packaging_level";

const LEVELS: [&str; 3] = ["each", "inner", "carton"];

/// What is on each of these items' own cards, those with anything on them,
/// the each first and the carton last.
pub async fn recorded(
    tx: &tokio_postgres::Transaction<'_>,
    items: &[Uuid],
) -> Result<HashMap<Uuid, Vec<RecordedCard>>, ApiError> {
    let rows = tx
        .query(
            &format!(
                "WITH f AS (
                     SELECT s.item_id, s.packaging_level,
                            bool_or(m.code = 'gross_weight') AS weighed,
                            bool_or(m.code IN ('length', 'diameter')) AS measured
                       FROM observable s
                       JOIN observation o ON o.observable_id = s.id
                       JOIN metric m ON m.id = o.metric_id
                      WHERE s.item_id = ANY($1) AND s.packaging_level::text = ANY($2) AND {LIVE}
                      GROUP BY 1, 2
                 ),
                 p AS (
                     SELECT s.item_id, s.packaging_level, count(DISTINCT i.face) AS faces
                       FROM observable s
                       JOIN observation_event e ON e.observable_id = s.id
                       JOIN observation_image i ON i.observation_event_id = e.id
                      WHERE s.item_id = ANY($1) AND s.packaging_level::text = ANY($2) AND {UNMOVED}
                      GROUP BY 1, 2
                 )
                 SELECT coalesce(f.item_id, p.item_id), coalesce(f.packaging_level, p.packaging_level)::text,
                        coalesce(f.weighed, false), coalesce(f.measured, false), coalesce(p.faces, 0)
                   FROM f FULL JOIN p ON p.item_id = f.item_id AND p.packaging_level = f.packaging_level
                  ORDER BY 1, array_position($2, coalesce(f.packaging_level, p.packaging_level)::text)"
            ),
            &[&items, &LEVELS.as_slice()],
        )
        .await?;
    let mut out: HashMap<Uuid, Vec<RecordedCard>> = HashMap::new();
    for r in rows {
        out.entry(r.get(0)).or_default().push(RecordedCard {
            level: r.get(1),
            weighed: r.get(2),
            measured: r.get(3),
            faces: r.get(4),
        });
    }
    Ok(out)
}

/// Whether a card has anything live on it: figures, or photos not moved away.
async fn holds(tx: &tokio_postgres::Transaction<'_>, item: Uuid, level: &str) -> Result<bool, ApiError> {
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM observable s JOIN observation o ON o.observable_id = s.id
                         WHERE {CARD} AND {LIVE})
             OR EXISTS (SELECT 1 FROM observable s
                          JOIN observation_event e ON e.observable_id = s.id
                          JOIN observation_image i ON i.observation_event_id = e.id
                         WHERE {CARD} AND {UNMOVED})"
    );
    Ok(tx.query_one(&sql, &[&item, &level]).await?.get(0))
}

/// The item's card at a level under the case pack in force at `at`.
async fn card_of(
    tx: &tokio_postgres::Transaction<'_>,
    tenant: Uuid,
    item: Uuid,
    level: &str,
    at: DateTime<Utc>,
) -> Result<Uuid, ApiError> {
    let subject = SubjectRef {
        package_id: None,
        location_id: None,
        lot_id: None,
        item_id: Some(item),
        item_style_id: None,
        item_part_id: None,
        packaging_level: Some(level.to_string()),
    };
    Ok(observable_for(tx, tenant, &subject, at).await?.0)
}

/// Moved, what was filed is taken from where it was; copied, it stays.
#[derive(Clone, Copy, PartialEq)]
enum Filing {
    Move,
    Copy,
}

/// File what is live on one card (`from_item` at `from`) again on another
/// observable, `target`, at level `to`, as the act `ev`. Copying, an event
/// already copied to `target` is not copied again: matching twice brings only
/// what is new.
///
/// A size on a single thing names the arrangement it was measured in (D138,
/// J72), and a pack's or a carton's never did: one filed on an each was the
/// thing in its packaging, so it is said to be as supplied.
async fn file_again(
    tx: &tokio_postgres::Transaction<'_>,
    ev: &NewClientEvent,
    from_item: Uuid,
    from: &str,
    target: Uuid,
    to: &str,
    filing: Filing,
) -> Result<(i64, i64), ApiError> {
    let act = ev.client_event_id;
    let person = ev.recorded_by_id;
    let tenant = ev.tenant_id;
    let at = ev.submitted_at;
    let copying = filing == Filing::Copy;

    // Each event with anything live on it, mirrored on the other card.
    let mirrored: HashMap<Uuid, Uuid> = tx
        .query(
            &format!(
                "WITH src AS (
                     SELECT e.id, e.observed_at, e.method, e.presentation_id
                       FROM observation_event e
                       JOIN observable s ON s.id = e.observable_id
                      WHERE {CARD}
                        AND (EXISTS (SELECT 1 FROM observation o
                                      WHERE o.observation_event_id = e.id AND {LIVE})
                             OR EXISTS (SELECT 1 FROM observation_image i
                                         WHERE i.observation_event_id = e.id AND {UNMOVED}))
                        AND NOT ($7 AND EXISTS (SELECT 1 FROM observation_event d
                                                 WHERE d.derived_from_event_id = e.id AND d.observable_id = $5))
                 )
                 INSERT INTO observation_event
                     (tenant_id, client_event_id, observable_id, observed_at, recorded_by_id,
                      method, ingestion_channel, derived_from_event_id, presentation_id)
                 SELECT $3, $4, $5, src.observed_at, $6, src.method, 'derived', src.id,
                        coalesce(src.presentation_id,
                                 (SELECT p.id FROM presentation p WHERE p.code = 'as_supplied' AND $8 = 'each'))
                   FROM src
                 RETURNING derived_from_event_id, id"
            ),
            &[&from_item, &from, &tenant, &act, &target, &person, &copying, &to],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();

    // Every live figure, copied into its mirror.
    let copied = tx
        .execute(
            &format!(
                "INSERT INTO observation
                     (tenant_id, observation_event_id, observable_id, observed_at, client_event_id,
                      metric_id, result_kind, dimension_id, value_numeric, value_instant,
                      value_code_id, value_boolean, value_text, uncertainty_dimension_id,
                      uncertainty_numeric, absent_reason, entered_value, entered_unit_id, confidence)
                 SELECT o.tenant_id, m.id, m.observable_id, m.observed_at, m.client_event_id,
                        o.metric_id, o.result_kind, o.dimension_id, o.value_numeric, o.value_instant,
                        o.value_code_id, o.value_boolean, o.value_text, o.uncertainty_dimension_id,
                        o.uncertainty_numeric, o.absent_reason, o.entered_value, o.entered_unit_id,
                        o.confidence
                   FROM observation o
                   JOIN observation_event m
                     ON m.derived_from_event_id = o.observation_event_id
                    AND m.client_event_id = $1 AND m.observable_id = $2
                  WHERE {LIVE}"
            ),
            &[&act, &target],
        )
        .await?;

    // Moved, retracted where they were, by an act of this person's now.
    if filing == Filing::Move {
        tx.execute(
            &format!(
                "WITH live AS (
                     SELECT o.id, o.observable_id, o.metric_id, o.result_kind, o.dimension_id
                       FROM observation o
                       JOIN observable s ON s.id = o.observable_id
                      WHERE {CARD} AND {LIVE}
                 ),
                 ev AS (
                     INSERT INTO observation_event
                         (tenant_id, client_event_id, observable_id, observed_at, recorded_by_id,
                          method, ingestion_channel)
                     SELECT DISTINCT $3::uuid, $4::uuid, live.observable_id, $5::timestamptz, $6::uuid,
                            'keyed'::observation_method, 'keyed'::ingestion_channel
                       FROM live
                     RETURNING id, observable_id
                 )
                 INSERT INTO observation
                     (tenant_id, observation_event_id, observable_id, observed_at, client_event_id,
                      metric_id, result_kind, dimension_id, absent_reason, retracts_observation_id)
                 SELECT $3, ev.id, live.observable_id, $5, $4, live.metric_id, live.result_kind,
                        live.dimension_id, 'retracted', live.id
                   FROM live JOIN ev ON ev.observable_id = live.observable_id"
            ),
            &[&from_item, &from, &tenant, &act, &at, &person],
        )
        .await?;
    }

    // Every photo not moved before, filed again in its mirror with its cut;
    // moved, the original marked moved to it. Each move and each cut is an
    // act of its own, named from this one.
    let images = tx
        .query(
            &format!(
                "SELECT i.id, i.observation_event_id, i.same_as_id
                   FROM observation_image i
                   JOIN observation_event e ON e.id = i.observation_event_id
                   JOIN observable s ON s.id = e.observable_id
                  WHERE {CARD} AND {UNMOVED}
                  ORDER BY i.captured_at, i.id"
            ),
            &[&from_item, &from],
        )
        .await?;
    let mut filed: HashMap<Uuid, Uuid> = HashMap::new();
    for image in &images {
        let (old, event, same_as): (Uuid, Uuid, Option<Uuid>) = (image.get(0), image.get(1), image.get(2));
        let Some(&into) = mirrored.get(&event) else { continue };
        let part = |what: &str| NewClientEvent {
            tenant_id: tenant,
            client_event_id: Uuid::new_v5(&act, format!("{what}:{old}").as_bytes()),
            site_id: ev.site_id,
            recorded_by_id: person,
            submitted_at: at,
        };
        let same_as = same_as.map(|s| filed.get(&s).copied().unwrap_or(s));
        let new: Uuid = tx
            .query_one(
                "INSERT INTO observation_image
                     (tenant_id, observation_event_id, face, digest, mime, byte_count,
                      width_px, height_px, captured_at, same_as_id)
                 SELECT tenant_id, $2, face, digest, mime, byte_count, width_px, height_px,
                        captured_at, $3
                   FROM observation_image WHERE id = $1
                 RETURNING id",
                &[&old, &into, &same_as],
            )
            .await?
            .get(0);
        let had_cut = tx
            .query_opt("SELECT 1 FROM observation_image_cut WHERE observation_image_id = $1", &[&old])
            .await?
            .is_some();
        if had_cut {
            let cut = part("cut");
            client_events::claim_act(tx, &cut).await?;
            tx.execute(
                "INSERT INTO observation_image_cut
                     (tenant_id, observation_image_id, client_event_id, recorded_by_id, corners,
                      digest, mime, byte_count, width_px, height_px)
                 SELECT tenant_id, $2, $3, $4, corners, digest, mime, byte_count, width_px, height_px
                   FROM observation_image_cut WHERE observation_image_id = $1
                  ORDER BY recorded_at DESC, id DESC LIMIT 1",
                &[&old, &new, &cut.client_event_id, &person],
            )
            .await?;
        }
        if filing == Filing::Move {
            let shift = part("move");
            client_events::claim_act(tx, &shift).await?;
            tx.execute(
                "INSERT INTO observation_image_move
                     (tenant_id, observation_image_id, moved_to_image_id, client_event_id, recorded_by_id)
                 VALUES ($1, $2, $3, $4, $5)",
                &[&tenant, &old, &new, &shift.client_event_id, &person],
            )
            .await?;
        }
        filed.insert(old, new);
    }
    Ok((copied as i64, filed.len() as i64))
}

fn a_level(l: &str) -> bool {
    LEVELS.contains(&l)
}

/// Move what one card of an item holds to another card of it, or of another item.
///
/// **A card that already has its own figures or photos is refused**, rather
/// than two sets mixed on it; move those away first.
#[post("/items/{id}/refile")]
pub async fn refile(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<RefileRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item = path.into_inner();
    let body = body.into_inner();
    if !a_level(&body.from) || !a_level(&body.to) {
        return Err(ApiError::Rejected("a card is the item's each, its pack or its carton".into()));
    }
    let to_item = body.to_item.unwrap_or(item);
    if body.from == body.to && to_item == item {
        return Err(ApiError::Rejected("that is the card it is on".into()));
    }
    let ev = NewClientEvent {
        tenant_id: who.tenant_id,
        client_event_id: body.client_event_id,
        site_id: who.site_id,
        recorded_by_id: who.person_id,
        submitted_at: body.occurred_at,
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                tx.query_opt("SELECT 1 FROM item WHERE id = $1", &[&item]).await?.ok_or(ApiError::NotFound)?;
                if tx.query_opt("SELECT 1 FROM item WHERE id = $1", &[&to_item]).await?.is_none() {
                    return Err(ApiError::Rejected("the item it would move to isn't in the catalogue".into()));
                }
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(Refiled { figures: 0, photos: 0, replay: true });
                }
                if !holds(tx, item, &body.from).await? {
                    return Err(ApiError::Rejected("there is nothing recorded on that card to move".into()));
                }
                if holds(tx, to_item, &body.to).await? {
                    return Err(ApiError::Rejected(
                        "the card it would move to has figures or photos of its own: move those away first".into(),
                    ));
                }
                let target = card_of(tx, ev.tenant_id, to_item, &body.to, body.occurred_at).await?;
                let (figures, photos) = file_again(tx, &ev, item, &body.from, target, &body.to, Filing::Move).await?;
                Ok(Refiled { figures, photos, replay: false })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Deserialize, Debug)]
pub struct MatchRequest {
    /// The item of its family whose cards are copied.
    pub from_item: Uuid,
    /// Which of its cards, each copied to the same card of this item:
    /// `each`, `inner` or `carton`.
    pub levels: Vec<String>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Copy what another item of the family has on some of its cards to the same
/// cards of this one (D228).
///
/// Copies keep when they were taken, so what this item has of its own and
/// newer stays in front of them, as the newest always does. A pack or a
/// carton is only the same thing when it holds the same (D23): with no case
/// pack of its own, this item is given the other's; with one that says a
/// different count, the match is refused.
#[post("/items/{id}/match")]
pub async fn match_family(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<MatchRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item = path.into_inner();
    let body = body.into_inner();
    if body.levels.is_empty() || !body.levels.iter().all(|l| a_level(l)) {
        return Err(ApiError::Rejected("say which cards to match: its each, its pack or its carton".into()));
    }
    if body.from_item == item {
        return Err(ApiError::Rejected("that is this item".into()));
    }
    let ev = NewClientEvent {
        tenant_id: who.tenant_id,
        client_event_id: body.client_event_id,
        site_id: who.site_id,
        recorded_by_id: who.person_id,
        submitted_at: body.occurred_at,
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let row = tx
                    .query_opt(
                        "SELECT a.code, b.code, a.style_id IS NOT NULL AND a.style_id = b.style_id
                           FROM item a LEFT JOIN item b ON b.id = $2
                          WHERE a.id = $1",
                        &[&item, &body.from_item],
                    )
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let (code, from_code, family): (String, Option<String>, Option<bool>) =
                    (row.get(0), row.get(1), row.get(2));
                let Some(from_code) = from_code else {
                    return Err(ApiError::Rejected("the item to match isn't in the catalogue".into()));
                };
                if family != Some(true) {
                    return Err(ApiError::Rejected(format!("{from_code} isn't of {code}'s family")));
                }
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(Refiled { figures: 0, photos: 0, replay: true });
                }

                // A pack or carton is the same thing only holding the same.
                if body.levels.iter().any(|l| l != "each") {
                    let theirs = crate::cartons::in_force(tx, body.from_item, body.occurred_at).await?;
                    let ours = crate::cartons::in_force(tx, item, body.occurred_at).await?;
                    let counts = |r: &tokio_postgres::Row| -> (Option<i32>, Option<i32>) { (r.get(1), r.get(2)) };
                    match (theirs.as_ref().map(counts), ours.as_ref().map(counts)) {
                        (Some(t), Some(o)) => {
                            let differ = |a: Option<i32>, b: Option<i32>| matches!((a, b), (Some(a), Some(b)) if a != b);
                            if differ(t.0, o.0) || differ(t.1, o.1) {
                                return Err(ApiError::Rejected(format!(
                                    "a carton of {from_code} doesn't hold what a carton of {code} does, \
                                     so its pack and carton aren't the same to match"
                                )));
                            }
                        }
                        (Some(t), None) => {
                            let said = NewClientEvent {
                                client_event_id: Uuid::new_v5(&ev.client_event_id, b"carton"),
                                ..ev.clone()
                            };
                            client_events::claim_act(tx, &said).await?;
                            tx.execute(
                                "INSERT INTO item_packing_config
                                     (tenant_id, item_id, units_per_inner, inners_per_carton,
                                      effective_from, client_event_id, recorded_by_id)
                                 VALUES ($1, $2, $3, $4, $5::timestamptz::date, $6, $7)",
                                &[
                                    &ev.tenant_id,
                                    &item,
                                    &t.0,
                                    &t.1,
                                    &body.occurred_at,
                                    &said.client_event_id,
                                    &ev.recorded_by_id,
                                ],
                            )
                            .await?;
                        }
                        // Nothing in force for theirs: its cards hold nothing under one.
                        (None, _) => {}
                    }
                }

                let (mut figures, mut photos) = (0, 0);
                let mut any = false;
                for level in &body.levels {
                    if !holds(tx, body.from_item, level).await? {
                        continue;
                    }
                    any = true;
                    let target = card_of(tx, ev.tenant_id, item, level, body.occurred_at).await?;
                    let (f, p) = file_again(tx, &ev, body.from_item, level, target, level, Filing::Copy).await?;
                    figures += f;
                    photos += p;
                }
                if !any {
                    return Err(ApiError::Rejected(format!("{from_code} has nothing recorded on those cards to match")));
                }
                Ok(Refiled { figures, photos, replay: false })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

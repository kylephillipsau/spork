//! The board's plan, kept as a draft. D245, docs/packing-plan.md (settled 4).
//!
//! What the packer planned for a fulfilment, and what Suggest planned round
//! it: parcels of a box type, or things as they are (migration 144), which
//! lines' units go in each, and the pallet each goes on (migration 142). Not a fact: the board sends the whole plan
//! each time it changes and it replaces the last. A planned parcel holds the
//! id its carton will be made with, and `POST /packages` with that id uses the
//! row up, so the carton made from it is the same parcel.

use std::collections::{HashMap, HashSet};

use actix_web::{put, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::routes::{caller, Spot};
use crate::tenancy::TenantScope;
use crate::AppState;

/// A parcel planned and not yet made.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlannedParcel {
    /// The id its carton will be made with.
    pub id: Uuid,
    /// The box type it is a box of; none for a thing as it is.
    pub package_type_id: Option<Uuid>,
    /// One of an item as it is (D196): what Ship makes, planned.
    pub as_is: Option<PlannedAsIs>,
    /// The planned pallet it goes on, or one already made; neither for none.
    pub on_parcel_id: Option<Uuid>,
    pub on_package_id: Option<Uuid>,
    /// Where on that pallet, when said.
    pub at: Option<Spot>,
    /// The packer placed it, rather than Suggest: Suggest plans round it.
    pub placed_by_hand: bool,
    pub lines: Vec<PlannedLine>,
}

/// One of an item at a level, as it is, as `package` says one (migration 113).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlannedAsIs {
    pub item_id: Uuid,
    /// `each`, `inner` or `carton`.
    pub level: String,
    /// Which of its cases, for an inner or a carton.
    pub config_id: Option<Uuid>,
}

/// So many of a line's units planned into a parcel, in the units it counts.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlannedLine {
    pub line_id: Uuid,
    pub quantity: i64,
}

#[derive(Deserialize, Debug)]
pub struct SavePlanRequest {
    pub parcels: Vec<PlannedParcel>,
}

/// Whether a plan holds together on its own, or why not: each parcel once,
/// each pallet it names one in the plan and never itself or a loop, a spot
/// only on a pallet, and counts above nought.
pub fn check_plan(parcels: &[PlannedParcel]) -> Result<(), String> {
    let mut ids = HashSet::new();
    for p in parcels {
        if !ids.insert(p.id) {
            return Err("a parcel is planned twice".into());
        }
    }
    let on: HashMap<Uuid, Uuid> = parcels.iter().filter_map(|p| Some((p.id, p.on_parcel_id?))).collect();
    for p in parcels {
        match (&p.package_type_id, &p.as_is) {
            (Some(_), None) => {}
            (None, Some(a)) => {
                if !matches!(a.level.as_str(), "each" | "inner" | "carton") {
                    return Err(format!("a thing as it is is an each, an inner or a carton, not {}", a.level));
                }
                if a.config_id.is_some() && a.level == "each" {
                    return Err("an each has no case to name".into());
                }
            }
            _ => return Err("a planned parcel is a box or a thing as it is".into()),
        }
        if p.on_parcel_id.is_some() && p.on_package_id.is_some() {
            return Err("a parcel goes on one pallet".into());
        }
        if p.on_parcel_id.is_some_and(|id| !ids.contains(&id)) {
            return Err("a parcel goes on a pallet the plan does not have".into());
        }
        if p.at.is_some() && p.on_parcel_id.is_none() && p.on_package_id.is_none() {
            return Err("a spot is on a pallet; this parcel is on none".into());
        }
        // Up the planned pallets, never back to where it started.
        let (mut at, mut steps) = (p.on_parcel_id, 0);
        while let Some(next) = at {
            steps += 1;
            if next == p.id || steps > parcels.len() {
                return Err("a parcel cannot go on itself".into());
            }
            at = on.get(&next).copied();
        }
        let mut lines = HashSet::new();
        for l in &p.lines {
            if l.quantity <= 0 {
                return Err("a planned count is more than nought".into());
            }
            if !lines.insert(l.line_id) {
                return Err("a line is planned into a parcel once; add the counts".into());
            }
        }
    }
    Ok(())
}

/// Replace a fulfilment's plan with this one.
#[put("/fulfilments/{id}/pack-plan")]
pub async fn save_plan(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SavePlanRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let fulfilment = path.into_inner();
    let parcels = body.into_inner().parcels;
    check_plan(&parcels).map_err(ApiError::Rejected)?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                tx.query_opt("SELECT 1 FROM fulfilment WHERE id = $1 FOR UPDATE", &[&fulfilment])
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let ids: Vec<Uuid> = parcels.iter().map(|p| p.id).collect();
                let made: i64 = tx
                    .query_one("SELECT count(*) FROM package WHERE id = ANY($1)", &[&ids])
                    .await?
                    .get(0);
                if made > 0 {
                    return Err(ApiError::Rejected("a planned parcel has been made already; read the bench again".into()));
                }
                let lines: Vec<Uuid> = parcels.iter().flat_map(|p| p.lines.iter().map(|l| l.line_id)).collect();
                let theirs: i64 = tx
                    .query_one(
                        "SELECT count(*) FROM fulfilment_line WHERE id = ANY($1) AND fulfilment_id <> $2",
                        &[&lines, &fulfilment],
                    )
                    .await?
                    .get(0);
                if theirs > 0 {
                    return Err(ApiError::Rejected("a planned line is another order's".into()));
                }
                // A thing as it is holds its own item and nothing else.
                for p in &parcels {
                    let Some(a) = &p.as_is else { continue };
                    let ids: Vec<Uuid> = p.lines.iter().map(|l| l.line_id).collect();
                    let other: i64 = tx
                        .query_one(
                            "SELECT count(*) FROM fulfilment_line fl JOIN order_line ol ON ol.id = fl.order_line_id
                              WHERE fl.id = ANY($1) AND ol.item_id <> $2",
                            &[&ids, &a.item_id],
                        )
                        .await?
                        .get(0);
                    if other > 0 {
                        return Err(ApiError::Rejected("a thing as it is holds only its own item".into()));
                    }
                }
                tx.execute("DELETE FROM pack_plan_parcel WHERE fulfilment_id = $1", &[&fulfilment])
                    .await?;
                for (position, p) in parcels.iter().enumerate() {
                    let at = p.at;
                    tx.execute(
                        "INSERT INTO pack_plan_parcel
                             (id, tenant_id, fulfilment_id, package_type_id, on_parcel_id, on_package_id,
                              at_x_mm, at_y_mm, at_z_mm, turned, placed_by_hand, position,
                              own_item_id, own_level, item_packing_config_id)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                                 $13, $14::text::packaging_level, $15)",
                        &[
                            &p.id,
                            &who.tenant_id,
                            &fulfilment,
                            &p.package_type_id,
                            &p.on_parcel_id,
                            &p.on_package_id,
                            &at.map(|s| s.x_mm),
                            &at.map(|s| s.y_mm),
                            &at.map(|s| s.z_mm),
                            &at.is_some_and(|s| s.turned),
                            &p.placed_by_hand,
                            &(position as i32),
                            &p.as_is.as_ref().map(|a| a.item_id),
                            &p.as_is.as_ref().map(|a| a.level.clone()),
                            &p.as_is.as_ref().and_then(|a| a.config_id),
                        ],
                    )
                    .await?;
                    for l in &p.lines {
                        tx.execute(
                            "INSERT INTO pack_plan_line (tenant_id, parcel_id, fulfilment_line_id, quantity)
                             VALUES ($1, $2, $3, $4)",
                            &[&who.tenant_id, &p.id, &l.line_id, &l.quantity],
                        )
                        .await?;
                    }
                }
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// A fulfilment's plan, in the order the board shows it.
pub async fn plan_of(tx: &tokio_postgres::Transaction<'_>, fulfilment: Uuid) -> Result<Vec<PlannedParcel>, ApiError> {
    let mut lines: HashMap<Uuid, Vec<PlannedLine>> = HashMap::new();
    for r in tx
        .query(
            "SELECT l.parcel_id, l.fulfilment_line_id, l.quantity
               FROM pack_plan_line l JOIN pack_plan_parcel p ON p.id = l.parcel_id
              WHERE p.fulfilment_id = $1
              ORDER BY l.fulfilment_line_id",
            &[&fulfilment],
        )
        .await?
    {
        lines.entry(r.get(0)).or_default().push(PlannedLine { line_id: r.get(1), quantity: r.get(2) });
    }
    Ok(tx
        .query(
            "SELECT id, package_type_id, on_parcel_id, on_package_id,
                    at_x_mm, at_y_mm, at_z_mm, turned, placed_by_hand,
                    own_item_id, own_level::text, item_packing_config_id
               FROM pack_plan_parcel WHERE fulfilment_id = $1
              ORDER BY position, id",
            &[&fulfilment],
        )
        .await?
        .iter()
        .map(|r| {
            let id: Uuid = r.get(0);
            PlannedParcel {
                id,
                package_type_id: r.get(1),
                as_is: r
                    .get::<_, Option<Uuid>>(9)
                    .map(|item_id| PlannedAsIs { item_id, level: r.get(10), config_id: r.get(11) }),
                on_parcel_id: r.get(2),
                on_package_id: r.get(3),
                at: match (r.get::<_, Option<i32>>(4), r.get::<_, Option<i32>>(5), r.get::<_, Option<i32>>(6)) {
                    (Some(x_mm), Some(y_mm), Some(z_mm)) => Some(Spot { x_mm, y_mm, z_mm, turned: r.get(7) }),
                    _ => None,
                },
                placed_by_hand: r.get(8),
                lines: lines.remove(&id).unwrap_or_default(),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parcel(id: u128, on: Option<u128>) -> PlannedParcel {
        PlannedParcel {
            id: Uuid::from_u128(id),
            package_type_id: Some(Uuid::from_u128(99)),
            as_is: None,
            on_parcel_id: on.map(Uuid::from_u128),
            on_package_id: None,
            at: None,
            placed_by_hand: true,
            lines: vec![PlannedLine { line_id: Uuid::from_u128(500), quantity: 4 }],
        }
    }

    #[test]
    fn a_plan_of_cartons_on_a_pallet_holds_together() {
        assert!(check_plan(&[parcel(1, None), parcel(2, Some(1)), parcel(3, Some(1))]).is_ok());
    }

    #[test]
    fn a_loop_a_missing_pallet_a_twice_planned_parcel_and_a_count_of_nought_are_refused() {
        assert!(check_plan(&[parcel(1, Some(2)), parcel(2, Some(1))]).unwrap_err().contains("itself"));
        assert!(check_plan(&[parcel(1, Some(7))]).unwrap_err().contains("does not have"));
        assert!(check_plan(&[parcel(1, None), parcel(1, None)]).unwrap_err().contains("twice"));
        let mut none = parcel(1, None);
        none.lines[0].quantity = 0;
        assert!(check_plan(&[none]).unwrap_err().contains("nought"));
        let mut spot = parcel(1, None);
        spot.at = Some(Spot { x_mm: 0, y_mm: 0, z_mm: 0, turned: false });
        assert!(check_plan(&[spot]).unwrap_err().contains("on none"));
    }

    /// **A thing as it is is planned too** (migration 144): a box or a thing
    /// as it is, never both or neither, and a level that is one.
    #[test]
    fn a_planned_parcel_is_a_box_or_a_thing_as_it_is() {
        let as_is = |level: &str| PlannedAsIs { item_id: Uuid::from_u128(7), level: level.into(), config_id: None };
        let mut own = parcel(2, Some(1));
        own.package_type_id = None;
        own.as_is = Some(as_is("carton"));
        assert!(check_plan(&[parcel(1, None), own.clone()]).is_ok());
        let mut both = own.clone();
        both.package_type_id = Some(Uuid::from_u128(99));
        assert!(check_plan(&[parcel(1, None), both]).unwrap_err().contains("a box or"));
        own.as_is = Some(as_is("pallet"));
        assert!(check_plan(&[parcel(1, None), own]).unwrap_err().contains("not pallet"));
    }
}

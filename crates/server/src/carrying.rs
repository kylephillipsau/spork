//! What a thing takes on top, and what a box carries. D249.
//!
//! A thing takes weight unless it says otherwise (`subject_load`, said and
//! inherited as "this way up" is, migration 145): `shipping::say_load`.
//!
//! A box type has two limits. **On its walls**, when what is above sits across
//! its edges: set from a test on the floor, or estimated. The estimate is the
//! method the standards give rather than a figure, since none gives one: what
//! the box crushes at empty by McKee's formula from its size and board, cut by
//! the Fibre Box Association's multipliers for the warehouse's conditions, and
//! divided by a factor for the road (ASTM D4169). **On its lid**, when
//! something sits within its top and the carton is hollow: 10 kg to start. A
//! cut takes its full box's board and limits unless it says its own
//! (`package_type_effective`).
//!
//! The conditions are the warehouse's, one setting for the workspace
//! (`workspace_setting`), not a carrier's.

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// What a hollow carton's lid carries until its box says (D249): the user's
/// own figure, which the research found no standard to replace.
pub const LID_TO_START_G: i64 = 10_000;

/// How the cartons are stacked, most of the time.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stacking {
    /// Corners over corners, each column straight.
    Columns,
    /// In columns, not quite lined up.
    Misaligned,
    /// Each layer turned across the one below.
    Interlocked,
}

/// How the cartons are handled on their way.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Handling {
    Careful,
    SomeRough,
    Rough,
}

/// The warehouse's conditions, which the estimate of a box's walls allows for.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Conditions {
    /// The relative humidity most days, in per cent: 50, 60, 70, 80 or 90.
    pub humidity_pct: i16,
    /// How long a load stands, in days: 1, 10, 30, 90 or 180.
    pub days_under_load: i16,
    pub stacking: Stacking,
    /// A pallet's deck has gaps between its boards.
    pub deck_gaps: bool,
    pub handling: Handling,
    /// What the walls must carry over what is on them, for the road.
    pub road_factor: i16,
}

impl Default for Conditions {
    /// D249's: 70% most days, interlocked on decks with gaps, some rough
    /// handling, and three times over for the road. Under ten days the
    /// table has no figure, and the road factor stands for it.
    fn default() -> Self {
        Conditions {
            humidity_pct: 70,
            days_under_load: 1,
            stacking: Stacking::Interlocked,
            deck_gaps: true,
            handling: Handling::SomeRough,
            road_factor: 3,
        }
    }
}

impl Conditions {
    /// What is left of a box's strength empty: the Fibre Box Handbook's
    /// multipliers for each condition, and the road factor. Medians where
    /// the handbook gives a range.
    pub fn share(&self) -> f64 {
        let humidity = match self.humidity_pct {
            50 => 1.0,
            60 => 0.9,
            70 => 0.8,
            80 => 0.68,
            _ => 0.48,
        };
        let time = match self.days_under_load {
            1 => 1.0,
            10 => 0.63,
            30 => 0.6,
            90 => 0.55,
            _ => 0.5,
        };
        let stacking = match self.stacking {
            Stacking::Columns => 1.0,
            Stacking::Misaligned => 0.875,
            Stacking::Interlocked => 0.6,
        };
        let deck = if self.deck_gaps { 0.85 } else { 1.0 };
        let handling = match self.handling {
            Handling::Careful => 1.0,
            Handling::SomeRough => 0.9,
            Handling::Rough => 0.7,
        };
        humidity * time * stacking * deck * handling / f64::from(self.road_factor)
    }

    /// Whether each is one the multipliers have a figure for.
    pub fn check(&self) -> Result<(), String> {
        if ![50, 60, 70, 80, 90].contains(&self.humidity_pct) {
            return Err("humidity is 50, 60, 70, 80 or 90%".into());
        }
        if ![1, 10, 30, 90, 180].contains(&self.days_under_load) {
            return Err("a load stands 1, 10, 30, 90 or 180 days".into());
        }
        if !(1..=10).contains(&self.road_factor) {
            return Err("the road factor is from 1 to 10".into());
        }
        Ok(())
    }
}

impl Stacking {
    pub fn as_str(self) -> &'static str {
        match self {
            Stacking::Columns => "columns",
            Stacking::Misaligned => "misaligned",
            Stacking::Interlocked => "interlocked",
        }
    }
    fn parse(s: &str) -> Stacking {
        match s {
            "columns" => Stacking::Columns,
            "misaligned" => Stacking::Misaligned,
            _ => Stacking::Interlocked,
        }
    }
}

impl Handling {
    pub fn as_str(self) -> &'static str {
        match self {
            Handling::Careful => "careful",
            Handling::SomeRough => "some_rough",
            Handling::Rough => "rough",
        }
    }
    fn parse(s: &str) -> Handling {
        match s {
            "careful" => Handling::Careful,
            "rough" => Handling::Rough,
            _ => Handling::SomeRough,
        }
    }
}

/// The workspace's conditions as said, or the defaults until something is.
pub async fn conditions(tx: &tokio_postgres::Transaction<'_>) -> Result<Conditions, ApiError> {
    Ok(tx
        .query_opt(
            "SELECT humidity_pct, days_under_load, stacking, deck_gaps, handling, road_factor
               FROM workspace_setting",
            &[],
        )
        .await?
        .map(|r| Conditions {
            humidity_pct: r.get(0),
            days_under_load: r.get(1),
            stacking: Stacking::parse(r.get(2)),
            deck_gaps: r.get(3),
            handling: Handling::parse(r.get(4)),
            road_factor: r.get(5),
        })
        .unwrap_or_default())
}

/// Say the warehouse's conditions (D249). Every box's estimate follows.
#[post("/workspace/conditions")]
pub async fn set_conditions(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<Conditions>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let c = body.into_inner();
    c.check().map_err(ApiError::Rejected)?;
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                tx.execute(
                    "INSERT INTO workspace_setting
                         (tenant_id, humidity_pct, days_under_load, stacking, deck_gaps, handling, road_factor)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)
                     ON CONFLICT (tenant_id) DO UPDATE
                        SET humidity_pct = EXCLUDED.humidity_pct,
                            days_under_load = EXCLUDED.days_under_load,
                            stacking = EXCLUDED.stacking,
                            deck_gaps = EXCLUDED.deck_gaps,
                            handling = EXCLUDED.handling,
                            road_factor = EXCLUDED.road_factor,
                            updated_at = now()",
                    &[
                        &tenant,
                        &c.humidity_pct,
                        &c.days_under_load,
                        &c.stacking.as_str(),
                        &c.deck_gaps,
                        &c.handling.as_str(),
                        &c.road_factor,
                    ],
                )
                .await?;
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// A box's board: single or double wall, and its edge crush in pounds an inch.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxBoard {
    pub walls: i16,
    pub ect: i16,
}

impl BoxBoard {
    /// What a box is assumed to be until its stamp is read: single-wall 32 ECT.
    pub const ASSUMED: BoxBoard = BoxBoard { walls: 1, ect: 32 };

    /// The board's thickness, in millimetres: C flute, and BC for double wall.
    fn caliper_mm(self) -> f64 {
        if self.walls >= 2 { 6.5 } else { 4.0 }
    }
}

/// What a box crushes at empty, in grams, by McKee's formula from its board
/// and its footprint: 5.87 × edge crush × √(caliper × perimeter).
pub fn crush_g(length_mm: i32, width_mm: i32, board: BoxBoard) -> i64 {
    // Pounds an inch to newtons a millimetre.
    let ect_n_per_mm = f64::from(board.ect) * 4.448_222 / 25.4;
    let perimeter_mm = 2.0 * f64::from(length_mm + width_mm);
    let newtons = 5.87 * ect_n_per_mm * (board.caliper_mm() * perimeter_mm).sqrt();
    (newtons / 9.806_65 * 1000.0).round() as i64
}

/// What a box carries (D249), on its walls and its lid, and where each figure
/// comes from.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Carries {
    /// What its walls carry, in grams.
    pub walls_g: i64,
    /// Estimated from its size and board under the warehouse's conditions,
    /// rather than set from a test.
    pub walls_estimated: bool,
    /// What it crushes at empty by the estimate, which the walls figure is
    /// worked out from; absent when a test set it.
    pub crush_g: Option<i64>,
    /// What its lid carries when it is hollow, in grams.
    pub lid_g: i64,
    /// Said of it or its full box, rather than the 10 kg to start.
    pub lid_said: bool,
    /// Its board, as stamped or assumed.
    pub board: BoxBoard,
    pub board_said: bool,
}

/// What a box of this footprint, board and limits carries under these
/// conditions. Its limits as `package_type_effective` reads them, a cut's
/// filled from its full box.
pub fn carries(
    length_mm: i32,
    width_mm: i32,
    board: Option<BoxBoard>,
    wall_load_g: Option<i64>,
    lid_load_g: Option<i64>,
    conditions: &Conditions,
) -> Carries {
    let said = board;
    let board = said.unwrap_or(BoxBoard::ASSUMED);
    let crush = crush_g(length_mm, width_mm, board);
    let estimate = (crush as f64 * conditions.share() / 100.0).round() as i64 * 100;
    Carries {
        walls_g: wall_load_g.unwrap_or(estimate),
        walls_estimated: wall_load_g.is_none(),
        crush_g: wall_load_g.is_none().then_some(crush),
        lid_g: lid_load_g.unwrap_or(LID_TO_START_G),
        lid_said: lid_load_g.is_some(),
        board,
        board_said: said.is_some(),
    }
}

#[derive(Deserialize, Debug)]
pub struct SetCarriesRequest {
    /// Its board as stamped, or none to assume single-wall 32 ECT.
    pub board: Option<BoxBoard>,
    /// What its walls carry from a test, in grams, or none to estimate it.
    pub wall_load_g: Option<i64>,
    /// What its lid carries when hollow, in grams, or none for 10 kg.
    pub lid_load_g: Option<i64>,
}

/// Say a box's board and what it carries (D249), all of it at once: a cut
/// left unsaid takes its full box's. The workspace's own boxes only, as with
/// its other settings; a pallet carries by the carrier's rules, not its walls.
#[post("/package-types/{id}/carries")]
pub async fn set_carries(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SetCarriesRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let b = body.into_inner();
    if let Some(board) = b.board {
        if !(1..=2).contains(&board.walls) || !(10..=200).contains(&board.ect) {
            return Err(ApiError::Rejected("a board is single or double wall, with an edge crush from 10 to 200".into()));
        }
    }
    if b.wall_load_g.is_some_and(|g| g <= 0) {
        return Err(ApiError::Rejected("its walls carry more than nothing; leave it empty to estimate".into()));
    }
    if b.lid_load_g.is_some_and(|g| g < 0) {
        return Err(ApiError::Rejected("a lid carries nothing or more; leave it empty for 10 kg".into()));
    }
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let changed = tx
                    .execute(
                        "UPDATE package_type
                            SET board_walls = $2, board_ect = $3, wall_load_g = $4, lid_load_g = $5
                          WHERE id = $1 AND tenant_id IS NOT NULL
                            AND coalesce(carrier_package_code, '') NOT IN ('PAL', 'SKI', 'SKD')",
                        &[
                            &id,
                            &b.board.map(|x| x.walls),
                            &b.board.map(|x| x.ect),
                            &b.wall_load_g,
                            &b.lid_load_g,
                        ],
                    )
                    .await?;
                if changed == 0 {
                    return Err(ApiError::NotFound);
                }
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mckee_gives_the_figure_the_study_compared_with() {
        // 495 × 330 single-wall 32 ECT: the 2021 study's box, which McKee put
        // at about 270 kg and the test crushed at about 290.
        let kg = crush_g(495, 330, BoxBoard::ASSUMED) as f64 / 1000.0;
        assert!((265.0..280.0).contains(&kg), "{kg}");
        // Double wall is stronger, and so is a stiffer board.
        assert!(crush_g(495, 330, BoxBoard { walls: 2, ect: 32 }) > crush_g(495, 330, BoxBoard::ASSUMED));
        assert!(crush_g(495, 330, BoxBoard { walls: 1, ect: 44 }) > crush_g(495, 330, BoxBoard::ASSUMED));
    }

    #[test]
    fn the_conditions_to_start_leave_about_an_eighth() {
        let share = Conditions::default().share();
        assert!((0.115..0.13).contains(&share), "{share}");
        let dry = Conditions { humidity_pct: 50, stacking: Stacking::Columns, deck_gaps: false, handling: Handling::Careful, ..Default::default() };
        assert!((dry.share() - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_box_carries_its_estimate_until_a_test_says() {
        let c = Conditions::default();
        let est = carries(450, 340, None, None, None, &c);
        assert!(est.walls_estimated && !est.board_said && !est.lid_said);
        assert_eq!(est.lid_g, LID_TO_START_G);
        // About 30 kg on a Medium Box's walls, as D249 worked it out.
        assert!((28_000..36_000).contains(&est.walls_g), "{}", est.walls_g);
        assert_eq!(est.walls_g % 100, 0);
        let tested = carries(450, 340, Some(BoxBoard { walls: 2, ect: 48 }), Some(45_000), Some(0), &c);
        assert_eq!((tested.walls_g, tested.walls_estimated, tested.crush_g), (45_000, false, None));
        assert_eq!((tested.lid_g, tested.lid_said, tested.board_said), (0, true, true));
    }

    #[test]
    fn only_conditions_with_a_figure_are_taken() {
        assert!(Conditions::default().check().is_ok());
        assert!(Conditions { humidity_pct: 65, ..Default::default() }.check().is_err());
        assert!(Conditions { days_under_load: 2, ..Default::default() }.check().is_err());
        assert!(Conditions { road_factor: 0, ..Default::default() }.check().is_err());
    }
}

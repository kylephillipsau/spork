//! Carriers, and the rules their pallets are built to. D250, settled 9.
//!
//! How a pallet must be built is a named set of **build rules**: each of a
//! fixed list ([`RuleKind`]), a must or a preference with a weight, or off,
//! with its figure. A carrier names the set its breakdown pallets are built to
//! and the set its pallets that go as one are built to, and one carrier is the
//! workspace's usual one. Nothing here knows any carrier: a carrier that
//! changes what it allows is a change of settings, and a new kind of rule is
//! code, because the planner has to understand it.
//!
//! A set is saved as a new version each time (`build_rules_version`), and a
//! pallet names the version it was started under (`package.build_rules_version_id`),
//! so changing a carrier's rules does not judge old pallets again.

use std::collections::HashMap;

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// The fixed list (D250), as `build_rule_kind` has it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// The most the load stands, in mm, with its base or from the deck.
    Height,
    /// The most the pallet and its load weigh, in grams.
    Weight,
    /// The most a carton stands out left or right, either side of the forks.
    OverhangSides,
    /// The most it stands out at the front, where the forks go in, or the back.
    OverhangEnds,
    /// Each delivered parcel's label on a side facing out, hidden behind nothing.
    Labels,
    /// The least share of each box's base resting on something, per thousand.
    Support,
    /// A parcel over so many grams is marked heavy.
    Heavy,
    /// A parcel over so many grams goes on a pallet.
    Loose,
    /// Each order's parcels next to one another.
    Together,
    /// Corners over corners where the boxes allow.
    Columns,
    /// The heavy and weight-taking low, the weight over the middle.
    LowAndCentred,
    /// As few heights on top as it can.
    FlatTop,
}

impl RuleKind {
    /// In the order a set shows them.
    pub const ALL: [RuleKind; 12] = [
        RuleKind::Height,
        RuleKind::Weight,
        RuleKind::OverhangSides,
        RuleKind::OverhangEnds,
        RuleKind::Labels,
        RuleKind::Support,
        RuleKind::Heavy,
        RuleKind::Loose,
        RuleKind::Together,
        RuleKind::Columns,
        RuleKind::LowAndCentred,
        RuleKind::FlatTop,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            RuleKind::Height => "height",
            RuleKind::Weight => "weight",
            RuleKind::OverhangSides => "overhang_sides",
            RuleKind::OverhangEnds => "overhang_ends",
            RuleKind::Labels => "labels",
            RuleKind::Support => "support",
            RuleKind::Heavy => "heavy",
            RuleKind::Loose => "loose",
            RuleKind::Together => "together",
            RuleKind::Columns => "columns",
            RuleKind::LowAndCentred => "low_and_centred",
            RuleKind::FlatTop => "flat_top",
        }
    }

    fn parse(s: &str) -> Option<RuleKind> {
        RuleKind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// It has a figure: a length, a weight or a share.
    fn has_amount(self) -> bool {
        matches!(
            self,
            RuleKind::Height
                | RuleKind::Weight
                | RuleKind::OverhangSides
                | RuleKind::OverhangEnds
                | RuleKind::Support
                | RuleKind::Heavy
                | RuleKind::Loose
        )
    }

    /// Nothing is lowest or flattest absolutely, so only preferred.
    fn prefer_only(self) -> bool {
        matches!(self, RuleKind::LowAndCentred | RuleKind::FlatTop)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Strength {
    Must,
    Prefer,
    Off,
}

impl Strength {
    fn as_str(self) -> &'static str {
        match self {
            Strength::Must => "must",
            Strength::Prefer => "prefer",
            Strength::Off => "off",
        }
    }
    fn parse(s: &str) -> Strength {
        match s {
            "must" => Strength::Must,
            "prefer" => Strength::Prefer,
            _ => Strength::Off,
        }
    }
}

/// One rule of a set.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BuildRule {
    pub kind: RuleKind,
    pub strength: Strength,
    /// How much a preference counts against the others, 1 to 3.
    pub weight: Option<i16>,
    /// Its figure, in its kind's unit: mm, grams, or per thousand for support.
    /// Kept while it is off, so turning it on again keeps it.
    pub amount: Option<i64>,
    /// Height only: counted with the pallet's base, or from its deck.
    pub with_base: Option<bool>,
    /// Labels only: a label on top of the top layer counts as facing out.
    pub top_counts: Option<bool>,
}

impl BuildRule {
    fn new(kind: RuleKind, strength: Strength, weight: Option<i16>, amount: Option<i64>) -> BuildRule {
        BuildRule {
            kind,
            strength,
            weight,
            amount,
            with_base: (kind == RuleKind::Height).then_some(true),
            top_counts: (kind == RuleKind::Labels).then_some(false),
        }
    }
}

/// What a new set starts as: the canvas's, and a carrier's published figures
/// for heavy and loose. Every one is a setting.
pub fn starting_rules() -> Vec<BuildRule> {
    use RuleKind::*;
    use Strength::*;
    vec![
        BuildRule::new(Height, Must, None, Some(1_800)),
        BuildRule::new(Weight, Must, None, Some(1_000_000)),
        BuildRule::new(OverhangSides, Prefer, Some(2), Some(0)),
        BuildRule::new(OverhangEnds, Prefer, Some(2), Some(0)),
        BuildRule::new(Labels, Prefer, Some(3), None),
        BuildRule::new(Support, Must, None, Some(750)),
        BuildRule::new(Heavy, Must, None, Some(20_000)),
        BuildRule::new(Loose, Must, None, Some(30_000)),
        BuildRule::new(Together, Prefer, Some(3), None),
        BuildRule::new(Columns, Prefer, Some(2), None),
        BuildRule::new(LowAndCentred, Prefer, Some(2), None),
        BuildRule::new(FlatTop, Prefer, Some(1), None),
    ]
}

/// Whether a set's rules can be kept: each of the list once, a weight on a
/// preference and none on anything else, and each kind's figure.
pub fn check_rules(rules: &[BuildRule]) -> Result<(), String> {
    for kind in RuleKind::ALL {
        let n = rules.iter().filter(|r| r.kind == kind).count();
        if n != 1 {
            return Err(format!("a set has each rule once, and {} {}", kind.as_str(), if n == 0 { "is missing" } else { "is there twice" }));
        }
    }
    for r in rules {
        let k = r.kind.as_str();
        match (r.strength, r.weight) {
            (Strength::Prefer, Some(1..=3)) | (Strength::Must | Strength::Off, None) => {}
            (Strength::Prefer, _) => return Err(format!("{k}: a preference weighs 1, 2 or 3")),
            _ => return Err(format!("{k}: only a preference has a weight")),
        }
        if r.kind.prefer_only() && r.strength == Strength::Must {
            return Err(format!("{k} is a preference, never a must"));
        }
        match (r.kind.has_amount(), r.amount) {
            (true, Some(a)) if a < 0 => return Err(format!("{k}: nothing less than nought")),
            (true, Some(a)) if r.kind == RuleKind::Support && a > 1000 => {
                return Err("support: no more than all of a box's base".into())
            }
            (true, Some(0)) if matches!(r.kind, RuleKind::Height | RuleKind::Weight | RuleKind::Heavy | RuleKind::Loose) => {
                return Err(format!("{k}: more than nothing"))
            }
            (true, Some(_)) | (false, None) => {}
            (true, None) => return Err(format!("{k} needs its figure")),
            (false, Some(_)) => return Err(format!("{k} has no figure")),
        }
        if (r.kind == RuleKind::Height) != r.with_base.is_some() {
            return Err("only height says whether the base counts, and it must".into());
        }
        if (r.kind == RuleKind::Labels) != r.top_counts.is_some() {
            return Err("only labels say whether the top counts, and they must".into());
        }
    }
    Ok(())
}

/// A set of rules, as one version of it has them.
#[derive(Serialize, Debug, Clone)]
pub struct RuleSet {
    /// The set.
    pub id: Uuid,
    /// The version: what a pallet keeps.
    pub version_id: Uuid,
    pub name: String,
    pub rules: Vec<BuildRule>,
}

/// These versions' rules, by version, each in the list's order.
pub async fn rule_sets(tx: &tokio_postgres::Transaction<'_>, versions: &[Uuid]) -> Result<HashMap<Uuid, RuleSet>, ApiError> {
    let mut out: HashMap<Uuid, RuleSet> = HashMap::new();
    if versions.is_empty() {
        return Ok(out);
    }
    for r in tx
        .query(
            "SELECT v.id, s.id, s.name, r.kind::text, r.strength::text, r.weight, r.amount, r.with_base, r.top_counts
               FROM build_rules_version v
               JOIN build_rules s ON s.id = v.build_rules_id
               JOIN build_rule r ON r.version_id = v.id
              WHERE v.id = ANY($1)
              ORDER BY v.id, r.kind",
            &[&versions],
        )
        .await?
    {
        let Some(kind) = RuleKind::parse(r.get(3)) else { continue };
        let set = out.entry(r.get(0)).or_insert_with(|| RuleSet { id: r.get(1), version_id: r.get(0), name: r.get(2), rules: vec![] });
        set.rules.push(BuildRule {
            kind,
            strength: Strength::parse(r.get(4)),
            weight: r.get(5),
            amount: r.get(6),
            with_base: r.get(7),
            top_counts: r.get(8),
        });
    }
    Ok(out)
}

/// Keep a set's rules as a new version, saved by this person.
async fn save_version(tx: &tokio_postgres::Transaction<'_>, tenant: Uuid, set: Uuid, by: Uuid, rules: &[BuildRule]) -> Result<Uuid, ApiError> {
    let version: Uuid = tx
        .query_one(
            "INSERT INTO build_rules_version (tenant_id, build_rules_id, saved_by_id) VALUES ($1, $2, $3) RETURNING id",
            &[&tenant, &set, &by],
        )
        .await?
        .get(0);
    for r in rules {
        tx.execute(
            "INSERT INTO build_rule (tenant_id, version_id, kind, strength, weight, amount, with_base, top_counts)
             VALUES ($1, $2, $3::text::build_rule_kind, $4::text::rule_strength, $5, $6, $7, $8)",
            &[&tenant, &version, &r.kind.as_str(), &r.strength.as_str(), &r.weight, &r.amount, &r.with_base, &r.top_counts],
        )
        .await?;
    }
    Ok(version)
}

/// The newest version of a set, or none when there is no such set.
async fn current_version(tx: &tokio_postgres::Transaction<'_>, set: Uuid) -> Result<Option<Uuid>, ApiError> {
    Ok(tx
        .query_opt("SELECT version_id FROM build_rules_now WHERE build_rules_id = $1", &[&set])
        .await?
        .map(|r| r.get(0)))
}

/// A set of rules as Workspace › Build rules shows it.
#[derive(Serialize, Debug)]
pub struct RuleSetRow {
    #[serde(flatten)]
    pub set: RuleSet,
    pub saved_at: DateTime<Utc>,
    pub saved_by: String,
    /// What points at it: "the usual carrier's breakdown pallets".
    pub used_by: Vec<String>,
}

/// The workspace's sets of build rules, each as it is now (D250).
#[get("/build-rules")]
pub async fn build_rules(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(|tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "SELECT n.version_id, n.saved_at, p.display_name,
                                coalesce(array_agg(u.used ORDER BY u.used) FILTER (WHERE u.used IS NOT NULL), '{}')
                           FROM build_rules s
                           JOIN build_rules_now n ON n.build_rules_id = s.id
                           JOIN person p ON p.id = n.saved_by_id
                           LEFT JOIN LATERAL (
                               SELECT c.name || '''s breakdown pallets' AS used
                                 FROM carrier c WHERE c.breakdown_rules_id = s.id
                               UNION ALL
                               SELECT c.name || '''s pallets that go as one'
                                 FROM carrier c WHERE c.as_one_rules_id = s.id) u ON true
                          GROUP BY s.id, s.name, n.version_id, n.saved_at, p.display_name
                          ORDER BY s.name",
                        &[],
                    )
                    .await?;
                let versions: Vec<Uuid> = rows.iter().map(|r| r.get(0)).collect();
                let mut sets = rule_sets(tx, &versions).await?;
                Ok(rows
                    .iter()
                    .filter_map(|r| {
                        Some(RuleSetRow {
                            set: sets.remove(&r.get::<_, Uuid>(0))?,
                            saved_at: r.get(1),
                            saved_by: r.get(2),
                            used_by: r.get(3),
                        })
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Deserialize, Debug)]
pub struct NewRuleSetRequest {
    pub name: String,
    /// A set to start from, as it is now; else the rules to start.
    pub from: Option<Uuid>,
}

#[derive(Serialize, Debug)]
pub struct Made {
    pub id: Uuid,
}

/// Start a set of build rules (D250), from another set or from the rules to
/// start.
#[post("/build-rules")]
pub async fn new_rule_set(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<NewRuleSetRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let b = body.into_inner();
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::Rejected("a set of rules has a name".into()));
    }
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let id = scope
        .run(move |tx| {
            Box::pin(async move {
                if tx.query_opt("SELECT 1 FROM build_rules WHERE name = $1", &[&name]).await?.is_some() {
                    return Err(ApiError::Rejected(format!("there is a set called {name} already")));
                }
                let rules = match b.from {
                    Some(from) => {
                        let version = current_version(tx, from).await?.ok_or(ApiError::NotFound)?;
                        rule_sets(tx, &[version]).await?.remove(&version).map(|s| s.rules).unwrap_or_default()
                    }
                    None => starting_rules(),
                };
                let id: Uuid = tx
                    .query_one("INSERT INTO build_rules (tenant_id, name) VALUES ($1, $2) RETURNING id", &[&who.tenant_id, &name])
                    .await?
                    .get(0);
                save_version(tx, who.tenant_id, id, who.person_id, &rules).await?;
                Ok(id)
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(Made { id }))
}

#[derive(Deserialize, Debug)]
pub struct SaveRuleSetRequest {
    pub name: String,
    pub rules: Vec<BuildRule>,
}

/// Save a set of build rules (D250): its name, and its rules as a new version
/// when they changed. Pallets started before keep the rules they were started
/// under.
#[put("/build-rules/{id}")]
pub async fn save_rule_set(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SaveRuleSetRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let b = body.into_inner();
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::Rejected("a set of rules has a name".into()));
    }
    check_rules(&b.rules).map_err(ApiError::Rejected)?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let version = current_version(tx, id).await?.ok_or(ApiError::NotFound)?;
                if tx.query_opt("SELECT 1 FROM build_rules WHERE name = $1 AND id <> $2", &[&name, &id]).await?.is_some() {
                    return Err(ApiError::Rejected(format!("there is a set called {name} already")));
                }
                tx.execute("UPDATE build_rules SET name = $2 WHERE id = $1", &[&id, &name]).await?;
                let now = rule_sets(tx, &[version]).await?.remove(&version).map(|s| s.rules).unwrap_or_default();
                let same = now.len() == b.rules.len() && b.rules.iter().all(|r| now.contains(r));
                if !same {
                    save_version(tx, who.tenant_id, id, who.person_id, &b.rules).await?;
                }
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// A carrier's service, as consignments name one.
#[derive(Serialize, Debug)]
pub struct ServiceRow {
    pub id: Uuid,
    pub name: String,
    pub code: String,
}

/// A carrier as Workspace › Carriers shows it.
#[derive(Serialize, Debug)]
pub struct CarrierRow {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    /// The workspace's own, rather than one the platform ships.
    pub own: bool,
    /// The one a pallet is started under unless the board is told another.
    pub usual: bool,
    /// The set its breakdown pallets are built to.
    pub breakdown_rules_id: Option<Uuid>,
    /// The set its pallets that go as one are built to.
    pub as_one_rules_id: Option<Uuid>,
    pub services: Vec<ServiceRow>,
}

/// The carriers, their services and the rules each points at (D250).
#[get("/carriers")]
pub async fn carriers(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(|tx| {
            Box::pin(async move {
                let usual = usual_carrier(tx).await?;
                let services = tx
                    .query("SELECT carrier_id, id, name, code FROM carrier_service ORDER BY name", &[])
                    .await?;
                let rows = tx
                    .query(
                        "SELECT id, name, code, tenant_id IS NOT NULL, breakdown_rules_id, as_one_rules_id
                           FROM carrier ORDER BY name",
                        &[],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| {
                        let id: Uuid = r.get(0);
                        CarrierRow {
                            id,
                            name: r.get(1),
                            code: r.get(2),
                            own: r.get(3),
                            usual: usual == Some(id),
                            breakdown_rules_id: r.get(4),
                            as_one_rules_id: r.get(5),
                            services: services
                                .iter()
                                .filter(|s| s.get::<_, Uuid>(0) == id)
                                .map(|s| ServiceRow { id: s.get(1), name: s.get(2), code: s.get(3) })
                                .collect(),
                        }
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// The workspace's usual carrier, when it has said one.
pub async fn usual_carrier(tx: &tokio_postgres::Transaction<'_>) -> Result<Option<Uuid>, ApiError> {
    Ok(tx
        .query_opt("SELECT usual_carrier_id FROM workspace_setting", &[])
        .await?
        .and_then(|r| r.get(0)))
}

/// The rules a pallet of this carrier's is built to now: its set for
/// breakdown pallets or for pallets that go as one, as that set is now.
pub async fn rules_for(tx: &tokio_postgres::Transaction<'_>, carrier: Uuid, breaks_down: bool) -> Result<Option<Uuid>, ApiError> {
    Ok(tx
        .query_opt(
            "SELECT n.version_id
               FROM carrier c
               JOIN build_rules_now n
                 ON n.build_rules_id = CASE WHEN $2 THEN c.breakdown_rules_id ELSE c.as_one_rules_id END
              WHERE c.id = $1",
            &[&carrier, &breaks_down],
        )
        .await?
        .map(|r| r.get(0)))
}

/// A carrier as the pack bench needs it: the rules its pallets are built to
/// now, for the board's checks and for a pallet not yet made.
#[derive(Serialize, Debug)]
pub struct BenchCarrier {
    pub id: Uuid,
    pub name: String,
    pub usual: bool,
    pub breakdown: Option<RuleSet>,
    pub as_one: Option<RuleSet>,
}

/// Every carrier with the rules its pallets are built to now.
pub async fn bench_carriers(tx: &tokio_postgres::Transaction<'_>) -> Result<Vec<BenchCarrier>, ApiError> {
    let usual = usual_carrier(tx).await?;
    let rows = tx
        .query(
            "SELECT c.id, c.name, b.version_id, a.version_id
               FROM carrier c
               LEFT JOIN build_rules_now b ON b.build_rules_id = c.breakdown_rules_id
               LEFT JOIN build_rules_now a ON a.build_rules_id = c.as_one_rules_id
              ORDER BY c.name",
            &[],
        )
        .await?;
    let versions: Vec<Uuid> = rows.iter().flat_map(|r| [r.get::<_, Option<Uuid>>(2), r.get(3)]).flatten().collect();
    let sets = rule_sets(tx, &versions).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let id: Uuid = r.get(0);
            BenchCarrier {
                id,
                name: r.get(1),
                usual: usual == Some(id),
                breakdown: r.get::<_, Option<Uuid>>(2).and_then(|v| sets.get(&v).cloned()),
                as_one: r.get::<_, Option<Uuid>>(3).and_then(|v| sets.get(&v).cloned()),
            }
        })
        .collect())
}

/// A code from a name, as a carrier is known on a consignment: "Fast Freight" is FASTFREIGHT.
fn code_of(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase()
}

#[derive(Deserialize, Debug)]
pub struct NewCarrierRequest {
    pub name: String,
    /// What a consignment calls it; from its name when not given.
    pub code: Option<String>,
}

/// Add a carrier the workspace sends with (settled 9).
#[post("/carriers")]
pub async fn add_carrier(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<NewCarrierRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let (name, code) = named(&body.name, body.code.as_deref())?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let id = scope
        .run(move |tx| {
            Box::pin(async move {
                if tx.query_opt("SELECT 1 FROM carrier WHERE code = $1 AND tenant_id IS NOT NULL", &[&code]).await?.is_some() {
                    return Err(ApiError::Rejected(format!("a carrier is known as {code} already")));
                }
                Ok(tx
                    .query_one(
                        "INSERT INTO carrier (tenant_id, name, code) VALUES ($1, $2, $3) RETURNING id",
                        &[&who.tenant_id, &name, &code],
                    )
                    .await?
                    .get::<_, Uuid>(0))
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(Made { id }))
}

/// A name trimmed and present, and a code from it unless one is given.
fn named(name: &str, code: Option<&str>) -> Result<(String, String), ApiError> {
    let name = name.trim().to_string();
    let code = code.map(str::trim).filter(|c| !c.is_empty()).map(str::to_uppercase).unwrap_or_else(|| code_of(&name));
    if name.is_empty() || code.is_empty() {
        return Err(ApiError::Rejected("a carrier or service has a name".into()));
    }
    Ok((name, code))
}

#[derive(Deserialize, Debug)]
pub struct SaveCarrierRequest {
    pub name: String,
    pub code: Option<String>,
    pub breakdown_rules_id: Option<Uuid>,
    pub as_one_rules_id: Option<Uuid>,
}

/// Change a carrier's name and the rules its pallets are built to (D250).
/// The workspace's own carriers only. Pallets made before keep their rules.
#[post("/carriers/{id}")]
pub async fn save_carrier(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SaveCarrierRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let b = body.into_inner();
    let (name, code) = named(&b.name, b.code.as_deref())?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                if tx
                    .query_opt("SELECT 1 FROM carrier WHERE code = $1 AND id <> $2 AND tenant_id IS NOT NULL", &[&code, &id])
                    .await?
                    .is_some()
                {
                    return Err(ApiError::Rejected(format!("a carrier is known as {code} already")));
                }
                for set in [b.breakdown_rules_id, b.as_one_rules_id].into_iter().flatten() {
                    if current_version(tx, set).await?.is_none() {
                        return Err(ApiError::Rejected("no such set of rules".into()));
                    }
                }
                let changed = tx
                    .execute(
                        "UPDATE carrier SET name = $2, code = $3, breakdown_rules_id = $4, as_one_rules_id = $5
                          WHERE id = $1 AND tenant_id IS NOT NULL",
                        &[&id, &name, &code, &b.breakdown_rules_id, &b.as_one_rules_id],
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

/// Add a service to one of the workspace's carriers: what a consignment books.
#[post("/carriers/{id}/services")]
pub async fn add_service(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<NewCarrierRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let carrier = path.into_inner();
    let (name, code) = named(&body.name, body.code.as_deref())?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let id = scope
        .run(move |tx| {
            Box::pin(async move {
                if tx.query_opt("SELECT 1 FROM carrier WHERE id = $1 AND tenant_id IS NOT NULL", &[&carrier]).await?.is_none() {
                    return Err(ApiError::NotFound);
                }
                if tx
                    .query_opt("SELECT 1 FROM carrier_service WHERE carrier_id = $1 AND code = $2", &[&carrier, &code])
                    .await?
                    .is_some()
                {
                    return Err(ApiError::Rejected(format!("that carrier has a service known as {code} already")));
                }
                Ok(tx
                    .query_one(
                        "INSERT INTO carrier_service (tenant_id, carrier_id, name, code) VALUES ($1, $2, $3, $4) RETURNING id",
                        &[&who.tenant_id, &carrier, &name, &code],
                    )
                    .await?
                    .get::<_, Uuid>(0))
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(Made { id }))
}

#[derive(Deserialize, Debug)]
pub struct UsualCarrierRequest {
    pub carrier_id: Option<Uuid>,
}

/// Say which carrier is the usual one (settled 9), or that none is.
#[post("/workspace/usual-carrier")]
pub async fn set_usual_carrier(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<UsualCarrierRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let carrier = body.carrier_id;
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                if let Some(c) = carrier {
                    if tx.query_opt("SELECT 1 FROM carrier WHERE id = $1", &[&c]).await?.is_none() {
                        return Err(ApiError::NotFound);
                    }
                }
                // The conditions' defaults come with the row the first time
                // anything is said of the workspace.
                let d = crate::carrying::Conditions::default();
                tx.execute(
                    "INSERT INTO workspace_setting
                         (tenant_id, humidity_pct, days_under_load, stacking, deck_gaps, handling, road_factor, usual_carrier_id)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                     ON CONFLICT (tenant_id) DO UPDATE
                        SET usual_carrier_id = EXCLUDED.usual_carrier_id, updated_at = now()",
                    &[
                        &tenant,
                        &d.humidity_pct,
                        &d.days_under_load,
                        &d.stacking.as_str(),
                        &d.deck_gaps,
                        &d.handling.as_str(),
                        &d.road_factor,
                        &carrier,
                    ],
                )
                .await?;
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
    fn the_rules_to_start_can_be_kept() {
        assert_eq!(check_rules(&starting_rules()), Ok(()));
    }

    #[test]
    fn a_set_has_every_rule_once() {
        let mut rules = starting_rules();
        rules.pop();
        assert!(check_rules(&rules).unwrap_err().contains("flat_top is missing"));
        let mut rules = starting_rules();
        rules.push(rules[0].clone());
        assert!(check_rules(&rules).unwrap_err().contains("height is there twice"));
    }

    #[test]
    fn each_rule_is_kept_as_its_kind_allows() {
        let with = |i: usize, f: &dyn Fn(&mut BuildRule)| {
            let mut rules = starting_rules();
            f(&mut rules[i]);
            check_rules(&rules)
        };
        // A must with a weight, a preference without one.
        assert!(with(0, &|r| r.weight = Some(2)).is_err());
        assert!(with(2, &|r| r.weight = None).is_err());
        assert!(with(2, &|r| r.weight = Some(4)).is_err());
        // Flat on top is never a must; off is fine.
        assert!(with(11, &|r| { r.strength = Strength::Must; r.weight = None }).is_err());
        assert!(with(11, &|r| { r.strength = Strength::Off; r.weight = None }).is_ok());
        // Figures: support is at most all of a base, a weight is more than nothing,
        // and together has none.
        assert!(with(5, &|r| r.amount = Some(1001)).is_err());
        assert!(with(1, &|r| r.amount = Some(0)).is_err());
        assert!(with(2, &|r| r.amount = Some(0)).is_ok());
        assert!(with(8, &|r| r.amount = Some(1)).is_err());
        assert!(with(0, &|r| r.amount = None).is_err());
        // An off rule keeps its figure.
        assert!(with(6, &|r| r.strength = Strength::Off).is_ok());
        // Only height says its base, only labels their top.
        assert!(with(0, &|r| r.with_base = None).is_err());
        assert!(with(1, &|r| r.with_base = Some(true)).is_err());
        assert!(with(4, &|r| r.top_counts = None).is_err());
    }

    #[test]
    fn a_code_is_made_from_a_name() {
        assert_eq!(code_of("Fast Freight 2"), "FASTFREIGHT2");
        assert_eq!(named("  Road Co ", None).unwrap(), ("Road Co".into(), "ROADCO".into()));
        assert_eq!(named("Road Co", Some(" rc ")).unwrap().1, "RC");
        assert!(named("  ", None).is_err());
    }
}

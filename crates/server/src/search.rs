//! The global search: bins, items and orders, found by any part of a code or
//! any word, as a person types (D189).
//!
//! # One index per tenant, in memory, rebuilt from the database
//!
//! Nothing here is a record. The index is a reading of the tables, built the
//! first time a tenant searches and rebuilt in the background once it is a
//! minute old, so what was added a minute ago is found and nothing has to tell
//! the index about each write. A site's worth of rows (thousands of items, a
//! few thousand bins, the open orders) builds in well under a second. Held in
//! memory, so there is nothing on disk to migrate, lose or keep in step.
//!
//! # How a query matches
//!
//! - **A code or number exactly**, a barcode or an order number scanned or
//!   typed whole, first of all;
//! - **any run of two or more characters inside a code or number**, so `3010`
//!   finds every code with 3010 in it;
//! - **every word, by its start**, across descriptions, customers and the
//!   places bins are in, a typo forgiven in a longer word.
//!
//! The exact scan of the header (D111) still decides where Enter goes when
//! what was typed is one thing; this is what is shown while typing.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, Query, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING};
use tantivy::tokenizer::{AsciiFoldingFilter, LowerCaser, NgramTokenizer, RawTokenizer, SimpleTokenizer, TextAnalyzer};
use tantivy::{doc, Index, IndexReader, TantivyDocument, Term};
use uuid::Uuid;

use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// How old an index may be before a search rebuilds it behind itself.
const FRESH: Duration = Duration::from_secs(60);
/// The longest piece of a code indexed: longer pieces are found by the exact
/// field or by their own shorter pieces.
const GRAM: usize = 20;

struct Fields {
    kind: Field,
    id: Field,
    title: Field,
    detail: Field,
    site: Field,
    exact: Field,
    gram: Field,
    words: Field,
}

struct Built {
    reader: IndexReader,
    fields: Fields,
    at: Instant,
}

struct Slot {
    built: Option<Arc<Built>>,
    rebuilding: bool,
}

fn slots() -> &'static Mutex<HashMap<Uuid, Slot>> {
    static SLOTS: OnceLock<Mutex<HashMap<Uuid, Slot>>> = OnceLock::new();
    SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn analyzers() -> (TextAnalyzer, TextAnalyzer, TextAnalyzer) {
    let exact = TextAnalyzer::builder(RawTokenizer::default()).filter(LowerCaser).build();
    let gram = TextAnalyzer::builder(NgramTokenizer::new(2, GRAM, false).expect("a valid n-gram range"))
        .filter(LowerCaser)
        .build();
    let words = TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(LowerCaser)
        .filter(AsciiFoldingFilter)
        .build();
    (exact, gram, words)
}

fn schema() -> (Schema, Fields) {
    let mut b = Schema::builder();
    let text = |tokenizer: &str| {
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(tokenizer)
                .set_index_option(IndexRecordOption::WithFreqs),
        )
    };
    let fields = Fields {
        kind: b.add_text_field("kind", STRING | STORED),
        id: b.add_text_field("id", STORED),
        title: b.add_text_field("title", STORED),
        detail: b.add_text_field("detail", STORED),
        site: b.add_text_field("site", STRING | STORED),
        exact: b.add_text_field("exact", text("exact")),
        gram: b.add_text_field("gram", text("gram")),
        words: b.add_text_field("words", text("words")),
    };
    (b.build(), fields)
}

/// One thing to find: what it is, where it goes, what it is called by.
struct Entry {
    kind: &'static str,
    id: Uuid,
    title: String,
    detail: String,
    site: Option<Uuid>,
    /// Codes and numbers: matched exactly and by any piece.
    codes: Vec<String>,
    /// Words: matched by their starts.
    words: Vec<String>,
}

/// Read a tenant's bins, items and orders.
async fn entries(state: &web::Data<AppState>, tenant: Uuid) -> Result<Vec<Entry>, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let mut out = vec![];
                for r in tx
                    .query(
                        "SELECT i.id, i.code, i.description, s.code,
                                (SELECT array_agg(b.barcode) FROM item_barcode b WHERE b.item_id = i.id),
                                -- Its article numbers, printed where its code isn't (D237),
                                -- each of several said in one (D238).
                                (SELECT array_agg(x) FROM reported_item_said r
                                   CROSS JOIN LATERAL unnest(art_numbers_in(r.value)) x
                                  WHERE r.item_id = i.id AND r.role = 'art_no')
                           FROM item i LEFT JOIN item_style s ON s.id = i.style_id
                          WHERE i.active",
                        &[],
                    )
                    .await?
                {
                    let code: String = r.get(1);
                    let description: String = r.get(2);
                    let style: Option<String> = r.get(3);
                    let mut codes = vec![code.clone()];
                    codes.extend(r.get::<_, Option<Vec<String>>>(4).unwrap_or_default());
                    codes.extend(style.clone());
                    codes.extend(r.get::<_, Option<Vec<String>>>(5).unwrap_or_default());
                    out.push(Entry {
                        kind: "item",
                        id: r.get(0),
                        title: code,
                        detail: description.clone(),
                        site: None,
                        codes,
                        words: vec![description],
                    });
                }
                for r in tx
                    .query(
                        "SELECT l.id, l.code, l.site_id, p.name, l.kind
                           FROM location l LEFT JOIN place p ON p.id = l.place_id
                          WHERE l.active",
                        &[],
                    )
                    .await?
                {
                    let code: String = r.get(1);
                    let place: Option<String> = r.get(3);
                    let kind: String = r.get(4);
                    let detail = match &place {
                        Some(p) => format!("Bin · {p}"),
                        None => format!("Bin · {}", kind.replace('_', " ")),
                    };
                    out.push(Entry {
                        kind: "bin",
                        id: r.get(0),
                        title: code.clone(),
                        detail,
                        site: r.get(2),
                        codes: vec![code],
                        words: place.into_iter().collect(),
                    });
                }
                for r in tx
                    .query(
                        "SELECT o.id, o.confirmation_number, o.external_ref, p.name, o.site_id,
                                (SELECT array_agg(f.reference) FROM fulfilment f
                                  WHERE f.order_id = o.id AND f.reference IS NOT NULL)
                           FROM \"order\" o LEFT JOIN party p ON p.id = o.customer_party_id",
                        &[],
                    )
                    .await?
                {
                    let number: Option<String> = r.get(1);
                    let po: Option<String> = r.get(2);
                    let customer: Option<String> = r.get(3);
                    let mut codes: Vec<String> = number.iter().chain(po.iter()).cloned().collect();
                    codes.extend(r.get::<_, Option<Vec<String>>>(5).unwrap_or_default());
                    let detail = [customer.clone(), po.as_ref().map(|p| format!("PO {p}"))]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · ");
                    out.push(Entry {
                        kind: "order",
                        id: r.get(0),
                        title: number.unwrap_or_else(|| "Order".into()),
                        detail,
                        site: r.get(4),
                        codes,
                        words: customer.into_iter().collect(),
                    });
                }
                Ok(out)
            })
        })
        .await
}

/// Build an index from entries. Pure: no database, so a test can build one.
fn build(entries: &[Entry]) -> tantivy::Result<Built> {
    let (schema, fields) = schema();
    let index = Index::create_in_ram(schema);
    let (exact, gram, words) = analyzers();
    index.tokenizers().register("exact", exact);
    index.tokenizers().register("gram", gram);
    index.tokenizers().register("words", words);
    let mut writer = index.writer_with_num_threads(1, 30_000_000)?;
    for e in entries {
        let mut d = doc!(
            fields.kind => e.kind,
            fields.id => e.id.to_string(),
            fields.title => e.title.as_str(),
            fields.detail => e.detail.as_str(),
            fields.site => e.site.map(|s| s.to_string()).unwrap_or_default(),
        );
        for c in &e.codes {
            d.add_text(fields.exact, c);
            d.add_text(fields.gram, c);
        }
        for w in &e.words {
            d.add_text(fields.words, w);
        }
        writer.add_document(d)?;
    }
    writer.commit()?;
    let reader = index.reader()?;
    Ok(Built { reader, fields, at: Instant::now() })
}

/// The query for what was typed.
fn query_for(built: &Built, typed: &str) -> Option<Box<dyn Query>> {
    let f = &built.fields;
    let q = typed.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let mut should: Vec<(Occur, Box<dyn Query>)> = vec![(
        Occur::Should,
        Box::new(BoostQuery::new(
            Box::new(TermQuery::new(Term::from_field_text(f.exact, &q), IndexRecordOption::Basic)),
            20.0,
        )),
    )];
    let piece = |s: &str| -> Option<Box<dyn Query>> {
        let n = s.chars().count();
        (2..=GRAM).contains(&n).then(|| {
            Box::new(TermQuery::new(Term::from_field_text(f.gram, s), IndexRecordOption::WithFreqs)) as Box<dyn Query>
        })
    };
    if let Some(p) = piece(&q) {
        should.push((Occur::Should, Box::new(BoostQuery::new(p, 4.0))));
    }
    // Every word, each by its start or as a piece of a code.
    let words: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    if !words.is_empty() {
        let all: Vec<(Occur, Box<dyn Query>)> = words
            .iter()
            .map(|w| {
                let typo = if w.chars().count() >= 5 { 1 } else { 0 };
                let mut either: Vec<(Occur, Box<dyn Query>)> = vec![(
                    Occur::Should,
                    Box::new(FuzzyTermQuery::new_prefix(Term::from_field_text(f.words, w), typo, true)),
                )];
                if let Some(p) = piece(w) {
                    either.push((Occur::Should, p));
                }
                (Occur::Must, Box::new(BooleanQuery::new(either)) as Box<dyn Query>)
            })
            .collect();
        should.push((Occur::Should, Box::new(BooleanQuery::new(all))));
    }
    Some(Box::new(BooleanQuery::new(should)))
}

/// One thing found.
#[derive(Serialize, Debug, Clone)]
pub struct Found {
    /// `item`, `bin` or `order`.
    pub kind: String,
    pub id: Uuid,
    pub title: String,
    pub detail: String,
    /// Where it opens.
    pub path: String,
    pub score: f32,
}

#[derive(Serialize, Debug)]
pub struct SearchAnswer {
    pub results: Vec<Found>,
}

/// The best matches, the most of each kind capped so one kind does not crowd
/// out the others, and bins only at the site being worked at.
fn search(built: &Built, typed: &str, site: Option<Uuid>, limit: usize) -> tantivy::Result<Vec<Found>> {
    let Some(query) = query_for(built, typed) else { return Ok(vec![]) };
    let searcher = built.reader.searcher();
    let top = searcher.search(&query, &TopDocs::with_limit(limit * 6).order_by_score())?;
    let f = &built.fields;
    let site = site.map(|s| s.to_string());
    let mut per: HashMap<String, usize> = HashMap::new();
    let mut out = vec![];
    for (score, at) in top {
        let d: TantivyDocument = searcher.doc(at)?;
        let text = |field: Field| d.get_first(field).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let kind = text(f.kind);
        if kind == "bin" {
            if let Some(s) = &site {
                if &text(f.site) != s {
                    continue;
                }
            }
        }
        let n = per.entry(kind.clone()).or_default();
        if *n >= limit.div_ceil(2).max(3) {
            continue;
        }
        *n += 1;
        let Ok(id) = Uuid::parse_str(&text(f.id)) else { continue };
        let path = match kind.as_str() {
            "item" => format!("/items/{id}"),
            "bin" => format!("/bins/{id}"),
            _ => format!("/orders/{id}"),
        };
        out.push(Found { kind, id, title: text(f.title), detail: text(f.detail), path, score });
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

/// The tenant's index: built now when there is none, rebuilt behind the search
/// when it is older than a minute.
async fn index_for(state: &web::Data<AppState>, tenant: Uuid) -> Result<Arc<Built>, ApiError> {
    let (current, stale) = {
        let mut slots = slots().lock().expect("the search slots");
        let slot = slots.entry(tenant).or_insert(Slot { built: None, rebuilding: false });
        let stale = slot.built.as_ref().is_some_and(|b| b.at.elapsed() > FRESH) && !slot.rebuilding;
        if stale {
            slot.rebuilding = true;
        }
        (slot.built.clone(), stale)
    };
    if let Some(built) = current {
        if stale {
            let state = state.clone();
            actix_web::rt::spawn(async move {
                let fresh = rebuild(&state, tenant).await;
                let mut slots = slots().lock().expect("the search slots");
                if let Some(slot) = slots.get_mut(&tenant) {
                    if let Ok(b) = fresh {
                        slot.built = Some(b);
                    }
                    slot.rebuilding = false;
                }
            });
        }
        return Ok(built);
    }
    let built = rebuild(state, tenant).await?;
    slots().lock().expect("the search slots").insert(tenant, Slot { built: Some(built.clone()), rebuilding: false });
    Ok(built)
}

async fn rebuild(state: &web::Data<AppState>, tenant: Uuid) -> Result<Arc<Built>, ApiError> {
    let read = entries(state, tenant).await?;
    let built = web::block(move || build(&read))
        .await
        .map_err(|e| ApiError::Rejected(format!("search index: {e}")))?
        .map_err(|e| ApiError::Rejected(format!("search index: {e}")))?;
    Ok(Arc::new(built))
}

#[derive(Deserialize, Debug)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
}

/// Bins, items and orders matching what was typed, best first (D189).
#[get("/search")]
pub async fn global_search(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<SearchQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let typed = query.q.clone().unwrap_or_default();
    let limit = query.limit.unwrap_or(12).clamp(1, 50);
    if typed.trim().is_empty() {
        return Ok(HttpResponse::Ok().json(SearchAnswer { results: vec![] }));
    }
    let built = index_for(&state, who.tenant_id).await?;
    let site = who.site_id;
    let results = web::block(move || search(&built, &typed, site, limit))
        .await
        .map_err(|e| ApiError::Rejected(format!("search: {e}")))?
        .map_err(|e| ApiError::Rejected(format!("search: {e}")))?;
    Ok(HttpResponse::Ok().json(SearchAnswer { results }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &'static str, title: &str, detail: &str, codes: &[&str], words: &[&str]) -> Entry {
        Entry {
            kind,
            id: Uuid::new_v4(),
            title: title.into(),
            detail: detail.into(),
            site: None,
            codes: codes.iter().map(|s| s.to_string()).collect(),
            words: words.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn index() -> Built {
        build(&[
            entry("item", "ABC-3010W-10", "Cut liner glove, white, XL", &["ABC-3010W-10", "9312345000017"], &["Cut liner glove, white, XL"]),
            entry("item", "ABC-3010W-08", "Cut liner glove, white, M", &["ABC-3010W-08"], &["Cut liner glove, white, M"]),
            entry("item", "XYZ-0829", "Bin liner, 82 L, black", &["XYZ-0829"], &["Bin liner, 82 L, black"]),
            entry("bin", "F-20-01", "Bin · Rack F", &["F-20-01"], &["Rack F"]),
            entry("order", "S100200", "North Foods · PO 4412", &["S100200", "4412", "IF9001"], &["North Foods"]),
        ])
        .expect("an index")
    }

    fn titles(built: &Built, q: &str) -> Vec<String> {
        search(built, q, None, 12).unwrap().into_iter().map(|f| f.title).collect()
    }

    #[test]
    fn a_code_is_found_whole_and_by_any_piece_of_it() {
        let b = index();
        assert_eq!(titles(&b, "ABC-3010W-10")[0], "ABC-3010W-10", "whole, first");
        let piece = titles(&b, "3010");
        assert!(piece.contains(&"ABC-3010W-10".to_string()) && piece.contains(&"ABC-3010W-08".to_string()), "{piece:?}");
        assert_eq!(titles(&b, "f-20")[0], "F-20-01", "a bin by part of its code, any case");
        assert_eq!(titles(&b, "9312345000017")[0], "ABC-3010W-10", "a barcode");
    }

    #[test]
    fn words_are_found_by_their_starts_and_all_must_match() {
        let b = index();
        let gloves = titles(&b, "glove white");
        assert_eq!(gloves.len(), 2, "{gloves:?}");
        assert_eq!(titles(&b, "bin lin")[0], "XYZ-0829", "the start of each word");
        assert!(titles(&b, "glvoe").iter().any(|t| t.starts_with("ABC")), "a typo in a longer word is forgiven");
        assert!(titles(&b, "glove black").is_empty(), "every word has to match");
    }

    #[test]
    fn an_order_is_found_by_its_number_its_po_its_fulfilment_and_its_customer() {
        let b = index();
        for q in ["S100200", "4412", "IF9001", "north"] {
            assert_eq!(titles(&b, q)[0], "S100200", "{q}");
        }
    }
}

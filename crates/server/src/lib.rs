//! The decisions the write paths are made of, and the thin read surface, as a library.
//!
//! `main.rs` is the service; this is what it is built from. Split out by D89 for
//! a reason worth stating: the disposition rules D88 wrote could not be tested
//! against the database at all, because a binary crate exposes nothing and the
//! suite that owns the fixture lives elsewhere. **A rule nothing outside its own
//! file can reach is a rule with one reader.** The same split now holds for the
//! read routes: the suite reaches them without starting a process.

pub mod adjusting;
pub mod assets;
pub mod auth;
pub mod barcodes;
pub mod baseline;
pub mod bench;
pub mod binding;
pub mod bins;
pub mod allocating;
pub mod capture;
pub mod client_events;
pub mod correction;
pub mod counting;
pub mod credentials;
pub mod despatch;
pub mod despatching;
pub mod findings;
pub mod error;
pub mod health;
pub mod images;
pub mod importing;
pub mod ledger_views;
pub mod locator;
pub mod moving;
pub mod observing;
pub mod orders;
pub mod prepack;
pub mod packages;
pub mod packing;
pub mod passkeys;
pub mod picking;
pub mod putaway_list;
pub mod receiving_list;
pub mod picking_list;
pub mod pictures;
pub mod receiving;
pub mod revalidation;
pub mod routes;
pub mod setup;
pub mod tenancy;
pub mod tokens;
pub mod web;
pub mod work;
pub mod workspace;

use deadpool_postgres::Pool;

/// Shared application state for the HTTP service and its tests.
pub struct AppState {
    pub pool: Pool,
}

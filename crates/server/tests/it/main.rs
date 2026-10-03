//! Every server integration test, as one test program.
//!
//! One binary rather than forty. Each file used to be its own, and each linked
//! the whole crate and compiled its own copy of the same generic web and
//! database code. Built once, a change to the server rebuilds one program.
//! Run one file with a filter: `cargo test -p spork-server --test it pack_walk_http::`.

mod common;

mod act_idempotency;
mod allocation_replay;
mod api_mount;
mod baseline_read;
mod binding_over_http;
mod backup_http;
mod capture_walk;
mod capture_worklist;
mod cartons_and_lists_http;
mod change_password;
mod consignments;
mod evidence_and_pictures;
mod fulfilment_intake;
mod handover_http;
mod happy_path;
mod import_over_http;
mod item_http;
mod ledger_http;
mod live_http;
mod locator;
mod observations;
mod order_search;
mod outbound_reads;
mod own_carton_http;
mod packaging_http;
mod pack_walk_http;
mod people_http;
mod packing_list;
mod passkeys_http;
mod picking_http;
mod places_http;
mod presentation_and_parts;
mod putaway_http;
mod receipt_disposition;
mod receipt_entered;
mod receipt_header;
mod receipt_http;
mod receiving_list_http;
mod reported_stock_import;
mod search_http;
mod setup_first_administrator;
mod ships_as_is_http;
mod sign_on;
mod tenancy;
mod weighing_http;
mod where_you_are_working;
mod workspace_http;

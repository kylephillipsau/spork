//! Loading reference data somebody exported from the system of record.
//!
//! One module per file the export produces, because they are different files
//! with different judgement in them: a bin list decides what a code means and
//! which warehouse is ours; an item master is a code and a description and
//! almost nothing to decide.
//!
//! What they share is the shape, and it is worth stating once:
//!
//! - **`read`** takes anything readable, so a path and a request body are the
//!   same thing to it.
//! - **`survey`** is pure. It says what the file contains before any database
//!   is consulted, which is what a person reads before deciding.
//! - **`load`** does the writes and rolls them back when it was not told to
//!   apply, so the dry run cannot disagree with the apply — it *is* the apply,
//!   undone.
//!
//! Each is called by two callers: the `import_*` example and the endpoint
//! behind the Import screen. An importer that exists twice is an importer whose
//! copies disagree about a warehouse.
//!
//! A fourth thing they share, added after the first two were written:
//! **what arrived is stored before it is read.** See [`received`], which is the
//! reason `party_message` has a writer at all.

pub mod bins;
pub mod items;
pub mod received;
pub mod stock;

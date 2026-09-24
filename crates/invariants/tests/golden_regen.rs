//! Regenerating the golden snapshot, which is deliberately not automatic.
//!
//! Run with `SPORK_REGENERATE_GOLDEN=1` after bumping `RESOLVER_VERSION`.
//! Ordinary runs do nothing, because a snapshot that rewrites itself when the
//! answers change is a snapshot that watches nothing.
#[test]
fn regenerate_when_asked() {
    if std::env::var("SPORK_REGENERATE_GOLDEN").is_err() {
        return;
    }
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let mut c = spork_invariants::connect_exclusive(&url);
    let text = spork_invariants::golden::render(&mut c).expect("render");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("fixtures/resolver-golden.txt");
    std::fs::write(&path, text).expect("write");
    eprintln!("wrote {}", path.display());
}

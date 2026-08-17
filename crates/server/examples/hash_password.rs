//! Hash a password the way `person_credential.phc` stores it.
//!
//! An example rather than a binary: it is a development convenience for setting
//! a demo credential, not something the image should ship.
fn main() {
    let Some(pw) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run -p nylonite-server --example hash_password -- <password>");
        std::process::exit(2);
    };
    println!("{}", nylonite_server::auth::hash_password(&pw).expect("hash"));
}

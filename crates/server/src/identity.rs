//! A host is known by its own authority, and found by its own name. D255.
//!
//! The handhelds reach the host over a network nobody here configures: the
//! address comes from DHCP and may move, and the router's DNS cannot be relied
//! on. So a host is found by the name it announces itself (mDNS,
//! `<computer>.local`), wherever its address goes, and trusted because its
//! certificate is signed by an authority the device was paired with: Spork's
//! own, made on first start and kept in the state directory. HTTPS on that name
//! is what lets the application be installed and start from the device.
//!
//! The certificate and key are written to the state directory, where the
//! HTTPS in front of Spork reads them (`scripts/local.ps1` starts Caddy).
//!
//! **The authority may sign only `.local` names** (a critical name constraint),
//! so a copy of its key could not stand in for any other site. It lasts ten
//! years; the host's certificate a year, made again by itself thirty days
//! before it ends, or when the host's name changes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use actix_web::{get, http::header, web, HttpResponse};
use maud::{html, DOCTYPE};
use openssl::asn1::Asn1Time;
use openssl::bn::{BigNum, MsbOption};
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::x509::extension::{
    AuthorityKeyIdentifier, BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName, SubjectKeyIdentifier,
};
use openssl::x509::{X509Extension, X509NameBuilder, X509};

/// What a device is shown to pair with this host.
pub struct HostIdentity {
    /// `desktop-0dstvgq.local`: what the host announces, and its certificate names.
    pub name: String,
    pub authority_pem: Vec<u8>,
    /// SHA-256 of the authority, colon-separated, to compare on the device and the desk.
    pub fingerprint: String,
}

type Fail = std::io::Error;

fn fail(what: &str, e: impl std::fmt::Display) -> Fail {
    std::io::Error::other(format!("{what}: {e}"))
}

/// The name this host announces: `SPORK_HOST_NAME` when set, else the
/// computer's name, as mDNS gives it, in lower case.
pub fn host_name() -> String {
    let base = std::env::var("SPORK_HOST_NAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "spork".into());
    let base = base.trim().trim_end_matches(".local").to_lowercase();
    format!("{base}.local")
}

fn directory() -> PathBuf {
    crate::setup::state_directory().join("identity")
}

/// The authority and this host's certificate, made or renewed as needed.
pub fn ensure(name: &str) -> Result<HostIdentity, Fail> {
    let dir = directory();
    std::fs::create_dir_all(&dir)?;
    let (authority, authority_key) = match read(&dir, "authority") {
        Some(pair) => pair,
        None => {
            let pair = make_authority()?;
            write(&dir, "authority", &pair)?;
            pair
        }
    };
    if read(&dir, "host").filter(|(cert, _)| fresh(cert, name)).is_none() {
        let (cert, key) = make_host(name, &authority, &authority_key)?;
        // The chain the HTTPS in front serves: this host's, then the authority's.
        let mut chain = cert.to_pem().map_err(|e| fail("host", e))?;
        chain.extend(authority.to_pem().map_err(|e| fail("host", e))?);
        write(&dir, "host", &(cert, key))?;
        std::fs::write(dir.join("host-chain.crt"), chain)?;
        tracing::info!(%name, "made this host's certificate");
    }
    let der = authority.to_der().map_err(|e| fail("authority", e))?;
    let digest = openssl::hash::hash(MessageDigest::sha256(), &der).map_err(|e| fail("fingerprint", e))?;
    Ok(HostIdentity {
        name: name.to_string(),
        authority_pem: authority.to_pem().map_err(|e| fail("authority", e))?,
        fingerprint: fingerprint(&digest),
    })
}

/// Colon-separated hex, as a phone's certificate screen shows it.
pub fn fingerprint(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

/// Still good for more than thirty days, and for this name.
fn fresh(cert: &X509, name: &str) -> bool {
    let named = cert
        .subject_alt_names()
        .is_some_and(|names| names.iter().any(|n| n.dnsname() == Some(name)));
    let soon = Asn1Time::days_from_now(30).ok();
    named && soon.is_some_and(|soon| cert.not_after() > soon)
}

fn read(dir: &Path, which: &str) -> Option<(X509, PKey<Private>)> {
    let cert = X509::from_pem(&std::fs::read(dir.join(format!("{which}.crt"))).ok()?).ok()?;
    let key = PKey::private_key_from_pem(&std::fs::read(dir.join(format!("{which}.key"))).ok()?).ok()?;
    Some((cert, key))
}

fn write(dir: &Path, which: &str, (cert, key): &(X509, PKey<Private>)) -> Result<(), Fail> {
    std::fs::write(dir.join(format!("{which}.key")), key.private_key_to_pem_pkcs8().map_err(|e| fail(which, e))?)?;
    std::fs::write(dir.join(format!("{which}.crt")), cert.to_pem().map_err(|e| fail(which, e))?)?;
    Ok(())
}

fn new_key() -> Result<PKey<Private>, Fail> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).map_err(|e| fail("key", e))?;
    PKey::from_ec_key(EcKey::generate(&group).map_err(|e| fail("key", e))?).map_err(|e| fail("key", e))
}

fn serial() -> Result<openssl::asn1::Asn1Integer, Fail> {
    let mut n = BigNum::new().map_err(|e| fail("serial", e))?;
    n.rand(127, MsbOption::MAYBE_ZERO, false).map_err(|e| fail("serial", e))?;
    n.to_asn1_integer().map_err(|e| fail("serial", e))
}

/// Spork's own authority: ten years, signing only `.local` names.
#[allow(deprecated)] // NAME_CONSTRAINTS has no builder in the openssl crate; the config string is its documented form.
fn make_authority() -> Result<(X509, PKey<Private>), Fail> {
    let f = |e: openssl::error::ErrorStack| fail("authority", e);
    let key = new_key()?;
    let mut name = X509NameBuilder::new().map_err(f)?;
    name.append_entry_by_text("O", "Spork").map_err(f)?;
    name.append_entry_by_text("CN", "Spork local authority").map_err(f)?;
    let name = name.build();
    let mut b = X509::builder().map_err(f)?;
    b.set_version(2).map_err(f)?;
    let number = serial()?;
    b.set_serial_number(&number).map_err(f)?;
    b.set_subject_name(&name).map_err(f)?;
    b.set_issuer_name(&name).map_err(f)?;
    b.set_pubkey(&key).map_err(f)?;
    let (from, until) = (Asn1Time::days_from_now(0).map_err(f)?, Asn1Time::days_from_now(3650).map_err(f)?);
    b.set_not_before(&from).map_err(f)?;
    b.set_not_after(&until).map_err(f)?;
    b.append_extension(BasicConstraints::new().critical().ca().pathlen(0).build().map_err(f)?).map_err(f)?;
    b.append_extension(KeyUsage::new().critical().key_cert_sign().crl_sign().build().map_err(f)?).map_err(f)?;
    let ski = SubjectKeyIdentifier::new().build(&b.x509v3_context(None, None)).map_err(f)?;
    b.append_extension(ski).map_err(f)?;
    let only_local = X509Extension::new_nid(None, Some(&b.x509v3_context(None, None)), Nid::NAME_CONSTRAINTS, "critical,permitted;DNS:local")
        .map_err(f)?;
    b.append_extension(only_local).map_err(f)?;
    b.sign(&key, MessageDigest::sha256()).map_err(f)?;
    Ok((b.build(), key))
}

/// This host's certificate, for its announced name, for a year.
fn make_host(host: &str, authority: &X509, authority_key: &PKey<Private>) -> Result<(X509, PKey<Private>), Fail> {
    let f = |e: openssl::error::ErrorStack| fail("host certificate", e);
    let key = new_key()?;
    let mut name = X509NameBuilder::new().map_err(f)?;
    name.append_entry_by_text("CN", host).map_err(f)?;
    let name = name.build();
    let mut b = X509::builder().map_err(f)?;
    b.set_version(2).map_err(f)?;
    let number = serial()?;
    b.set_serial_number(&number).map_err(f)?;
    b.set_subject_name(&name).map_err(f)?;
    b.set_issuer_name(authority.subject_name()).map_err(f)?;
    b.set_pubkey(&key).map_err(f)?;
    let (from, until) = (Asn1Time::days_from_now(0).map_err(f)?, Asn1Time::days_from_now(365).map_err(f)?);
    b.set_not_before(&from).map_err(f)?;
    b.set_not_after(&until).map_err(f)?;
    b.append_extension(BasicConstraints::new().build().map_err(f)?).map_err(f)?;
    b.append_extension(KeyUsage::new().critical().digital_signature().build().map_err(f)?).map_err(f)?;
    b.append_extension(ExtendedKeyUsage::new().server_auth().build().map_err(f)?).map_err(f)?;
    let san = SubjectAlternativeName::new().dns(host).build(&b.x509v3_context(Some(authority), None)).map_err(f)?;
    b.append_extension(san).map_err(f)?;
    let ski = SubjectKeyIdentifier::new().build(&b.x509v3_context(Some(authority), None)).map_err(f)?;
    b.append_extension(ski).map_err(f)?;
    let aki = AuthorityKeyIdentifier::new().keyid(false).build(&b.x509v3_context(Some(authority), None)).map_err(f)?;
    b.append_extension(aki).map_err(f)?;
    b.sign(authority_key, MessageDigest::sha256()).map_err(f)?;
    Ok((b.build(), key))
}

/// What the pairing pages need of the identity.
pub struct Pairing {
    pub name: String,
    pub authority_pem: Vec<u8>,
    pub fingerprint: String,
}

impl From<&HostIdentity> for Pairing {
    fn from(id: &HostIdentity) -> Self {
        Pairing { name: id.name.clone(), authority_pem: id.authority_pem.clone(), fingerprint: id.fingerprint.clone() }
    }
}

pub fn configure(cfg: &mut web::ServiceConfig, pairing: Arc<Pairing>) {
    cfg.app_data(web::Data::from(pairing)).service(pair_page).service(authority_file);
}

/// The authority, to install on a device. A certificate is public: anyone may have it.
#[get("/pair/spork-authority.crt")]
async fn authority_file(pairing: web::Data<Pairing>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/x-x509-ca-cert")
        .insert_header((header::CONTENT_DISPOSITION, "attachment; filename=\"spork-authority.crt\""))
        .body(pairing.authority_pem.clone())
}

/// Pairing a device (D255): what to install, the fingerprint to check it by,
/// and where to go after. Plain HTTP serves it to a device not yet paired.
#[get("/pair")]
async fn pair_page(pairing: web::Data<Pairing>) -> HttpResponse {
    let https = format!("https://{}/", pairing.name);
    let page = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Pair with Spork" }
                style { (PAIR_CSS) }
            }
            body {
                main {
                    h1 { "Pair this device with Spork" }
                    p { "Once, on each phone or computer: then it opens Spork securely at "
                        strong { (pairing.name) } ", wherever this host is on the network, and can install it as an app." }
                    ol {
                        li { a href="/pair/spork-authority.crt" { "Download Spork's authority" } "." }
                        li { "Install it. On Android: " em { "Settings › Security › Encryption & credentials › Install a certificate › CA certificate" }
                             ", and choose " strong { "spork-authority.crt" } ". On an iPhone: open the download, then "
                             em { "Settings › General › About › Certificate Trust Settings" } " and turn it on." }
                        li { "Check its fingerprint, shown on the phone's certificate screen, matches this one on the desk:"
                             code { (pairing.fingerprint) } }
                        li { "Open " a href=(https) { (https) } ", sign in, and choose " em { "Install app" } " from the browser's menu." }
                        li { "To sign in with a fingerprint or face, add a passkey there under your name › " em { "Passkeys" }
                             ". One added on another device or at another address does not work here." }
                    }
                    p.note { "The authority can vouch only for names ending in .local, so it cannot stand in for any other site." }
                }
            }
        }
    };
    HttpResponse::Ok().content_type("text/html; charset=utf-8").body(page.into_string())
}

const PAIR_CSS: &str = "body{margin:0;font:16px/1.5 system-ui,sans-serif;color:#141413;background:#f7f7f5}\
main{max-width:640px;margin:0 auto;padding:24px 16px}h1{font-size:24px;margin:0 0 12px}\
li{margin:0 0 12px}code{display:block;margin-top:6px;padding:8px;background:#fff;border:1px solid #ddd;\
border-radius:6px;font-size:13px;word-break:break-all}.note{color:#555;font-size:14px}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_authority_signs_this_hosts_name_and_only_local_names() {
        let (authority, key) = make_authority().unwrap();
        let constraints = authority.to_text().unwrap();
        let text = String::from_utf8_lossy(&constraints);
        assert!(text.contains("X509v3 Name Constraints: critical"), "{text}");
        assert!(text.contains("DNS:local"), "{text}");
        let (host, _) = make_host("desk-1.local", &authority, &key).unwrap();
        assert!(host.verify(&authority.public_key().unwrap()).unwrap(), "signed by the authority");
        assert!(fresh(&host, "desk-1.local"), "good for a year, for its name");
        assert!(!fresh(&host, "desk-2.local"), "renamed, it is made again");
    }

    #[actix_web::test]
    async fn a_device_is_offered_the_authority_and_its_fingerprint() {
        use actix_web::{test, App};
        let pairing = Arc::new(Pairing { name: "desk-1.local".into(), authority_pem: b"PEM".to_vec(), fingerprint: "0A:FF".into() });
        let app = test::init_service(App::new().configure(|cfg| configure(cfg, pairing))).await;
        let page = test::call_and_read_body(&app, test::TestRequest::get().uri("/pair").to_request()).await;
        let page = String::from_utf8_lossy(&page);
        assert!(page.contains("0A:FF") && page.contains("https://desk-1.local/"), "{page}");
        let file = test::call_service(&app, test::TestRequest::get().uri("/pair/spork-authority.crt").to_request()).await;
        assert_eq!(file.headers().get("content-type").unwrap(), "application/x-x509-ca-cert");
        assert_eq!(test::read_body(file).await.as_ref(), b"PEM");
    }

    #[test]
    fn a_fingerprint_reads_as_a_phone_shows_it() {
        assert_eq!(fingerprint(&[0x0a, 0xff]), "0A:FF");
    }
}

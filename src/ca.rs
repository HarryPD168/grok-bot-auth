use std::path::PathBuf;
use std::process::Command;

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use time::{Duration, OffsetDateTime};

use crate::error::{Error, Result};
use crate::store;

pub struct LoadedCa {
    pub issuer: Issuer<'static, KeyPair>,
    pub cert_pem: String,
}

fn ca_dir() -> PathBuf {
    store::data_dir().join("ca")
}

pub fn cert_path() -> PathBuf {
    ca_dir().join("ca.crt")
}

fn key_path() -> PathBuf {
    ca_dir().join("ca.key")
}

pub fn ensure_ca() -> Result<LoadedCa> {
    std::fs::create_dir_all(ca_dir())?;
    if !cert_path().exists() || !key_path().exists() {
        generate()?;
    }
    load()
}

fn generate() -> Result<()> {
    let key = KeyPair::generate().map_err(|error| Error::Msg(format!("CA key: {error}")))?;
    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|error| Error::Msg(format!("CA params: {error}")))?;
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, "grok-bot-auth local CA");
    name.push(DnType::OrganizationName, "grok-bot-auth");
    params.distinguished_name = name;
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
    ];
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::minutes(5);
    params.not_after = now + Duration::days(3650);
    let cert = params
        .self_signed(&key)
        .map_err(|error| Error::Msg(format!("CA cert: {error}")))?;
    std::fs::write(key_path(), key.serialize_pem())?;
    std::fs::write(cert_path(), cert.pem())?;
    Ok(())
}

fn load() -> Result<LoadedCa> {
    let cert_pem = std::fs::read_to_string(cert_path())?;
    let key_pem = std::fs::read_to_string(key_path())?;
    let key =
        KeyPair::from_pem(&key_pem).map_err(|error| Error::Msg(format!("CA key pem: {error}")))?;
    let issuer = Issuer::from_ca_cert_pem(&cert_pem, key)
        .map_err(|error| Error::Msg(format!("CA issuer: {error}")))?;
    Ok(LoadedCa { issuer, cert_pem })
}

pub fn install_user_root() -> Result<String> {
    let output = Command::new("certutil")
        .args([
            "-user",
            "-addstore",
            "-f",
            "Root",
            &cert_path().to_string_lossy(),
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(Error::Msg(format!("certutil failed: {stdout}{stderr}")));
    }
    Ok(stdout)
}

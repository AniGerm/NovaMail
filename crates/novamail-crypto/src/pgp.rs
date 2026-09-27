//! OpenPGP helpers backed by Sequoia (sign / encrypt / decrypt / verify).

use std::io::{self, Write};

use anyhow::anyhow;
use sequoia_openpgp::armor;
use sequoia_openpgp::cert::prelude::*;
use sequoia_openpgp::crypto::SessionKey;
use sequoia_openpgp::parse::stream::*;
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::policy::{Policy, StandardPolicy};
use sequoia_openpgp::serialize::stream::*;
use sequoia_openpgp::serialize::SerializeInto;
use sequoia_openpgp::types::SymmetricAlgorithm;
use sequoia_openpgp::Cert;

use crate::{CryptoError, CryptoResult};

#[derive(Debug, Clone)]
pub struct PgpKeyInfo {
    pub fingerprint: String,
    pub user_ids: Vec<String>,
    pub has_secret: bool,
    pub armored_public: String,
    pub armored_secret: Option<String>,
}

pub fn generate_key(userid: &str) -> CryptoResult<PgpKeyInfo> {
    let (cert, _rev) = CertBuilder::new()
        .add_userid(userid)
        .add_signing_subkey()
        .add_transport_encryption_subkey()
        .generate()
        .map_err(|e| CryptoError::Pgp(e.to_string()))?;
    cert_to_info(&cert)
}

pub fn import_armored(armored: &str) -> CryptoResult<PgpKeyInfo> {
    let cert = Cert::from_bytes(armored.as_bytes())
        .map_err(|e| CryptoError::Pgp(format!("import failed: {e}")))?;
    cert_to_info(&cert)
}

pub fn cert_to_info(cert: &Cert) -> CryptoResult<PgpKeyInfo> {
    let fingerprint = cert.fingerprint().to_string();
    let user_ids = cert
        .userids()
        .map(|u| String::from_utf8_lossy(u.userid().value()).into_owned())
        .collect();
    let has_secret = cert.is_tsk();
    let armored_public = String::from_utf8(
        cert.armored()
            .to_vec()
            .map_err(|e| CryptoError::Pgp(e.to_string()))?,
    )
    .map_err(|e| CryptoError::Pgp(e.to_string()))?;
    let armored_secret = if has_secret {
        Some(
            String::from_utf8(
                cert.as_tsk()
                    .armored()
                    .to_vec()
                    .map_err(|e| CryptoError::Pgp(e.to_string()))?,
            )
            .map_err(|e| CryptoError::Pgp(e.to_string()))?,
        )
    } else {
        None
    };
    Ok(PgpKeyInfo {
        fingerprint,
        user_ids,
        has_secret,
        armored_public,
        armored_secret,
    })
}

pub fn parse_cert(armored: &str) -> CryptoResult<Cert> {
    Cert::from_bytes(armored.as_bytes()).map_err(|e| CryptoError::Pgp(e.to_string()))
}

/// Sign plaintext; returns ASCII-armored signed message.
pub fn sign_message(plaintext: &str, signer_secret_armored: &str) -> CryptoResult<String> {
    let p = &StandardPolicy::new();
    let cert = parse_cert(signer_secret_armored)?;
    let keypair = cert
        .keys()
        .unencrypted_secret()
        .with_policy(p, None)
        .supported()
        .alive()
        .revoked(false)
        .for_signing()
        .next()
        .ok_or_else(|| CryptoError::Pgp("no signing key".into()))?
        .key()
        .clone()
        .into_keypair()
        .map_err(|e| CryptoError::Pgp(e.to_string()))?;

    let mut sink = Vec::new();
    {
        let mut writer = armor::Writer::new(&mut sink, armor::Kind::Message)
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        let message = Message::new(&mut writer);
        let signer = Signer::new(message, keypair)
            .map_err(|e| CryptoError::Pgp(e.to_string()))?
            .build()
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        let mut literal = LiteralWriter::new(signer)
            .build()
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        literal
            .write_all(plaintext.as_bytes())
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        literal
            .finalize()
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        writer
            .finalize()
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
    }
    String::from_utf8(sink).map_err(|e| CryptoError::Pgp(e.to_string()))
}

/// Encrypt plaintext for one or more recipient public keys.
pub fn encrypt_message(plaintext: &str, recipient_publics: &[&str]) -> CryptoResult<String> {
    let p = &StandardPolicy::new();
    if recipient_publics.is_empty() {
        return Err(CryptoError::Pgp("no recipients for encryption".into()));
    }
    let certs: Vec<Cert> = recipient_publics
        .iter()
        .map(|a| parse_cert(a))
        .collect::<CryptoResult<_>>()?;

    let mut recipients = Vec::new();
    for cert in &certs {
        for key in cert
            .keys()
            .with_policy(p, None)
            .supported()
            .alive()
            .revoked(false)
            .for_transport_encryption()
        {
            recipients.push(key);
        }
    }
    if recipients.is_empty() {
        return Err(CryptoError::Pgp("no encryption-capable keys".into()));
    }

    let mut sink = Vec::new();
    {
        let mut writer = armor::Writer::new(&mut sink, armor::Kind::Message)
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        let message = Message::new(&mut writer);
        let encryptor = Encryptor::for_recipients(message, recipients)
            .build()
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        let mut literal = LiteralWriter::new(encryptor)
            .build()
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        literal
            .write_all(plaintext.as_bytes())
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        literal
            .finalize()
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
        writer
            .finalize()
            .map_err(|e| CryptoError::Pgp(e.to_string()))?;
    }
    String::from_utf8(sink).map_err(|e| CryptoError::Pgp(e.to_string()))
}

#[derive(Debug, Clone)]
pub struct DecryptResult {
    pub plaintext: String,
    pub signature_valid: Option<bool>,
    pub signer_fpr: Option<String>,
}

/// Decrypt an armored message using available secret keys; optionally verify with public certs.
pub fn decrypt_message(
    ciphertext: &str,
    secret_armoreds: &[&str],
    public_armoreds: &[&str],
) -> CryptoResult<DecryptResult> {
    let p = StandardPolicy::new();
    let secrets: Vec<Cert> = secret_armoreds
        .iter()
        .map(|a| parse_cert(a))
        .collect::<CryptoResult<_>>()?;
    let publics: Vec<Cert> = public_armoreds
        .iter()
        .map(|a| parse_cert(a))
        .collect::<CryptoResult<_>>()?;

    let helper = Helper {
        secrets: &secrets,
        publics: &publics,
        policy: &p,
        signature_valid: None,
        signer_fpr: None,
    };

    let mut decryptor = DecryptorBuilder::from_bytes(ciphertext.as_bytes())
        .map_err(|e| CryptoError::Pgp(e.to_string()))?
        .with_policy(&p, None, helper)
        .map_err(|e| CryptoError::Pgp(e.to_string()))?;

    let mut plaintext = Vec::new();
    io::copy(&mut decryptor, &mut plaintext).map_err(|e| CryptoError::Pgp(e.to_string()))?;
    let helper = decryptor.into_helper();

    Ok(DecryptResult {
        plaintext: String::from_utf8_lossy(&plaintext).into_owned(),
        signature_valid: helper.signature_valid,
        signer_fpr: helper.signer_fpr,
    })
}

#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub plaintext: String,
    pub valid: bool,
    pub signer_fpr: Option<String>,
}

pub fn verify_message(signed: &str, public_armoreds: &[&str]) -> CryptoResult<VerifyResult> {
    let p = StandardPolicy::new();
    let publics: Vec<Cert> = public_armoreds
        .iter()
        .map(|a| parse_cert(a))
        .collect::<CryptoResult<_>>()?;

    let helper = VerifyHelper {
        publics: &publics,
        valid: false,
        signer_fpr: None,
    };

    let mut verifier = VerifierBuilder::from_bytes(signed.as_bytes())
        .map_err(|e| CryptoError::Pgp(e.to_string()))?
        .with_policy(&p, None, helper)
        .map_err(|e| CryptoError::Pgp(e.to_string()))?;

    let mut plaintext = Vec::new();
    io::copy(&mut verifier, &mut plaintext).map_err(|e| CryptoError::Pgp(e.to_string()))?;
    let helper = verifier.into_helper();

    Ok(VerifyResult {
        plaintext: String::from_utf8_lossy(&plaintext).into_owned(),
        valid: helper.valid,
        signer_fpr: helper.signer_fpr,
    })
}

pub fn looks_like_pgp(text: &str) -> bool {
    let t = text.trim();
    t.contains("-----BEGIN PGP MESSAGE-----")
        || t.contains("-----BEGIN PGP SIGNED MESSAGE-----")
}

struct Helper<'a> {
    secrets: &'a [Cert],
    publics: &'a [Cert],
    policy: &'a dyn Policy,
    signature_valid: Option<bool>,
    signer_fpr: Option<String>,
}

impl<'a> VerificationHelper for Helper<'a> {
    fn get_certs(
        &mut self,
        _ids: &[sequoia_openpgp::KeyHandle],
    ) -> sequoia_openpgp::Result<Vec<Cert>> {
        Ok(self.publics.to_vec())
    }

    fn check(&mut self, structure: MessageStructure) -> sequoia_openpgp::Result<()> {
        for layer in structure.into_iter() {
            if let MessageLayer::SignatureGroup { results } = layer {
                for result in results {
                    match result {
                        Ok(good) => {
                            self.signature_valid = Some(true);
                            self.signer_fpr = good
                                .sig
                                .issuer_fingerprints()
                                .next()
                                .map(|fp| fp.to_string());
                        }
                        Err(_) => {
                            if self.signature_valid.is_none() {
                                self.signature_valid = Some(false);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'a> DecryptionHelper for Helper<'a> {
    fn decrypt(
        &mut self,
        pkesks: &[sequoia_openpgp::packet::PKESK],
        _skesks: &[sequoia_openpgp::packet::SKESK],
        sym_algo: Option<SymmetricAlgorithm>,
        decrypt: &mut dyn FnMut(Option<SymmetricAlgorithm>, &SessionKey) -> bool,
    ) -> sequoia_openpgp::Result<Option<Cert>> {
        for secret in self.secrets {
            for ka in secret
                .keys()
                .unencrypted_secret()
                .with_policy(self.policy, None)
                .for_transport_encryption()
            {
                let mut pair = ka.key().clone().into_keypair()?;
                for pkesk in pkesks {
                    if let Some((algo, session_key)) = pkesk.decrypt(&mut pair, sym_algo) {
                        if decrypt(algo, &session_key) {
                            return Ok(Some(secret.clone()));
                        }
                    }
                }
            }
        }
        Err(anyhow!("no secret key could decrypt the message").into())
    }
}

struct VerifyHelper<'a> {
    publics: &'a [Cert],
    valid: bool,
    signer_fpr: Option<String>,
}

impl<'a> VerificationHelper for VerifyHelper<'a> {
    fn get_certs(
        &mut self,
        _ids: &[sequoia_openpgp::KeyHandle],
    ) -> sequoia_openpgp::Result<Vec<Cert>> {
        Ok(self.publics.to_vec())
    }

    fn check(&mut self, structure: MessageStructure) -> sequoia_openpgp::Result<()> {
        for (i, layer) in structure.into_iter().enumerate() {
            if let (0, MessageLayer::SignatureGroup { results }) = (i, layer) {
                for result in results {
                    if let Ok(good) = result {
                        self.valid = true;
                        self.signer_fpr = good
                            .sig
                            .issuer_fingerprints()
                            .next()
                            .map(|fp| fp.to_string());
                    }
                }
            }
        }
        if self.valid {
            Ok(())
        } else {
            Err(anyhow!("no valid signature").into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_encrypt_decrypt_roundtrip() {
        let alice = generate_key("Alice <alice@example.com>").unwrap();
        let bob = generate_key("Bob <bob@example.com>").unwrap();
        let msg = "geheim";
        let ct = encrypt_message(msg, &[&bob.armored_public]).unwrap();
        let pt = decrypt_message(
            &ct,
            &[bob.armored_secret.as_deref().unwrap()],
            &[&alice.armored_public],
        )
        .unwrap();
        assert_eq!(pt.plaintext, msg);
    }

    #[test]
    fn sign_verify_roundtrip() {
        let alice = generate_key("Alice <alice@example.com>").unwrap();
        let signed = sign_message("hello", alice.armored_secret.as_deref().unwrap()).unwrap();
        let verified = verify_message(&signed, &[&alice.armored_public]).unwrap();
        assert!(verified.valid);
        assert_eq!(verified.plaintext, "hello");
    }
}

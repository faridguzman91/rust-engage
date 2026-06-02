//! X3DH (Extended Triple Diffie-Hellman) key agreement.
//! Implements https://signal.org/docs/specifications/x3dh/

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use rand::rngs::OsRng;

const INFO: &[u8] = b"engage-x3dh-v1";
const F: &[u8] = &[0xFFu8; 32]; // 32 0xFF bytes — curve25519 domain separator

/// A remote party's prekey bundle as fetched from the key server.
#[derive(Debug, Clone)]
pub struct RemoteBundle {
    pub identity_key: PublicKey,       // IK_B
    pub signed_prekey: PublicKey,      // SPK_B
    pub spk_signature: [u8; 64],
    pub one_time_prekey: Option<PublicKey>, // OPK_B
}

/// Material produced by the initiator after X3DH.
#[derive(Debug)]
pub struct X3DHOutput {
    /// 32-byte shared secret — feed into the Double Ratchet.
    pub shared_secret: [u8; 32],
    /// Ephemeral public key to send to the recipient so they can derive the same secret.
    pub ephemeral_key: PublicKey,
    /// The one-time prekey ID consumed (if any).
    pub otpk_id: Option<u32>,
}

/// Initiator side: compute X3DH shared secret from our identity key and their bundle.
pub fn initiate(
    our_identity_secret: &StaticSecret,
    our_identity_pub: &PublicKey,
    bundle: &RemoteBundle,
) -> Result<X3DHOutput, String> {
    // Verify the signed prekey signature
    let vk = VerifyingKey::from_bytes(bundle.identity_key.as_bytes())
        .map_err(|e| format!("bad identity key: {e}"))?;
    let sig = ed25519_dalek::Signature::from_bytes(&bundle.spk_signature);
    vk.verify(bundle.signed_prekey.as_bytes(), &sig)
        .map_err(|_| "SPK signature verification failed".to_string())?;

    // Generate ephemeral key pair EK_A
    let ek_secret = StaticSecret::random_from_rng(OsRng);
    let ek_pub = PublicKey::from(&ek_secret);

    // DH1 = DH(IK_A, SPK_B)
    let dh1 = our_identity_secret.diffie_hellman(&bundle.signed_prekey);
    // DH2 = DH(EK_A, IK_B)
    let dh2 = ek_secret.diffie_hellman(&bundle.identity_key);
    // DH3 = DH(EK_A, SPK_B)
    let dh3 = ek_secret.diffie_hellman(&bundle.signed_prekey);

    let mut ikm = Vec::with_capacity(128 + 32 * 4);
    ikm.extend_from_slice(F);
    ikm.extend_from_slice(dh1.as_bytes());
    ikm.extend_from_slice(dh2.as_bytes());
    ikm.extend_from_slice(dh3.as_bytes());

    // DH4 = DH(EK_A, OPK_B) — optional
    let otpk_id = if let Some(otpk) = &bundle.one_time_prekey {
        let dh4 = ek_secret.diffie_hellman(otpk);
        ikm.extend_from_slice(dh4.as_bytes());
        Some(0u32)
    } else {
        None
    };

    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut shared_secret = [0u8; 32];
    hk.expand(INFO, &mut shared_secret)
        .map_err(|e| format!("HKDF expand error: {e}"))?;

    Ok(X3DHOutput {
        shared_secret,
        ephemeral_key: ek_pub,
        otpk_id,
    })
}

/// Recipient side: derive the same shared secret from the initiator's ephemeral key.
pub fn receive(
    our_identity_secret: &StaticSecret,
    our_signed_prekey_secret: &StaticSecret,
    our_one_time_prekey_secret: Option<&StaticSecret>,
    initiator_identity_key: &PublicKey,
    ephemeral_key: &PublicKey,
) -> Result<[u8; 32], String> {
    let dh1 = our_signed_prekey_secret.diffie_hellman(initiator_identity_key);
    let dh2 = our_identity_secret.diffie_hellman(ephemeral_key);
    let dh3 = our_signed_prekey_secret.diffie_hellman(ephemeral_key);

    let mut ikm = Vec::with_capacity(128 + 32 * 4);
    ikm.extend_from_slice(F);
    ikm.extend_from_slice(dh1.as_bytes());
    ikm.extend_from_slice(dh2.as_bytes());
    ikm.extend_from_slice(dh3.as_bytes());

    if let Some(otpk_secret) = our_one_time_prekey_secret {
        let dh4 = otpk_secret.diffie_hellman(ephemeral_key);
        ikm.extend_from_slice(dh4.as_bytes());
    }

    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut shared_secret = [0u8; 32];
    hk.expand(INFO, &mut shared_secret)
        .map_err(|e| format!("HKDF expand error: {e}"))?;

    Ok(shared_secret)
}

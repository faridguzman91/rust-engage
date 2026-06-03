//! @faridguzman91: X3DH (Extended Triple Diffie-Hellman) key agreement.
//! Implements the full Signal X3DH specification:
//! https://signal.org/docs/specifications/x3dh/
//!
//! Initiator (Alice) runs `initiate()` against Bob's prekey bundle.
//! Recipient (Bob) runs `receive()` with Alice's ephemeral key to derive the same secret.
//! Both sides feed the shared secret into the Double Ratchet (see ratchet.rs).

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use rand::rngs::OsRng;

const INFO: &[u8] = b"engage-x3dh-v1";
// @faridguzman91: 32 × 0xFF is the curve25519 domain separator prepended to the IKM
// as specified in the X3DH spec to distinguish the protocol from raw DH usage.
const F: &[u8] = &[0xFFu8; 32];

/// A remote party's prekey bundle as fetched from the key server.
#[derive(Debug, Clone)]
pub struct RemoteBundle {
    pub identity_key: PublicKey,            // IK_B — long-term identity key
    pub signed_prekey: PublicKey,           // SPK_B — medium-term signed prekey
    pub spk_signature: [u8; 64],            // Ed25519 signature of SPK_B by IK_B
    pub one_time_prekey: Option<PublicKey>, // OPK_B — optional one-time prekey
}

/// Material produced by the initiator after X3DH.
#[derive(Debug)]
pub struct X3DHOutput {
    /// 32-byte shared secret — feed into Double Ratchet init_sender().
    pub shared_secret: [u8; 32],
    /// Ephemeral public key (EK_A) to send to the recipient in the first message.
    pub ephemeral_key: PublicKey,
    /// The one-time prekey ID that was consumed (if any).
    pub otpk_id: Option<u32>,
}

/// @faridguzman91: Initiator side — compute X3DH shared secret.
/// Performs 3 (or 4) DH operations, concatenates with the domain separator,
/// and extracts a 32-byte shared secret via HKDF-SHA256.
pub fn initiate(
    our_identity_secret: &StaticSecret,
    our_identity_pub: &PublicKey,
    bundle: &RemoteBundle,
) -> Result<X3DHOutput, String> {
    // Verify the signed prekey signature before using it
    let vk = VerifyingKey::from_bytes(bundle.identity_key.as_bytes())
        .map_err(|e| format!("bad identity key: {e}"))?;
    let sig = ed25519_dalek::Signature::from_bytes(&bundle.spk_signature);
    vk.verify(bundle.signed_prekey.as_bytes(), &sig)
        .map_err(|_| "SPK signature verification failed".to_string())?;

    // Generate ephemeral key pair EK_A
    let ek_secret = StaticSecret::random_from_rng(OsRng);
    let ek_pub = PublicKey::from(&ek_secret);

    // @faridguzman91: Four DH operations as per X3DH spec
    let dh1 = our_identity_secret.diffie_hellman(&bundle.signed_prekey); // DH(IK_A, SPK_B)
    let dh2 = ek_secret.diffie_hellman(&bundle.identity_key);             // DH(EK_A, IK_B)
    let dh3 = ek_secret.diffie_hellman(&bundle.signed_prekey);            // DH(EK_A, SPK_B)

    let mut ikm = Vec::with_capacity(128 + 32 * 4);
    ikm.extend_from_slice(F);
    ikm.extend_from_slice(dh1.as_bytes());
    ikm.extend_from_slice(dh2.as_bytes());
    ikm.extend_from_slice(dh3.as_bytes());

    // DH4 = DH(EK_A, OPK_B) — optional, improves forward secrecy of the first message
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

    Ok(X3DHOutput { shared_secret, ephemeral_key: ek_pub, otpk_id })
}

/// @faridguzman91: Recipient side — derive the same shared secret from EK_A.
/// Mirror of `initiate()` — DH operations are in symmetric order.
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

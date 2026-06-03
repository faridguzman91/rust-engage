//! @faridguzman91: Session manager — ties X3DH and the Double Ratchet together
//! and persists session state per contact in SQLite.
//!
//! One SessionManager is created per request (borrowing the DB connection).
//! State is serialised as JSON and stored in the `sessions` table; loading
//! and saving happens on every encrypt/decrypt call so the ratchet always
//! reflects the latest state even across app restarts.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rusqlite::{params, Connection};
use x25519_dalek::{PublicKey, StaticSecret};

use super::ratchet::{RatchetMessage, RatchetState};
use super::x3dh::{self, RemoteBundle};

pub struct SessionManager<'a> {
    pub db: &'a Connection,
}

impl<'a> SessionManager<'a> {
    pub fn new(db: &'a Connection) -> Self {
        Self { db }
    }

    fn load_state(&self, contact_id: &str) -> Option<RatchetState> {
        self.db
            .query_row(
                "SELECT state_json FROM sessions WHERE contact_id=?1",
                params![contact_id],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
    }

    fn save_state(&self, contact_id: &str, state: &RatchetState) -> Result<(), String> {
        let json = serde_json::to_string(state).map_err(|e| e.to_string())?;
        self.db
            .execute(
                "INSERT OR REPLACE INTO sessions (contact_id, state_json) VALUES (?1, ?2)",
                params![contact_id, json],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// @faridguzman91: Initiator path — run X3DH against the remote bundle, seed the
    /// Double Ratchet, persist state, and return the ephemeral key (EK_A) so the caller
    /// can include it in the first message envelope.
    pub fn init_outbound_session(
        &self,
        contact_id: &str,
        our_ik_secret_bytes: &[u8],
        bundle: &RemoteBundle,
    ) -> Result<PublicKey, String> {
        let mut ik_bytes = [0u8; 32];
        ik_bytes.copy_from_slice(&our_ik_secret_bytes[..32]);
        let our_ik_secret = StaticSecret::from(ik_bytes);
        let our_ik_pub = PublicKey::from(&our_ik_secret);

        let x3dh_out = x3dh::initiate(&our_ik_secret, &our_ik_pub, bundle)?;
        let state = RatchetState::init_sender(&x3dh_out.shared_secret, &bundle.signed_prekey);
        self.save_state(contact_id, &state)?;
        Ok(x3dh_out.ephemeral_key)
    }

    /// @faridguzman91: Recipient path — run X3DH receive with the initiator's EK_A,
    /// seed the Double Ratchet, persist state. Called when a first message arrives
    /// (identified by the presence of ephemeralKey in the WS envelope).
    pub fn init_inbound_session(
        &self,
        contact_id: &str,
        our_ik_secret_bytes: &[u8],
        our_spk_secret_bytes: &[u8],
        initiator_ik_pub_bytes: &[u8],
        ephemeral_key_bytes: &[u8],
    ) -> Result<(), String> {
        let secret_from_bytes = |b: &[u8]| -> StaticSecret {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b[..32]);
            StaticSecret::from(arr)
        };
        let pub_from_bytes = |b: &[u8]| -> PublicKey {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b[..32]);
            PublicKey::from(arr)
        };

        let our_ik = secret_from_bytes(our_ik_secret_bytes);
        let our_spk = secret_from_bytes(our_spk_secret_bytes);
        // @faridguzman91: Fresh ratchet key for this session's initial DH step
        let ratchet_key = StaticSecret::random_from_rng(rand::rngs::OsRng);

        let shared_secret = x3dh::receive(
            &our_ik,
            &our_spk,
            None, // TODO: look up and pass the OPK secret if otpk_id is present
            &pub_from_bytes(initiator_ik_pub_bytes),
            &pub_from_bytes(ephemeral_key_bytes),
        )?;

        let state = RatchetState::init_receiver(&shared_secret, &ratchet_key);
        self.save_state(contact_id, &state)?;
        Ok(())
    }

    pub fn encrypt(&self, contact_id: &str, plaintext: &[u8]) -> Result<String, String> {
        let mut state = self
            .load_state(contact_id)
            .ok_or_else(|| "no session for contact".to_string())?;
        let msg = state.encrypt(plaintext);
        self.save_state(contact_id, &state)?;
        serde_json::to_string(&msg).map_err(|e| e.to_string())
    }

    pub fn decrypt(&self, contact_id: &str, msg_json: &str) -> Result<Vec<u8>, String> {
        let mut state = self
            .load_state(contact_id)
            .ok_or_else(|| "no session for contact".to_string())?;
        let msg: RatchetMessage = serde_json::from_str(msg_json).map_err(|e| e.to_string())?;
        let pt = state.decrypt(&msg)?;
        self.save_state(contact_id, &state)?;
        Ok(pt)
    }
}

//! Double Ratchet algorithm implementation.
//! Implements https://signal.org/docs/specifications/doubleratchet/

use base64::Engine as _;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

const MAX_SKIP: u32 = 1000;
const KDF_RK_INFO: &[u8] = b"engage-ratchet-rk";
const KDF_CK_INFO: &[u8] = b"engage-ratchet-ck";

fn kdf_rk(rk: &[u8; 32], dh_out: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let hk = Hkdf::<Sha256>::new(Some(rk), dh_out);
    let mut out = [0u8; 64];
    hk.expand(KDF_RK_INFO, &mut out).unwrap();
    let (a, b) = out.split_at(32);
    (a.try_into().unwrap(), b.try_into().unwrap())
}

fn kdf_ck(ck: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let hk = Hkdf::<Sha256>::new(Some(ck), &[0x01]);
    let mut new_ck = [0u8; 32];
    hk.expand(KDF_CK_INFO, &mut new_ck).unwrap();

    let hk2 = Hkdf::<Sha256>::new(Some(ck), &[0x02]);
    let mut mk = [0u8; 32];
    hk2.expand(b"engage-ratchet-mk", &mut mk).unwrap();
    (new_ck, mk)
}

fn encrypt_with_key(mk: &[u8; 32], plaintext: &[u8], associated: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(mk));
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut ct = cipher
        .encrypt(nonce, aes_gcm::aead::Payload { msg: plaintext, aad: associated })
        .expect("encryption failed");
    let mut out = nonce_bytes.to_vec();
    out.append(&mut ct);
    out
}

fn decrypt_with_key(mk: &[u8; 32], ciphertext: &[u8], associated: &[u8]) -> Result<Vec<u8>, String> {
    if ciphertext.len() < 12 {
        return Err("ciphertext too short".into());
    }
    let (nonce_bytes, ct) = ciphertext.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(mk));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, aes_gcm::aead::Payload { msg: ct, aad: associated })
        .map_err(|e| format!("decryption failed: {e}"))
}

/// Header prepended to every ratchet message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub dh_pub: Vec<u8>,
    pub pn: u32,
    pub n: u32,
}

/// Serialized ratchet message: header + ciphertext, both base64-encoded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatchetMessage {
    pub header: Header,
    pub ciphertext: String, // base64
}

/// Persistent ratchet session state — serialize and store in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatchetState {
    // DH ratchet sending key pair
    dhs_priv: Vec<u8>,
    dhs_pub: Vec<u8>,
    // DH ratchet remote public key
    dhr: Option<Vec<u8>>,
    // Root key
    rk: [u8; 32],
    // Chain keys
    cks: Option<[u8; 32]>,
    ckr: Option<[u8; 32]>,
    // Message counters
    ns: u32,
    nr: u32,
    pn: u32,
    // Skipped message keys: (dh_pub_b64, n) -> mk
    #[serde(default)]
    skipped: std::collections::HashMap<String, [u8; 32]>,
}

impl RatchetState {
    /// Initiator initialises with the shared secret from X3DH and the recipient's ratchet key.
    pub fn init_sender(shared_secret: &[u8; 32], recipient_ratchet_key: &PublicKey) -> Self {
        let dhs = StaticSecret::random_from_rng(OsRng);
        let dhs_pub = PublicKey::from(&dhs);
        let dh_out: [u8; 32] = dhs.diffie_hellman(recipient_ratchet_key).to_bytes();
        let (rk, cks) = kdf_rk(shared_secret, &dh_out);

        Self {
            dhs_priv: dhs.to_bytes().to_vec(),
            dhs_pub: dhs_pub.as_bytes().to_vec(),
            dhr: Some(recipient_ratchet_key.as_bytes().to_vec()),
            rk,
            cks: Some(cks),
            ckr: None,
            ns: 0,
            nr: 0,
            pn: 0,
            skipped: Default::default(),
        }
    }

    /// Recipient initialises with the shared secret and their own ratchet key pair.
    pub fn init_receiver(shared_secret: &[u8; 32], our_ratchet_secret: &StaticSecret) -> Self {
        let dhs_pub = PublicKey::from(our_ratchet_secret);
        Self {
            dhs_priv: our_ratchet_secret.to_bytes().to_vec(),
            dhs_pub: dhs_pub.as_bytes().to_vec(),
            dhr: None,
            rk: *shared_secret,
            cks: None,
            ckr: None,
            ns: 0,
            nr: 0,
            pn: 0,
            skipped: Default::default(),
        }
    }

    fn dhs_secret(&self) -> StaticSecret {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&self.dhs_priv);
        StaticSecret::from(bytes)
    }

    pub fn encrypt(&mut self, plaintext: &[u8]) -> RatchetMessage {
        let (new_cks, mk) = kdf_ck(self.cks.as_ref().expect("no send chain key"));
        self.cks = Some(new_cks);
        let header = Header {
            dh_pub: self.dhs_pub.clone(),
            pn: self.pn,
            n: self.ns,
        };
        self.ns += 1;
        let assoc = serde_json::to_vec(&header).unwrap();
        let ct = encrypt_with_key(&mk, plaintext, &assoc);
        RatchetMessage {
            header,
            ciphertext: base64::engine::general_purpose::STANDARD.encode(ct),
        }
    }

    pub fn decrypt(&mut self, msg: &RatchetMessage) -> Result<Vec<u8>, String> {
        use base64::Engine;
        let ct = base64::engine::general_purpose::STANDARD
            .decode(&msg.ciphertext)
            .map_err(|e| e.to_string())?;
        let assoc = serde_json::to_vec(&msg.header).unwrap();

        // Check skipped keys first
        let skip_key = format!(
            "{}_{}",
            base64::engine::general_purpose::STANDARD.encode(&msg.header.dh_pub),
            msg.header.n
        );
        if let Some(mk) = self.skipped.remove(&skip_key) {
            return decrypt_with_key(&mk, &ct, &assoc);
        }

        let msg_dh_pub: [u8; 32] = msg.header.dh_pub.clone().try_into()
            .map_err(|_| "invalid DH pub key length")?;
        let msg_dh_key = PublicKey::from(msg_dh_pub);

        // If this is a new ratchet step, advance the DH ratchet
        let need_ratchet = self.dhr.as_deref() != Some(msg.header.dh_pub.as_slice());

        if need_ratchet {
            self.skip_message_keys(msg.header.pn)?;
            self.dh_ratchet(&msg_dh_key);
        }

        self.skip_message_keys(msg.header.n)?;

        let (new_ckr, mk) = kdf_ck(self.ckr.as_ref().expect("no recv chain key"));
        self.ckr = Some(new_ckr);
        self.nr += 1;

        decrypt_with_key(&mk, &ct, &assoc)
    }

    fn skip_message_keys(&mut self, until: u32) -> Result<(), String> {
        if self.nr + MAX_SKIP < until {
            return Err("too many skipped messages".into());
        }
        if let Some(ckr) = self.ckr.as_ref().cloned() {
            let mut ck = ckr;
            while self.nr < until {
                let (new_ck, mk) = kdf_ck(&ck);
                ck = new_ck;
                let key = format!(
                    "{}_{}",
                    base64::engine::general_purpose::STANDARD.encode(
                        self.dhr.as_deref().unwrap_or(&[])
                    ),
                    self.nr
                );
                self.skipped.insert(key, mk);
                self.nr += 1;
            }
            self.ckr = Some(ck);
        }
        Ok(())
    }

    fn dh_ratchet(&mut self, remote_key: &PublicKey) {
        self.pn = self.ns;
        self.ns = 0;
        self.nr = 0;
        self.dhr = Some(remote_key.as_bytes().to_vec());

        let dh_out: [u8; 32] = self.dhs_secret().diffie_hellman(remote_key).to_bytes();
        let (new_rk, ckr) = kdf_rk(&self.rk, &dh_out);
        self.rk = new_rk;
        self.ckr = Some(ckr);

        // Generate new DH send key pair
        let new_dhs = StaticSecret::random_from_rng(OsRng);
        let new_dhs_pub = PublicKey::from(&new_dhs);
        let dh_out2: [u8; 32] = new_dhs.diffie_hellman(remote_key).to_bytes();
        let (new_rk2, cks) = kdf_rk(&self.rk, &dh_out2);
        self.rk = new_rk2;
        self.cks = Some(cks);
        self.dhs_priv = new_dhs.to_bytes().to_vec();
        self.dhs_pub = new_dhs_pub.as_bytes().to_vec();
    }
}

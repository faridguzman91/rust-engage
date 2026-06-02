use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "identityKey")]
    pub identity_key: String,
    #[serde(rename = "signedPreKey")]
    pub signed_prekey: SignedPreKey,
    #[serde(rename = "oneTimePreKeys")]
    pub one_time_prekeys: Vec<OneTimePreKey>,
    #[serde(rename = "registrationId")]
    pub registration_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPreKey {
    #[serde(rename = "keyId")]
    pub key_id: u32,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimePreKey {
    #[serde(rename = "keyId")]
    pub key_id: u32,
    #[serde(rename = "publicKey")]
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreKeyBundle {
    #[serde(rename = "registrationId")]
    pub registration_id: u32,
    #[serde(rename = "identityKey")]
    pub identity_key: String,
    #[serde(rename = "signedPreKey")]
    pub signed_prekey: SignedPreKey,
    #[serde(rename = "oneTimePreKey")]
    pub one_time_prekey: Option<OneTimePreKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    #[serde(rename = "recipientId")]
    pub recipient_id: String,
    #[serde(rename = "senderIk")]
    pub sender_ik: String,
    #[serde(rename = "ephemeralKey")]
    pub ephemeral_key: Option<String>,
    #[serde(rename = "otpkId")]
    pub otpk_id: Option<u32>,
    pub ciphertext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: String,
    #[serde(rename = "senderId")]
    pub sender_id: String,
    #[serde(rename = "senderIk")]
    pub sender_ik: String,
    #[serde(rename = "ephemeralKey")]
    pub ephemeral_key: Option<String>,
    #[serde(rename = "otpkId")]
    pub otpk_id: Option<u32>,
    pub ciphertext: String,
    pub timestamp: i64,
}

/// Envelope pushed over the WebSocket to a connected client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEnvelope {
    Message { payload: StoredMessage },
    Ack { message_id: String },
    Error { code: String, message: String },
}

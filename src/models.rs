use serde::{Deserialize, Serialize};

// @faridguzman91: userId is intentionally absent — the server derives it from the
// JWT claims.sub so the client cannot forge or override their own identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
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

// ── Group models ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    /// Initial member user_ids (besides the creator)
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "identityKey")]
    pub identity_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    #[serde(rename = "createdBy")]
    pub created_by: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub members: Vec<GroupMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMemberRequest {
    #[serde(rename = "userId")]
    pub user_id: String,
}

/// @faridguzman91: Group message request — carries group_id instead of recipient_id.
/// The server fans out to every member except the sender.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendGroupMessageRequest {
    #[serde(rename = "groupId")]
    pub group_id: String,
    #[serde(rename = "senderIk")]
    pub sender_ik: String,
    pub ciphertext: String,
}

// ── WsEnvelope ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupStoredMessage {
    pub id: String,
    #[serde(rename = "groupId")]
    pub group_id: String,
    #[serde(rename = "senderId")]
    pub sender_id: String,
    #[serde(rename = "senderIk")]
    pub sender_ik: String,
    pub ciphertext: String,
    pub timestamp: i64,
}

/// Envelope pushed over the WebSocket to a connected client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEnvelope {
    Message { payload: StoredMessage },
    GroupMessage { payload: GroupStoredMessage },
    /// Forwarded to the original sender when the recipient's client ACKs delivery.
    Ack { message_id: String },
    /// Forwarded to the original sender when the recipient opens and reads the message.
    Read { message_id: String },
    Error { code: String, message: String },
}

/// Frame sent by a connected client over the WebSocket (client → server direction).
#[derive(Debug, Deserialize)]
pub struct ClientFrame {
    #[serde(rename = "type")]
    pub kind: String,
    /// camelCase to match the JS side
    #[serde(rename = "messageId")]
    pub message_id: String,
}

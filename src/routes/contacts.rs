use axum::{extract::{Extension, State}, http::StatusCode, Json};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::state::AppState;

const PEOPLE_API: &str =
    "https://people.googleapis.com/v1/people/me/connections\
     ?personFields=emailAddresses&pageSize=1000";
const GOOGLE_REFRESH_URL: &str = "https://oauth2.googleapis.com/token";

#[derive(Serialize)]
pub struct ContactSuggestion {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "identityKey")]
    pub identity_key: String,
    pub email: String,
}

// ── Google People API response shapes ────────────────────────────────────────

#[derive(Deserialize)]
struct PeopleResponse {
    connections: Option<Vec<Person>>,
}

#[derive(Deserialize)]
struct Person {
    #[serde(rename = "emailAddresses")]
    email_addresses: Option<Vec<EmailAddress>>,
}

#[derive(Deserialize)]
struct EmailAddress {
    value: String,
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    expires_in: Option<u64>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn call_people_api(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<Vec<String>, (StatusCode, String)> {
    let resp = client
        .get(PEOPLE_API)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err((StatusCode::UNAUTHORIZED, String::new()));
    }
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!("People API error {status}: {body}");
        return Err((StatusCode::BAD_GATEWAY, format!("People API {status}: {body}")));
    }

    let body: PeopleResponse = resp.json().await.map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    let emails = body
        .connections
        .unwrap_or_default()
        .into_iter()
        .flat_map(|p| p.email_addresses.unwrap_or_default())
        .map(|e| e.value.to_lowercase())
        .collect();

    Ok(emails)
}

// ── Route handler ─────────────────────────────────────────────────────────────

/// GET /api/contacts/suggest
/// Returns engage users whose Google email is in the caller's Gmail contacts.
pub async fn suggest(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<ContactSuggestion>>, (StatusCode, String)> {
    // Load stored tokens for the requesting user
    let (access_token, refresh_token, token_expires_at): (String, String, i64) = {
        let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        db.query_row(
            "SELECT access_token, refresh_token, token_expires_at FROM oauth_accounts WHERE user_id=?1",
            params![claims.sub],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| (StatusCode::NOT_FOUND, "no OAuth account found".into()))?
    };

    if access_token.is_empty() {
        return Err((StatusCode::FORBIDDEN, "re-authentication required".into()));
    }

    let client = reqwest::Client::new();
    let now = Utc::now().timestamp();

    // Use the access token, refreshing it if expired
    let active_token = if now >= token_expires_at - 60 {
        if refresh_token.is_empty() {
            return Err((StatusCode::FORBIDDEN, "re-authentication required".into()));
        }

        let resp = client
            .post(GOOGLE_REFRESH_URL)
            .form(&[
                ("client_id", state.oauth.client_id.as_str()),
                ("client_secret", state.oauth.client_secret.as_str()),
                ("refresh_token", refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("refresh failed: {e}")))?;

        if !resp.status().is_success() {
            return Err((StatusCode::FORBIDDEN, "re-authentication required".into()));
        }

        let refreshed: RefreshResponse = resp
            .json()
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("refresh decode failed: {e}")))?;

        let new_expires = now + refreshed.expires_in.unwrap_or(3600) as i64;
        {
            let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let _ = db.execute(
                "UPDATE oauth_accounts SET access_token=?1, token_expires_at=?2 WHERE user_id=?3",
                params![refreshed.access_token, new_expires, claims.sub],
            );
        }
        refreshed.access_token
    } else {
        access_token
    };

    // Fetch the caller's Gmail contact email addresses
    let emails = call_people_api(&client, &active_token).await.map_err(|(s, msg)| {
        if s == StatusCode::UNAUTHORIZED {
            (StatusCode::FORBIDDEN, "re-authentication required".into())
        } else {
            (s, msg)
        }
    })?;

    if emails.is_empty() {
        return Ok(Json(vec![]));
    }

    // Build a parameterised IN clause
    let placeholders: String = emails
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 2))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT oa.user_id, d.display_name, d.identity_key, oa.email
         FROM oauth_accounts oa
         JOIN devices d ON d.user_id = oa.user_id
         WHERE lower(oa.email) IN ({placeholders})
           AND oa.user_id != ?1
           AND d.identity_key != ''"
    );

    let suggestions: Vec<ContactSuggestion> = {
        let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let mut stmt = db.prepare(&sql).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        param_values.push(Box::new(claims.sub.clone()));
        for email in &emails {
            param_values.push(Box::new(email.clone()));
        }
        let params_refs: Vec<&dyn rusqlite::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

        let rows: Vec<ContactSuggestion> = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(ContactSuggestion {
                    user_id: row.get(0)?,
                    display_name: row.get(1)?,
                    identity_key: row.get(2)?,
                    email: row.get(3)?,
                })
            })
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    Ok(Json(suggestions))
}

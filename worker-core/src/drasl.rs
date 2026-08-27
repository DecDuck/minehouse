//! An [`azalea`] account that authenticates against a Drasl / authlib-injector
//! Yggdrasil server instead of Mojang.
//!
//! authlib-injector servers expose the classic Yggdrasil endpoints under a
//! single API root, e.g. for Drasl that is typically
//! `https://drasl.example.com/authlib-injector`. Given that root the endpoints
//! used here are:
//! - `{root}/authserver/authenticate`
//! - `{root}/authserver/refresh`
//! - `{root}/sessionserver/session/minecraft/join`

use std::{future::Future, pin::Pin, sync::Mutex};

use anyhow::Context;
use azalea::{
    account::{Account, AccountTrait},
    auth::{AuthError, sessionserver::ClientSessionServerError},
};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use tracing::warn;
use uuid::Uuid;

/// Matches the (crate-private) `BoxFuture` alias used by [`AccountTrait`].
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// An account authenticated against a Drasl / authlib-injector server.
#[derive(Debug)]
pub struct DraslAccount {
    /// authlib-injector API root, without a trailing slash.
    api_root: String,
    username: String,
    uuid: Uuid,
    access_token: Mutex<String>,
    client_token: Mutex<String>,
    client: reqwest::Client,
}

impl DraslAccount {
    /// Authenticate against the given authlib-injector API root and return an
    /// [`Account`] ready to join online-mode servers backed by that Yggdrasil
    /// server.
    ///
    /// `api_root` is the authlib-injector API root (e.g.
    /// `https://drasl.example.com/authlib-injector`). `username` is the account
    /// username or email as expected by the server.
    pub async fn authenticate(
        api_root: &str,
        username: &str,
        password: &str,
    ) -> anyhow::Result<Account> {
        let api_root = api_root.trim_end_matches('/').to_owned();
        let client = reqwest::Client::new();

        let res = client
            .post(format!("{api_root}/authserver/authenticate"))
            .json(&json!({
                "agent": { "name": "Minecraft", "version": 1 },
                "username": username,
                "password": password,
                "requestUser": false,
            }))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("Drasl authentication failed ({status}): {body}");
        }

        let body: AuthenticateResponse = res.json().await?;
        let profile = body
            .selected_profile
            .context("Drasl server returned no selectedProfile")?;
        let uuid = Uuid::parse_str(&profile.id)
            .with_context(|| format!("invalid profile UUID from Drasl server: {}", profile.id))?;

        Ok(Self {
            api_root,
            username: profile.name,
            uuid,
            access_token: Mutex::new(body.access_token),
            client_token: Mutex::new(body.client_token),
            client,
        }
        .into())
    }

    /// Refresh the access token via the Yggdrasil `/authserver/refresh`
    /// endpoint, updating the stored access and client tokens on success.
    async fn try_refresh(&self) -> anyhow::Result<()> {
        let (access_token, client_token) = {
            let access = self
                .access_token
                .lock()
                .expect("access_token poisoned")
                .clone();
            let client = self
                .client_token
                .lock()
                .expect("client_token poisoned")
                .clone();
            (access, client)
        };

        let res = self
            .client
            .post(format!("{}/authserver/refresh", self.api_root))
            .json(&json!({
                "accessToken": access_token,
                "clientToken": client_token,
                "requestUser": false,
            }))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("Drasl token refresh failed ({status}): {body}");
        }

        let body: RefreshResponse = res.json().await?;
        *self.access_token.lock().expect("access_token poisoned") = body.access_token;
        *self.client_token.lock().expect("client_token poisoned") = body.client_token;
        Ok(())
    }
}

impl AccountTrait for DraslAccount {
    fn username(&self) -> &str {
        &self.username
    }

    fn uuid(&self) -> Uuid {
        self.uuid
    }

    fn access_token(&self) -> Option<String> {
        Some(
            self.access_token
                .lock()
                .expect("access_token poisoned")
                .clone(),
        )
    }

    fn refresh(&self) -> BoxFuture<'_, Result<(), AuthError>> {
        // Yggdrasil access tokens are long-lived; refresh is best-effort so a
        // transient failure doesn't tear the client down. A stale token will
        // instead surface as an `InvalidSession` from `join`.
        Box::pin(async move {
            if let Err(err) = self.try_refresh().await {
                warn!("failed to refresh Drasl access token: {err:#}");
            }
            Ok(())
        })
    }

    fn join<'a>(
        &'a self,
        public_key: &'a [u8],
        private_key: &'a [u8; 16],
        server_id: &'a str,
        proxy: Option<reqwest::Proxy>,
    ) -> BoxFuture<'a, Result<(), ClientSessionServerError>> {
        Box::pin(async move {
            let server_hash = azalea_crypto::hex_digest(&azalea_crypto::digest_data(
                server_id.as_bytes(),
                public_key,
                private_key,
            ));

            let client = match proxy {
                Some(proxy) => reqwest::ClientBuilder::new().proxy(proxy).build()?,
                None => self.client.clone(),
            };

            let mut encode_buffer = Uuid::encode_buffer();
            let undashed_uuid = self.uuid.simple().encode_lower(&mut encode_buffer);
            let access_token = self
                .access_token
                .lock()
                .expect("access_token poisoned")
                .clone();

            let res = client
                .post(format!(
                    "{}/sessionserver/session/minecraft/join",
                    self.api_root
                ))
                .json(&json!({
                    "accessToken": access_token,
                    "selectedProfile": undashed_uuid,
                    "serverId": server_hash,
                }))
                .send()
                .await?;

            match res.status() {
                StatusCode::NO_CONTENT | StatusCode::OK => Ok(()),
                StatusCode::FORBIDDEN => Err(ClientSessionServerError::ForbiddenOperation),
                StatusCode::TOO_MANY_REQUESTS => Err(ClientSessionServerError::RateLimited),
                status_code => {
                    let body = res.text().await.unwrap_or_default();
                    Err(ClientSessionServerError::UnexpectedResponse {
                        status_code: status_code.as_u16(),
                        body,
                    })
                }
            }
        })
    }
}

#[derive(Deserialize)]
struct AuthenticateResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "clientToken")]
    client_token: String,
    #[serde(rename = "selectedProfile")]
    selected_profile: Option<Profile>,
}

#[derive(Deserialize)]
struct RefreshResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "clientToken")]
    client_token: String,
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}
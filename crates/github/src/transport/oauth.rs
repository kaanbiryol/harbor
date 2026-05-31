use http::header;
use octocrab::{Octocrab, auth::DeviceCodes};
use secrecy::{ExposeSecret, SecretString};

use crate::{GitHubError, Result};

use super::map_octocrab_error;

#[derive(Clone)]
pub struct GitHubDeviceFlow {
    client_id: String,
    codes: DeviceCodes,
}

impl GitHubDeviceFlow {
    pub fn user_code(&self) -> &str {
        &self.codes.user_code
    }

    pub fn verification_uri(&self) -> &str {
        &self.codes.verification_uri
    }

    pub fn expires_in(&self) -> u64 {
        self.codes.expires_in
    }

    pub fn interval(&self) -> u64 {
        self.codes.interval
    }

    pub async fn poll_for_token(self) -> Result<String> {
        smol::unblock(move || {
            let runtime = oauth_runtime()?;
            runtime.block_on(async move {
                let client_id = SecretString::from(self.client_id);
                let crab = oauth_device_crab()?;
                let auth = self
                    .codes
                    .poll_until_available(&crab, &client_id)
                    .await
                    .map_err(map_octocrab_error)?;

                Ok(auth.access_token.expose_secret().to_string())
            })
        })
        .await
    }
}

pub async fn start_oauth_device_flow(client_id: impl Into<String>) -> Result<GitHubDeviceFlow> {
    let client_id = client_id.into();
    smol::unblock(move || {
        let runtime = oauth_runtime()?;
        runtime.block_on(async move {
            let secret_client_id = SecretString::from(client_id.clone());
            let crab = oauth_device_crab()?;
            let codes = crab
                .authenticate_as_device(&secret_client_id, ["repo", "read:org"])
                .await
                .map_err(map_octocrab_error)?;

            Ok(GitHubDeviceFlow { client_id, codes })
        })
    })
    .await
}

fn oauth_runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("harbor-github-oauth")
        .worker_threads(1)
        .build()
        .map_err(|error| GitHubError::Transport(error.to_string()))
}

fn oauth_device_crab() -> Result<Octocrab> {
    Octocrab::builder()
        .base_uri("https://github.com")
        .map_err(map_octocrab_error)?
        .add_header(header::ACCEPT, "application/json".to_string())
        .build()
        .map_err(map_octocrab_error)
}

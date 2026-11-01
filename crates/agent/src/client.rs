//! One client for every provider kind.
//!
//! `ProviderClient` checks the base URL against the destination policy when it
//! is built, builds an HTTP client whose resolver enforces the same policy at
//! connect time, renders a provider-neutral request for the profile's wire
//! format and runs exactly one turn. It never retries: whether a failed turn
//! may be repeated is the host's decision, from [`crate::errors`].

use tokio::sync::watch;

use crate::anthropic::{self, AnthropicClient};
use crate::destination::{DestinationPolicy, DestinationRefusal, outbound_client};
use crate::openai::{self, ApiKey, Control, OpenAiClient};
use crate::profile::{Capabilities, ModelProfile, ProviderKind};
use crate::request::ModelRequest;
use crate::transport::{DEFAULT_MAX_RESPONSE_BYTES, Tee};
use crate::types::{Partial, Turn, TurnError};

enum Inner {
    Anthropic(AnthropicClient),
    OpenAi(OpenAiClient),
}

pub struct ProviderClient {
    profile: ModelProfile,
    caps: Capabilities,
    inner: Inner,
}

impl std::fmt::Debug for ProviderClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderClient")
            .field("profile", &self.profile.id)
            .finish_non_exhaustive()
    }
}

/// Resolves when the flag turns true. A dropped sender never cancels.
async fn cancelled(mut rx: watch::Receiver<bool>) {
    loop {
        if *rx.borrow_and_update() {
            return;
        }
        if rx.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

impl ProviderClient {
    pub fn new(
        profile: ModelProfile,
        key: ApiKey,
        policy: &DestinationPolicy,
    ) -> Result<Self, DestinationRefusal> {
        Self::with_max_response_bytes(profile, key, policy, DEFAULT_MAX_RESPONSE_BYTES)
    }

    pub fn with_max_response_bytes(
        profile: ModelProfile,
        key: ApiKey,
        policy: &DestinationPolicy,
        max_response_bytes: u64,
    ) -> Result<Self, DestinationRefusal> {
        policy.check_url(&profile.base_url)?;
        let http = outbound_client(
            policy,
            std::time::Duration::from_secs(u64::from(profile.timeout_secs.max(1))),
        )
        .map_err(|e| {
            DestinationRefusal::new(
                "invalid_url",
                format!("HTTP client could not be built: {e}"),
            )
        })?;
        let inner = match profile.provider {
            ProviderKind::Anthropic => Inner::Anthropic(AnthropicClient::with_client(
                http,
                &profile.base_url,
                key,
                max_response_bytes,
            )),
            ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => Inner::OpenAi(
                OpenAiClient::with_client(http, &profile.base_url, key, max_response_bytes),
            ),
        };
        let caps = profile.starting_capabilities();
        Ok(Self {
            profile,
            caps,
            inner,
        })
    }

    pub fn profile(&self) -> &ModelProfile {
        &self.profile
    }

    pub fn capabilities(&self) -> &Capabilities {
        &self.caps
    }

    /// Use capabilities from a probe when rendering requests.
    pub fn set_capabilities(&mut self, caps: Capabilities) {
        self.caps = caps;
    }

    pub fn render(&self, req: &ModelRequest) -> serde_json::Value {
        match self.profile.provider {
            ProviderKind::Anthropic => anthropic::render_with(req, &self.profile, &self.caps),
            _ => openai::render_with(req, &self.profile, &self.caps),
        }
    }

    /// List model IDs without inference.
    pub async fn list_models(&self) -> Result<(u16, Vec<String>), TurnError> {
        match &self.inner {
            Inner::Anthropic(c) => c.list_models().await,
            Inner::OpenAi(c) => c.list_models().await,
        }
    }

    /// Run one turn. `cancel` turning true closes the connection and returns
    /// `Cancelled`. `tee` receives the response bytes unchanged.
    pub async fn run_turn(
        &self,
        req: &ModelRequest,
        cancel: watch::Receiver<bool>,
        tee: Option<Tee>,
    ) -> Result<Turn, TurnError> {
        self.run_turn_with(req, cancel, tee, || {}).await
    }

    /// Like [`ProviderClient::run_turn`], calling `on_accepted` once the
    /// provider answered 2xx and before the stream is read.
    pub async fn run_turn_with(
        &self,
        req: &ModelRequest,
        cancel: watch::Receiver<bool>,
        tee: Option<Tee>,
        on_accepted: impl FnOnce(),
    ) -> Result<Turn, TurnError> {
        if *cancel.borrow() {
            return Err(TurnError::Cancelled {
                partial: Partial::default(),
            });
        }
        let body = self.render(req);
        let started = async {
            match &self.inner {
                Inner::Anthropic(c) => c.start(&body, tee).await.map(|(s, _)| s),
                Inner::OpenAi(c) => c.start(&body, tee).await.map(|(s, _)| s),
            }
        };
        let src = tokio::select! {
            r = started => r?,
            _ = cancelled(cancel.clone()) => return Err(TurnError::Cancelled { partial: Partial::default() }),
        };
        on_accepted();
        let flag = cancel.clone();
        let observe = move |_: &Partial| {
            if *flag.borrow() {
                Control::Cancel
            } else {
                Control::Continue
            }
        };
        let assembling = async {
            match &self.inner {
                Inner::Anthropic(_) => anthropic::assemble(src, observe).await,
                Inner::OpenAi(_) => openai::assemble(src, observe).await,
            }
        };
        tokio::select! {
            r = assembling => r,
            _ = cancelled(cancel) => Err(TurnError::Cancelled { partial: Partial::default() }),
        }
    }
}

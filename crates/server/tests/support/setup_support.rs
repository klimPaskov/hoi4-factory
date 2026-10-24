// Support for the Setup gate tests (`setup_gates.rs`), included after the shared harness.
//
// * `PinnedSource`: Setup's real `HttpSourceClient` (the public repository, files verified against
//   the manifest's size and SHA-256 by Setup itself), with "latest" fixed to one commit so every
//   run plans against the same package. The cache is shared between runs.
// * `RecordingModel`: the server's own Anthropic client, tee-ing every real reply to a file.
// * `ReplayModel`: replays a reply recorded by `RecordingModel`. It is labelled recorded-real
//   replay: the reply is byte-identical to what the provider returned except `input_sha256`, which
//   Setup's validator requires to equal the digest of the current request (that digest covers the
//   scan id and root path, which differ on every run). Nothing is invented.

use factory_setup_adapter::setup::{AppError, HttpSourceClient, SourceFetcher, SourceRequest};
use factory_setup_adapter::{ModelClient, ModelError, ModelReply, ModelRequest};

/// Head of klimPaskov/Agentic-HOI4-Modding when these tests were written (a commit on `main`).
pub const PACKAGE_REVISION: &str = "cbefeb75cb469ec8b851baa542de53c47dcd754e";

fn setup_cache_dir() -> PathBuf {
    let dir = std::env::var_os("FACTORY_TEST_SETUP_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("hf-setup-test-cache"));
    std::fs::create_dir_all(&dir).expect("create the Setup package cache");
    std::fs::canonicalize(&dir).expect("resolve the Setup package cache")
}

pub struct PinnedSource {
    inner: factory_server::startup::DetachedDrop<HttpSourceClient>,
    manifest_cache: PathBuf,
}

impl PinnedSource {
    /// Built on the blocking pool: Setup's client must not be created on the async runtime.
    pub async fn new() -> Arc<Self> {
        tokio::task::spawn_blocking(|| {
            let cache = setup_cache_dir();
            Arc::new(Self {
                inner: factory_server::startup::DetachedDrop::new(
                    HttpSourceClient::for_cache_root(cache.clone()).expect("source client"),
                ),
                manifest_cache: cache,
            })
        })
        .await
        .unwrap()
    }
}

impl SourceFetcher for PinnedSource {
    fn resolve_commit(
        &self,
        request: &SourceRequest,
    ) -> Result<(String, Option<String>, Option<String>), AppError> {
        use hoi4_mod_setup::models::SourceMode;
        match request.mode {
            SourceMode::Latest => Ok((PACKAGE_REVISION.to_owned(), Some("main".to_owned()), None)),
            SourceMode::PinnedCommit
                if request.requested_ref.as_deref() == Some(PACKAGE_REVISION) =>
            {
                Ok((PACKAGE_REVISION.to_owned(), None, None))
            }
            _ => Err(AppError::Source(
                "the test source serves only its pinned commit".into(),
            )),
        }
    }

    fn fetch_manifest(&self, revision: &str) -> Result<Vec<u8>, AppError> {
        let file = self.manifest_cache.join(format!("manifest-{revision}.json"));
        if let Ok(bytes) = std::fs::read(&file) {
            return Ok(bytes);
        }
        let bytes = self.inner.fetch_manifest(revision)?;
        let _ = std::fs::write(&file, &bytes);
        Ok(bytes)
    }

    fn fetch_verified_file(
        &self,
        revision: &str,
        path: &str,
        expected_sha256: &str,
        expected_size: Option<u64>,
    ) -> Result<Vec<u8>, AppError> {
        self.inner
            .fetch_verified_file(revision, path, expected_sha256, expected_size)
    }

    fn prefetch_verified_files(&self, revision: &str, files: &[(String, String, Option<u64>)]) {
        self.inner.prefetch_verified_files(revision, files)
    }
}

// ----- model clients -------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordedCall {
    pub response_id: Option<String>,
    pub usage: Option<Value>,
    /// SHA-256 of the prompt text the provider received (the prompt itself is not stored).
    pub prompt_sha256: String,
    pub reply: Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Recording {
    pub label: String,
    pub provider: String,
    pub model: String,
    pub recorded_at: String,
    pub note: String,
    pub calls: Vec<RecordedCall>,
}

pub fn recording_path(label: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/recorded")
        .join(format!("{label}.json"))
}

/// The server's real Anthropic client, tee-ing replies. Used only by `#[ignore]`d live tests.
pub struct RecordingModel {
    inner: factory_server::model::AnthropicModel,
    model: String,
    calls: Mutex<Vec<RecordedCall>>,
}

impl RecordingModel {
    pub fn new(key: &str, model: &str) -> Arc<Self> {
        let settings = factory_server::config::AnthropicSettings {
            api_key: factory_accounts::Redacted(key.to_owned()),
            model: model.to_owned(),
            base_url: "https://api.anthropic.com".to_owned(),
        };
        Arc::new(Self {
            inner: factory_server::model::AnthropicModel::new(&settings),
            model: model.to_owned(),
            calls: Mutex::new(Vec::new()),
        })
    }

    /// Token usage so far, summed, and the estimated cost in USD at the recorded price.
    pub fn spend(&self) -> (u64, u64, f64) {
        let calls = self.calls.lock().unwrap();
        let (mut input, mut output) = (0u64, 0u64);
        for call in calls.iter() {
            if let Some(usage) = &call.usage {
                input += usage["input_tokens"].as_u64().unwrap_or(0);
                output += usage["output_tokens"].as_u64().unwrap_or(0);
            }
        }
        let usd = factory_agent::cost::price_for(&self.model)
            .map(|p| (input as f64 * p.input_per_mtok + output as f64 * p.output_per_mtok) / 1e6)
            .unwrap_or(f64::NAN);
        (input, output, usd)
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    /// Writes the recorded replies for the replay tests when `FACTORY_RECORD_REPLIES` is set.
    pub fn save(&self, label: &str, note: &str) {
        let (input, output, usd) = self.spend();
        println!(
            "LIVE SPEND [{label}]: calls={} input_tokens={input} output_tokens={output} estimated_usd={usd:.6}",
            self.call_count()
        );
        if std::env::var_os("FACTORY_RECORD_REPLIES").is_none() {
            return;
        }
        let recording = Recording {
            label: label.to_owned(),
            provider: "anthropic".to_owned(),
            model: self.model.clone(),
            recorded_at: chrono::Utc::now().to_rfc3339(),
            note: note.to_owned(),
            calls: self.calls.lock().unwrap().clone(),
        };
        let path = recording_path(label);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_vec_pretty(&recording).unwrap()).unwrap();
        println!("recorded {} calls to {}", recording.calls.len(), path.display());
    }
}

#[async_trait]
impl ModelClient for RecordingModel {
    async fn complete(&self, request: ModelRequest) -> Result<ModelReply, ModelError> {
        let prompt_sha256 = sha256_hex(request.prompt.as_bytes());
        let reply = self.inner.complete(request).await?;
        self.calls.lock().unwrap().push(RecordedCall {
            response_id: reply.response_id.clone(),
            usage: reply.usage.clone(),
            prompt_sha256,
            reply: reply.reply.clone(),
        });
        Ok(reply)
    }
}

/// Replays a recording (see the header of this file).
pub struct ReplayModel {
    recording: Recording,
    next: Mutex<usize>,
}

impl ReplayModel {
    pub fn load(label: &str) -> Arc<Self> {
        let path = recording_path(label);
        let bytes = std::fs::read(&path).unwrap_or_else(|_| {
            panic!(
                "no recorded reply {} - run the live tests with FACTORY_RECORD_REPLIES=1 to record it",
                path.display()
            )
        });
        Arc::new(Self {
            recording: serde_json::from_slice(&bytes).expect("a recording"),
            next: Mutex::new(0),
        })
    }

    pub fn calls_made(&self) -> usize {
        *self.next.lock().unwrap()
    }
}

#[async_trait]
impl ModelClient for ReplayModel {
    async fn complete(&self, request: ModelRequest) -> Result<ModelReply, ModelError> {
        let index = {
            let mut next = self.next.lock().unwrap();
            let index = *next;
            *next += 1;
            index
        };
        let call = self
            .recording
            .calls
            .get(index)
            .ok_or_else(|| ModelError::Failed("the recording has no further reply".into()))?;
        let mut reply = call.reply.clone();
        reply["input_sha256"] = Value::String(request.input_sha256.clone());
        Ok(ModelReply {
            reply,
            response_id: call.response_id.clone(),
            usage: call.usage.clone(),
        })
    }
}

/// The Anthropic key for live tests: the Windows user environment, else the process environment.
/// Never printed.
pub fn live_key() -> String {
    let from_user_env = if cfg!(windows) {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[Environment]::GetEnvironmentVariable('ANTHROPIC_API_KEY','User')",
            ])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    } else {
        None
    };
    from_user_env
        .filter(|k| !k.is_empty())
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty()))
        .expect("ANTHROPIC_API_KEY is not set for this user")
}

pub const LIVE_MODEL: &str = "claude-haiku-5-5";

// ----- driving the flow over HTTP --------------------------------------------------------------

pub struct Flow<'a> {
    pub client: &'a Client,
    pub project: String,
}

impl Flow<'_> {
    pub fn base(&self) -> String {
        format!("/api/v1/projects/{}/setup", self.project)
    }

    pub async fn post(&self, tail: &str, body: Value) -> Resp {
        self.client.post(&format!("{}/{tail}", self.base()), body).await
    }

    pub async fn get(&self, tail: &str) -> Resp {
        self.client.get(&format!("{}/{tail}", self.base())).await
    }

    /// Scan, approve every offered excerpt (the person's decision in a browser), analyze.
    /// Returns the analysis response.
    pub async fn scan_and_analyze(&self, mode: &str, description: Option<&str>) -> Resp {
        let mut body = json!({ "mode": mode });
        if let Some(d) = description {
            body["description"] = json!(d);
        }
        let scan = self.post("scan", body).await;
        assert_eq!(scan.status, 200, "scan: {}", scan.text);
        if scan.body["evidence"]["required"] == true {
            let references: Vec<Value> = scan.body["evidence"]["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["reference"].clone())
                .collect();
            let approved = self
                .post(
                    "evidence",
                    json!({"scan_id": scan.body["scan_id"], "references": references}),
                )
                .await;
            assert_eq!(approved.status, 200, "evidence: {}", approved.text);
        }
        self.post("analyze", json!({})).await
    }

    /// Confirms the values as proposed, builds the plan. Returns the plan response.
    pub async fn confirm_and_plan(&self, analysis: &Resp) -> Resp {
        let confirm = self
            .post(
                "confirm",
                json!({
                    "analysis_id": analysis.body["analysis_id"],
                    "values": analysis.body["values"],
                }),
            )
            .await;
        assert_eq!(confirm.status, 200, "confirm: {}", confirm.text);
        self.post("plan", json!({})).await
    }

    /// Approves exactly the plan shown and applies it.
    pub async fn approve_and_apply(&self, plan: &Resp) -> Resp {
        let plan = &plan.body["plan"];
        let approved = self
            .post(
                "approve",
                json!({"plan_id": plan["plan_id"], "plan_sha256": plan["plan_sha256"]}),
            )
            .await;
        assert_eq!(approved.status, 200, "approve: {}", approved.text);
        self.post("apply", json!({"plan_id": plan["plan_id"]})).await
    }
}

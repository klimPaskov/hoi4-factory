//! C-06: the pure builder (no database) and the immutable store (PostgreSQL).

mod common;

use common::*;
use factory_tasks::context::*;
use factory_tasks::*;
use serde_json::json;

fn summary(n: u8, status: ReceiptStatus) -> ReceiptSummary {
    ReceiptSummary {
        operation_id: OperationId(uuid::Uuid::from_bytes([n; 16])),
        tool: "write_file".into(),
        status,
        args_sha256: hash(n),
        before_sha256: Some(hash(1)),
        after_sha256: None,
        partial: false,
        summary: format!("write_file: {status}"),
    }
}

fn task_for_scope() -> Task {
    // Only the scope fields matter to the builder.
    let now = chrono::Utc::now();
    Task {
        id: TaskId::new(),
        project_id: factory_accounts::ProjectId::new(),
        owner_account_id: factory_accounts::AccountId::new(),
        created_by_account_id: factory_accounts::AccountId::new(),
        objective: "Add one focus".into(),
        requirement_ids: vec!["REQ-014".into(), "REQ-001".into()],
        mode: TaskMode::Edit,
        allowed_paths: vec!["common/b/".into(), "common/a/".into()],
        state: TaskState::Ready,
        paused: false,
        pause_reason: None,
        budget_micros: 1,
        max_turns: 1,
        model_choice: json!({}),
        version: 1,
        last_seq: 1,
        created_at: now,
        updated_at: now,
    }
}

fn big(path: &str, priority: u32, kb: usize) -> SourceLocator {
    let mut l = locator(path, &"x".repeat(kb * 1024), false);
    l.priority = priority;
    l
}

#[test]
fn identical_inputs_give_identical_manifest_hashes_whatever_the_order() {
    let task = task_for_scope();
    let mut a = inputs_for(&task);
    a.source_locators = vec![
        locator("common/b.txt", "bbb", true),
        locator("common/a.txt", "aaa", true),
        big("opt/1.txt", 1, 1),
        big("opt/2.txt", 2, 1),
    ];
    a.completed_receipts = vec![
        summary(2, ReceiptStatus::Succeeded),
        summary(1, ReceiptStatus::Succeeded),
    ];
    let mut b = a.clone();
    b.source_locators.reverse();
    b.completed_receipts.reverse();
    let one = build_context(a.clone()).unwrap();
    let two = build_context(b).unwrap();
    assert_ne!(one.id, two.id, "each build is a new bundle");
    assert_eq!(one.manifest_sha256, two.manifest_sha256);
    assert_eq!(one.manifest, two.manifest);
    assert_eq!(one.content, two.content);
    one.verify().unwrap();
    // The hash moves with the inputs: a changed source hash is a changed bundle.
    let mut c = a;
    c.source_locators[0] = locator("common/b.txt", "bbc", true);
    assert_ne!(
        build_context(c).unwrap().manifest_sha256,
        one.manifest_sha256
    );
    // The estimate is labelled as one.
    assert!(one.manifest.token_estimate.is_estimate);
    assert_eq!(one.manifest.token_estimate.method, TOKEN_ESTIMATE_METHOD);
}

#[test]
fn scope_is_canonically_ordered_and_always_present() {
    let bundle = build_context(inputs_for(&task_for_scope())).unwrap();
    assert_eq!(
        bundle.content.scope.allowed_paths,
        ["common/a/", "common/b/"]
    );
    assert_eq!(bundle.content.scope.requirement_ids, ["REQ-001", "REQ-014"]);
    assert_eq!(
        bundle.content.safety_constraints.len(),
        SAFETY_CONSTRAINTS.len()
    );
    let kinds: Vec<_> = bundle.manifest.included.iter().map(|i| i.kind).collect();
    assert!(kinds.contains(&ItemKind::Scope));
    assert!(kinds.contains(&ItemKind::SafetyConstraints));
    assert_eq!(bundle.manifest.source_versions.len(), 1);
    assert_eq!(
        bundle.manifest.source_versions[0].path,
        "common/national_focus/a.txt"
    );
}

#[test]
fn required_context_too_large_is_explicit() {
    let task = task_for_scope();
    let mut inputs = inputs_for(&task);
    inputs.source_locators = vec![{
        let mut l = locator("common/huge.txt", &"y".repeat(64 * 1024), true);
        l.priority = 0;
        l
    }];
    inputs.budget = TokenBudget {
        context_tokens: 10_000,
        reserved_output: 2_000,
        reserved_tools: 1_000,
    };
    match build_context(inputs.clone()) {
        Err(ContextError::RequiredContextTooLarge {
            required_tokens,
            available_tokens,
        }) => {
            assert_eq!(available_tokens, 7_000);
            assert!(required_tokens > 16_000, "{required_tokens}");
        }
        other => panic!("{other:?}"),
    }
    // Reservations larger than the window leave nothing, and that is the same error.
    inputs.source_locators.clear();
    inputs.budget = TokenBudget {
        context_tokens: 100,
        reserved_output: 90,
        reserved_tools: 90,
    };
    assert!(matches!(
        build_context(inputs),
        Err(ContextError::RequiredContextTooLarge {
            available_tokens: 0,
            ..
        })
    ));
}

#[test]
fn unresolved_effects_are_never_truncated() {
    let task = task_for_scope();
    let mut inputs = inputs_for(&task);
    inputs.unresolved_effects = vec![
        summary(7, ReceiptStatus::UnknownEffect),
        summary(8, ReceiptStatus::Started),
    ];
    inputs.source_locators = vec![big("opt/1.txt", 1, 40)];
    inputs.recent_messages = (0..30)
        .map(|i| VisibleMessage {
            role: MessageRole::User,
            text: format!("message {i} {}", "z".repeat(300)),
        })
        .collect();
    // Just enough for the mandatory part plus a little.
    let mandatory = build_context({
        let mut m = inputs.clone();
        m.source_locators.clear();
        m.recent_messages.clear();
        m
    })
    .unwrap()
    .manifest
    .budget
    .mandatory_tokens;
    inputs.budget = TokenBudget {
        context_tokens: mandatory + 200,
        reserved_output: 0,
        reserved_tools: 0,
    };
    let bundle = build_context(inputs).unwrap();
    assert_eq!(bundle.content.unresolved_effects.len(), 2);
    assert!(
        bundle
            .manifest
            .included
            .iter()
            .filter(|i| i.kind == ItemKind::UnresolvedEffect)
            .all(|i| i.mandatory)
    );
    assert!(
        bundle.manifest.budget.mandatory_tokens + bundle.manifest.budget.optional_tokens
            <= mandatory + 200
    );
}

#[test]
fn optional_content_is_dropped_lowest_priority_first_and_every_omission_is_recorded() {
    let task = task_for_scope();
    let mut inputs = inputs_for(&task);
    inputs.source_locators = vec![
        locator("req/a.txt", "required", true),
        big("opt/high.txt", 1, 6),
        big("opt/low.txt", 9, 6),
    ];
    inputs.approved_references = vec![Reference {
        id: "RULE-1".into(),
        kind: ReferenceKind::Rule,
        version: "3".into(),
        owner: "maintainer".into(),
        approval_revision: "r7".into(),
        text: "Use the project's tag prefix.".into(),
    }];
    inputs.recent_messages = (0..6)
        .map(|i| VisibleMessage {
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            text: format!("m{i} {}", "q".repeat(2000)),
        })
        .collect();
    inputs.completed_receipts = vec![summary(3, ReceiptStatus::Succeeded)];
    let base = build_context({
        let mut m = inputs.clone();
        m.source_locators.truncate(1);
        m.approved_references.clear();
        m.recent_messages.clear();
        m.completed_receipts.clear();
        m
    })
    .unwrap()
    .manifest
    .budget
    .mandatory_tokens;
    // Room for the reference, one big source and about two messages; not for the rest.
    inputs.budget = TokenBudget {
        context_tokens: base + 2_900,
        reserved_output: 0,
        reserved_tools: 0,
    };
    let bundle = build_context(inputs).unwrap();
    let paths: Vec<&str> = bundle
        .content
        .sources
        .iter()
        .map(|s| s.path.as_str())
        .collect();
    assert!(paths.contains(&"opt/high.txt"), "{paths:?}");
    let low = bundle
        .content
        .sources
        .iter()
        .find(|s| s.path == "opt/low.txt");
    assert!(
        low.is_none_or(|l| l.content.is_none()),
        "low priority loses its text first"
    );
    assert_eq!(
        bundle.content.approved_references.len(),
        1,
        "references outrank sources"
    );
    // Newest messages are kept, oldest dropped.
    let kept: Vec<&str> = bundle
        .content
        .recent_messages
        .iter()
        .map(|m| &m.text[..2])
        .collect();
    assert!(!kept.is_empty() && kept.len() < 6, "{kept:?}");
    assert_eq!(*kept.last().unwrap(), "m5");
    assert!(!kept.contains(&"m0"));
    // Receipt summaries are the most derived and go before messages.
    assert!(
        bundle.content.completed_receipts.is_empty(),
        "messages were dropped, so no receipt summary"
    );
    assert!(
        bundle
            .manifest
            .omissions
            .iter()
            .any(|o| o.kind == ItemKind::CompletedReceipt)
    );
    // Everything not included is explained.
    let omitted_ids: Vec<&str> = bundle
        .manifest
        .omissions
        .iter()
        .map(|o| o.id.as_str())
        .collect();
    assert!(omitted_ids.contains(&"message:0"), "{omitted_ids:?}");
    assert!(
        bundle
            .manifest
            .omissions
            .iter()
            .all(|o| o.reason == OmissionReason::BudgetExceeded && o.tokens > 0)
    );
    let total = bundle.manifest.budget;
    assert_eq!(
        total.remaining,
        total.available - total.mandatory_tokens - total.optional_tokens
    );
    assert!(total.mandatory_tokens + total.optional_tokens <= total.available);
    // Included plus omitted accounts for every optional item offered.
    let included_optional = bundle
        .manifest
        .included
        .iter()
        .filter(|i| !i.mandatory)
        .count();
    assert!(included_optional >= 2);
}

#[test]
fn pointer_without_text_when_only_the_pointer_fits() {
    let task = task_for_scope();
    let mut inputs = inputs_for(&task);
    inputs.source_locators = vec![locator("req.txt", "r", true), big("opt/big.txt", 1, 20)];
    let base = build_context({
        let mut m = inputs.clone();
        m.source_locators.truncate(1);
        m
    })
    .unwrap();
    let used = base.manifest.budget.mandatory_tokens + base.manifest.budget.optional_tokens;
    inputs.budget = TokenBudget {
        context_tokens: used + 400,
        reserved_output: 0,
        reserved_tools: 0,
    };
    let bundle = build_context(inputs).unwrap();
    let kept = bundle
        .content
        .sources
        .iter()
        .find(|s| s.path == "opt/big.txt")
        .unwrap();
    assert!(kept.content.is_none());
    assert_eq!(kept.sha256.len(), 64, "still version-pinned");
    assert!(
        bundle
            .manifest
            .omissions
            .iter()
            .any(|o| o.kind == ItemKind::SourceContent && o.id == "opt/big.txt")
    );
    assert!(
        bundle
            .manifest
            .source_versions
            .iter()
            .any(|v| v.path == "opt/big.txt")
    );
}

#[test]
fn locators_are_validated() {
    let task = task_for_scope();
    for (path, why) in [
        ("/abs", "absolute"),
        ("a/../b", "dotdot"),
        ("a\\b", "backslash"),
    ] {
        let mut inputs = inputs_for(&task);
        inputs.source_locators = vec![locator(path, "x", true)];
        assert!(
            matches!(
                build_context(inputs),
                Err(ContextError::InvalidLocator { .. })
            ),
            "{why}"
        );
    }
    let mut inputs = inputs_for(&task);
    let mut stale = locator("a.txt", "new text", true);
    stale.sha256 = sha256_hex(b"old text");
    inputs.source_locators = vec![stale];
    assert!(matches!(
        build_context(inputs),
        Err(ContextError::SourceMismatch { .. })
    ));
    // A region excerpt cannot be checked against the whole-file hash, so it is accepted.
    let mut inputs = inputs_for(&task);
    let mut part = locator("a.txt", "excerpt", true);
    part.sha256 = sha256_hex(b"the whole file");
    part.bytes = 999;
    part.region = Some(Region::Lines { start: 3, end: 9 });
    inputs.source_locators = vec![part];
    build_context(inputs).unwrap();
    // The same locator twice is recorded as a duplicate, not silently merged.
    let mut inputs = inputs_for(&task);
    inputs.source_locators = vec![locator("a.txt", "x", true), locator("a.txt", "x", true)];
    let bundle = build_context(inputs).unwrap();
    assert_eq!(bundle.content.sources.len(), 1);
    assert!(
        bundle
            .manifest
            .omissions
            .iter()
            .any(|o| o.reason == OmissionReason::Duplicate)
    );
}

#[test]
fn the_bundle_has_no_place_for_credentials_or_private_reasoning() {
    let bundle = build_context(inputs_for(&task_for_scope())).unwrap();
    let text = serde_json::to_string(&bundle).unwrap().to_lowercase();
    for word in [
        "api_key",
        "password",
        "reasoning",
        "thinking",
        "authorization",
        "token_value",
    ] {
        assert!(!text.contains(word), "{word}");
    }
    let top: Vec<_> = serde_json::to_value(&bundle.content)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(
        top,
        [
            "approved_references",
            "capability_profile",
            "completed_receipts",
            "recent_messages",
            "safety_constraints",
            "scope",
            "sources",
            "unresolved_effects"
        ]
    );
}

#[tokio::test]
async fn stored_bundles_round_trip_and_verify() {
    let w = World::new().await;
    let ready = w.ready_task(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let loaded = load_bundle(&mut tx, ready.bundle.id).await.unwrap();
    assert_eq!(loaded, ready.bundle);
    assert_eq!(loaded.manifest_sha256, ready.bundle.manifest_sha256);
    assert!(matches!(
        load_bundle(&mut tx, BundleId::new()).await,
        Err(BundleError::NotFound)
    ));
    // A bundle that does not verify is not stored.
    let mut bad = sample_bundle(&ready.task);
    bad.manifest.budget.remaining += 1;
    assert!(matches!(
        store_bundle(&mut tx, w.a.project, w.a.account, ready.task.id, &bad).await,
        Err(BundleError::InconsistentBundle)
    ));
}

#[tokio::test]
async fn bundles_are_immutable_and_tampering_is_detected_on_load() {
    let w = World::new().await;
    let ready = w.ready_task(&w.a).await;
    let id = ready.bundle.id;

    // The application role has no UPDATE or DELETE on bundles.
    let mut tx = w.begin(&w.a).await;
    for sql in [
        "UPDATE context_bundles SET manifest_sha256 = repeat('0', 64)",
        "DELETE FROM context_bundles",
    ] {
        let err = must_fail(sqlx::query(sql).execute(&mut *tx).await).await;
        assert!(err.contains("permission denied"), "{sql}: {err}");
        tx.rollback().await.unwrap();
        tx = w.begin(&w.a).await;
    }
    // The trigger refuses even a superuser statement.
    for sql in [
        "UPDATE context_bundles SET created_at = now()",
        "DELETE FROM context_bundles",
    ] {
        let err = must_fail(sqlx::query(sql).execute(&w.db.admin).await).await;
        assert!(err.contains("append-only"), "{sql}: {err}");
    }

    // Tamper as superuser by switching the trigger off: the stored content no longer hashes.
    sqlx::query("ALTER TABLE context_bundles DISABLE TRIGGER context_bundles_append_only")
        .execute(&w.db.admin)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE context_bundles SET bundle = jsonb_set(bundle, '{content,scope,objective}', '\"Delete everything\"') WHERE id = $1",
    )
    .bind(id)
    .execute(&w.db.admin)
    .await
    .unwrap();
    let err = load_bundle(&mut tx, id).await.unwrap_err();
    assert!(
        matches!(err, BundleError::IntegrityFailure { .. }),
        "{err:?}"
    );
    // Tampering with the manifest instead is caught by the manifest hash.
    sqlx::query("UPDATE context_bundles SET bundle = $2 WHERE id = $1")
        .bind(id)
        .bind(serde_json::to_value(&ready.bundle).unwrap())
        .execute(&w.db.admin)
        .await
        .unwrap();
    load_bundle(&mut tx, id).await.unwrap();
    sqlx::query(
        "UPDATE context_bundles SET bundle = jsonb_set(bundle, '{manifest,budget,available}', '1') WHERE id = $1",
    )
    .bind(id)
    .execute(&w.db.admin)
    .await
    .unwrap();
    let err = load_bundle(&mut tx, id).await.unwrap_err();
    match err {
        BundleError::IntegrityFailure { expected, actual } => {
            assert_eq!(expected, ready.bundle.manifest_sha256);
            assert_ne!(actual, expected);
        }
        other => panic!("{other:?}"),
    }
}

// EXP-01 reproduction harness for HOI4 Factory S01 (local clone only, never pushed).
//
// Runs a real `run_transaction` in a child process, hard-kills it at a chosen
// boundary (or lets an upstream abort checkpoint crash it), then runs the
// native recovery action that `find_incomplete_transaction` and
// `recovery_action` report. The independent before/after inventory is taken by
// a separate script outside this harness. This file only drives the
// transaction and writes a small JSON report.
//
// The confirmed-analysis record comes from upstream's own unit-test helper
// `test_codex_analysis()`. It is test-harness evidence for the transaction
// state machine, not a product binding and not proof of the provider gate.

use super::*;
use std::process::Stdio;
use std::time::{Duration, Instant};

fn exp01_bytes(index: usize, size: usize) -> Vec<u8> {
    let line = format!("EXP01 synthetic managed fragment {index:04}, original test text.\n");
    let mut bytes = Vec::with_capacity(size);
    while bytes.len() < size {
        bytes.extend_from_slice(line.as_bytes());
    }
    bytes.truncate(size);
    bytes
}

fn exp01_plan(project: &Path, count: usize, size: usize) -> (InstallationPlan, Vec<PreparedFile>) {
    let mut plan = plan();
    plan.selected_components = vec!["core.agents".into()];
    plan.operations.clear();
    let mut prepared = Vec::new();
    for index in 0..count {
        let bytes = exp01_bytes(index, size);
        let id = format!("exp01-create-{index:04}");
        let destination = format!("exp01_setup/managed/file-{index:04}.md");
        plan.operations.push(PlanOperation {
            id: id.clone(),
            component_id: "core.agents".into(),
            ownership: Some(Ownership::Managed),
            location_scope: None,
            action: OperationAction::Create,
            source_path: Some("generated:exp01".into()),
            destination: destination.clone(),
            source_sha256: Some(sha256_bytes(&bytes)),
            source_size: Some(bytes.len() as u64),
            platform: Some(ManifestPlatform::All),
            executable: false,
            result_sha256: None,
            base_sha256: None,
            local_sha256: None,
            local_state: LocalState::Absent,
            resolution: None,
            external: false,
            rollback: RollbackAction::RemoveCreated,
            external_parent_identity: None,
        });
        prepared.push(PreparedFile {
            operation_id: id,
            destination,
            expected_sha256: sha256_bytes(&bytes),
            bytes,
        });
    }
    // One approved replacement of an existing fixture file exercises backup,
    // quarantine and restore. Its current bytes are the precondition.
    let replace_path = project.join("exp01_setup").join("replace-me.txt");
    if let Ok(current) = fs::read(&replace_path) {
        let incoming = b"EXP01 approved replacement bytes\n".to_vec();
        plan.operations.push(PlanOperation {
            id: "exp01-replace".into(),
            component_id: "core.agents".into(),
            ownership: Some(Ownership::Managed),
            location_scope: None,
            action: OperationAction::Replace,
            source_path: Some("generated:exp01".into()),
            destination: "exp01_setup/replace-me.txt".into(),
            source_sha256: Some(sha256_bytes(&incoming)),
            source_size: Some(incoming.len() as u64),
            platform: Some(ManifestPlatform::All),
            executable: false,
            result_sha256: None,
            base_sha256: None,
            local_sha256: Some(sha256_bytes(&current)),
            local_state: LocalState::Unmodified,
            resolution: None,
            external: false,
            rollback: RollbackAction::RestoreBackup,
            external_parent_identity: None,
        });
        prepared.push(PreparedFile {
            operation_id: "exp01-replace".into(),
            destination: "exp01_setup/replace-me.txt".into(),
            expected_sha256: sha256_bytes(&incoming),
            bytes: incoming,
        });
    }
    (plan, prepared)
}

fn exp01_hoi4ms_leftovers(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else { continue };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                if name != ".git" {
                    stack.push(path);
                }
            } else if name.starts_with(".hoi4ms-") {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                found.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.sort();
    found
}

fn exp01_count_files(directory: &Path) -> usize {
    let mut count = 0;
    let mut stack = vec![directory.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else { continue };
        for entry in entries.filter_map(Result::ok) {
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                stack.push(entry.path());
            } else {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn exp01_apply_worker() {
    if std::env::var("EXP01_MODE").as_deref() != Ok("worker") {
        return;
    }
    let project = PathBuf::from(std::env::var_os("EXP01_PROJECT").unwrap());
    let app = PathBuf::from(std::env::var_os("EXP01_APP").unwrap());
    let plan_path = PathBuf::from(std::env::var_os("EXP01_PLAN").unwrap());
    let (plan, prepared): (InstallationPlan, Vec<PreparedFile>) =
        serde_json::from_slice(&fs::read(plan_path).unwrap()).unwrap();
    let result = run_test_transaction(
        &project,
        &plan,
        &prepared,
        &TransactionOptions {
            app_data_root: Some(app),
            ..Default::default()
        },
    );
    match result {
        Ok((journal, _)) => println!("EXP01_WORKER_RESULT ok state={}", journal.state),
        Err(error) => println!("EXP01_WORKER_RESULT err {error}"),
    }
}

#[test]
fn exp01_trial() {
    let Ok(kill) = std::env::var("EXP01_TRIAL_KILL") else {
        return;
    };
    let project = PathBuf::from(std::env::var_os("EXP01_PROJECT").unwrap());
    let app = PathBuf::from(std::env::var_os("EXP01_APP").unwrap());
    let report_path = PathBuf::from(std::env::var_os("EXP01_REPORT").unwrap());
    let count: usize = std::env::var("EXP01_OPS").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    let size: usize = std::env::var("EXP01_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(262_144);
    fs::create_dir_all(&app).unwrap();
    // recover_only: no new transaction, only the native recovery action for
    // whatever incomplete journal an earlier interruption left.
    let recover_only = kill == "recover_only";
    let (plan, prepared) = exp01_plan(&project, if recover_only { 0 } else { count }, size);
    let plan_path = report_path.with_extension("plan.json");
    let started = Instant::now();
    let mut killed_at = String::from("not_killed");
    let mut observed_staged = 0usize;
    let mut observed_managed = 0usize;
    let (worker_success, worker_code, worker_result) = if recover_only {
        killed_at = "recover_only".into();
        (true, None, String::from("EXP01_WORKER_RESULT not_started"))
    } else {
    fs::write(&plan_path, serde_json::to_vec(&(&plan, &prepared)).unwrap()).unwrap();

    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "transaction::tests::exp01_repro::exp01_apply_worker", "--nocapture"])
        .env("EXP01_MODE", "worker")
        .env("EXP01_PROJECT", &project)
        .env("EXP01_APP", &app)
        .env("EXP01_PLAN", &plan_path)
        .env_remove("EXP01_TRIAL_KILL")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if kill == "finalization" {
        command.env("HOI4_MOD_SETUP_TEST_ABORT_AT", "after_lock_write");
    }
    let mut child = command.spawn().unwrap();
    let staging = crate::paths::transaction_root(&app, plan.plan_id).staging;
    let managed = project.join("exp01_setup").join("managed");
    loop {
        if let Some(_status) = child.try_wait().unwrap() {
            break;
        }
        if started.elapsed() > Duration::from_secs(600) {
            child.kill().ok();
            killed_at = "timeout".into();
            break;
        }
        match kill.as_str() {
            "staging" => {
                observed_staged = exp01_count_files(&staging);
                if observed_staged >= 1 && exp01_count_files(&managed) == 0 {
                    child.kill().unwrap();
                    killed_at = format!("staging staged_files={observed_staged}");
                    break;
                }
            }
            "apply" => {
                observed_managed = exp01_count_files(&managed);
                if observed_managed >= 1 {
                    child.kill().unwrap();
                    killed_at = format!("apply managed_files_seen={observed_managed}");
                    break;
                }
            }
            _ => {}
        }
        std::thread::sleep(Duration::from_micros(200));
    }
    let output = child.wait_with_output().unwrap();
    let worker_stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let worker_result = worker_stdout
        .lines()
        .find(|line| line.starts_with("EXP01_WORKER_RESULT"))
        .unwrap_or("EXP01_WORKER_RESULT none")
        .to_string();
    let _ = fs::remove_file(&plan_path);
    (output.status.success(), output.status.code(), worker_result)
    };
    let leftovers_after_kill = exp01_hoi4ms_leftovers(&project);

    let skip_recovery = std::env::var("EXP01_SKIP_RECOVERY").as_deref() == Ok("1");
    let incomplete = find_incomplete_transaction(&app, &project);
    let mut recovery = serde_json::json!({ "found": false, "skipped": skip_recovery });
    if skip_recovery {
    } else if let Ok(Some(journal)) = &incomplete {
        let action = recovery_action(journal);
        let journal_path = crate::paths::transaction_root(&app, journal.transaction_id)
            .transaction
            .join("journal.json");
        let before_state = journal.state.clone();
        let outcome: Result<String, String> = match action.as_str() {
            "rollback" => {
                let mut journal = read_journal(&journal_path).unwrap();
                rollback_transaction(&project, &mut journal, &journal_path)
                    .map(|()| journal.state.clone())
                    .map_err(|error| error.to_string())
            }
            "resume" => resume_transaction(&project, &app, journal.transaction_id)
                .map(|(journal, _)| journal.state)
                .map_err(|error| error.to_string()),
            "discard_staging" => discard_staging(&project, &app, journal.transaction_id)
                .map(|journal| journal.state)
                .map_err(|error| error.to_string()),
            other => Err(format!("no automatic action for {other}")),
        };
        recovery = serde_json::json!({
            "found": true,
            "transaction_id": journal.transaction_id.to_string(),
            "state_before": before_state,
            "last_checkpoint": journal.last_checkpoint,
            "recommended_action": action,
            "resume_allowed": journal.recovery.resume_allowed,
            "rollback_allowed": journal.recovery.rollback_allowed,
            "discard_staging_allowed": journal.recovery.discard_staging_allowed,
            "project_apply_started": journal.recovery.project_apply_started,
            "outcome": match &outcome { Ok(state) => serde_json::json!({"ok": state}), Err(error) => serde_json::json!({"err": error}) },
        });
    } else if let Err(error) = &incomplete {
        recovery = serde_json::json!({ "found": false, "error": error.to_string() });
    }
    let report = serde_json::json!({
        "kill": kill,
        "ops": count,
        "size": size,
        "plan_id": plan.plan_id.to_string(),
        "killed_at": killed_at,
        "observed_staged": observed_staged,
        "observed_managed": observed_managed,
        "worker_exit_success": worker_success,
        "worker_exit_code": worker_code,
        "worker_result": worker_result,
        "hoi4ms_leftovers_after_kill": leftovers_after_kill,
        "recovery": recovery,
        "hoi4ms_leftovers_after_recovery": exp01_hoi4ms_leftovers(&project),
        "elapsed_ms": started.elapsed().as_millis() as u64,
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

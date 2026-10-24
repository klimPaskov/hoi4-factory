// Workspace, upload, GitHub and Setup routes are served by crates/server (intake_api.rs,
// setup_api.rs). This is the single API module for them. The Setup flow is a server session:
// scan -> evidence approval -> analyze -> confirm values -> plan -> conflict decisions -> approve
// -> apply, with recovery for an interrupted apply. Each call below is one step of that flow, and
// the UI in components/setup/ drives them in order. A 404 or 501 shows a "not available yet"
// state (ApiError.isUnavailable) where a route or capability is missing on this server.
import { request } from './client';

const base = (projectId: string) => `/projects/${encodeURIComponent(projectId)}`;

// ----- Workspace ---------------------------------------------------------------------------

export interface WorkspaceSummary {
  /** Whether a mod is present in the project workspace yet. */
  has_source: boolean;
  /** Where the source came from. */
  origin?: 'created' | 'upload' | 'github' | null;
  file_count?: number;
  /** Branch or ref the workspace is on. */
  ref?: string | null;
  /** Short revision identifier of the current source. */
  revision?: string | null;
  /** Setup state, for example "not_started", "scanned", "applied" or "needs_rollback". */
  setup_state?: string | null;
}

/**
 * GET /api/v1/projects/{id}/workspace answers { summary, descriptor }: the summary is this
 * shape, the descriptor is the C-01 WorkspaceDescriptor (null while the project has no
 * workspace, then summary.has_source is false).
 */
export async function getWorkspace(projectId: string, signal?: AbortSignal): Promise<WorkspaceSummary> {
  const data = await request<
    { summary: WorkspaceSummary } | { workspace: WorkspaceSummary } | WorkspaceSummary
  >(`${base(projectId)}/workspace`, { signal });
  if ('summary' in data) return data.summary;
  if ('workspace' in data) return data.workspace;
  return data;
}

// ----- Upload ------------------------------------------------------------------------------

export interface UploadItem {
  file: File;
  /** Path inside the upload. For a folder this is webkitRelativePath. */
  path: string;
}

/**
 * POST /api/v1/projects/{id}/uploads (multipart/form-data).
 * Fields: `kind` ("zip" or "folder"), `git_init_consent` ("true" lets the server start a Git
 * history when the mod has none), then one `files` part per file whose filename is the relative
 * path inside the upload.
 */
export async function uploadMod(
  projectId: string,
  kind: 'zip' | 'folder',
  items: UploadItem[],
  options: { gitInitConsent?: boolean } = {},
): Promise<{ upload?: { upload_id?: string } }> {
  const form = new FormData();
  form.append('kind', kind);
  form.append('git_init_consent', options.gitInitConsent ? 'true' : 'false');
  for (const item of items) form.append('files', item.file, item.path);
  return request(`${base(projectId)}/uploads`, { method: 'POST', form });
}

// ----- GitHub ------------------------------------------------------------------------------

export interface GithubRepository {
  /** "owner/name". */
  full_name: string;
  default_branch: string;
  private: boolean;
}

/** GET /api/v1/projects/{id}/github/repositories */
export async function listGithubRepositories(
  projectId: string,
  signal?: AbortSignal,
): Promise<GithubRepository[]> {
  const data = await request<{ repositories: GithubRepository[] }>(
    `${base(projectId)}/github/repositories`,
    { signal },
  );
  return data.repositories;
}

/** POST /api/v1/projects/{id}/github/import. `ref` is a branch name; empty means the default branch. */
export async function importFromGithub(
  projectId: string,
  input: { repository: string; ref?: string },
): Promise<{ link?: { id: string } }> {
  return request(`${base(projectId)}/github/import`, { method: 'POST', body: input });
}

// ----- Setup -------------------------------------------------------------------------------

const setupBase = (projectId: string) => `${base(projectId)}/setup`;

export interface EvidenceCandidate {
  /** Stable name for this excerpt. Approval sends back the references the person kept. */
  reference: string;
  path: string;
  category: string;
  key: string;
  excerpt: string;
  excerpt_sha256: string;
}

export interface SetupEvidence {
  /** True when the model needs excerpts of the mod, so the person must approve what is sent. */
  required: boolean;
  /** True when the candidate list was cut short. */
  truncated: boolean;
  candidates: EvidenceCandidate[];
}

export interface SetupScan {
  scan_id: string;
  session_id?: string;
  mode?: 'create' | 'existing';
  /** True when limits cut the scan short. Incomplete coverage must be shown. */
  incomplete: boolean;
  receipt_id?: string;
  /** Whether the scan left the workspace untouched. Null when that was not checked. */
  workspace_unchanged?: boolean | null;
  /** Plain-language facts the scan found, for the user to confirm or correct. */
  facts: { label: string; value: string }[];
  evidence: SetupEvidence;
}

/** Fills what a sparser answer leaves out, so the UI never reads undefined. */
function normalizeScan(scan: Partial<SetupScan> & { scan_id: string }): SetupScan {
  return {
    ...scan,
    incomplete: scan.incomplete ?? false,
    facts: scan.facts ?? [],
    evidence: {
      required: scan.evidence?.required ?? false,
      truncated: scan.evidence?.truncated ?? false,
      candidates: scan.evidence?.candidates ?? [],
    },
  };
}

/**
 * POST /api/v1/projects/{id}/setup/scan starts (or restarts) the Setup session. `mode: "create"`
 * plans a new mod (the server prepares an empty workspace); `"existing"` inspects what was
 * brought in and answers 409 `no_source` when nothing was. The scan is read-only.
 */
export async function scanSetup(
  projectId: string,
  input: { mode: 'create' | 'existing'; description?: string },
): Promise<SetupScan> {
  const scan = await request<Partial<SetupScan> & { scan_id: string }>(
    `${setupBase(projectId)}/scan`,
    { method: 'POST', body: input },
  );
  return normalizeScan(scan);
}

/**
 * POST .../setup/evidence records which excerpts the person approved for the model. Only the
 * approved references are ever sent to the model provider. An empty list is allowed.
 */
export async function approveEvidence(
  projectId: string,
  input: { scan_id: string; references: string[] },
): Promise<{ approved: number }> {
  return request(`${setupBase(projectId)}/evidence`, { method: 'POST', body: input });
}

export interface ReviewValues {
  description: string;
  /** One folder path per entry. */
  folder_profile: string[];
  identity: {
    display_name: string;
    project_id: string;
    author: string;
    version: string;
    supported_game_version: string;
    script_prefix: string;
    primary_namespace: string;
    default_branch: string;
    descriptor_tags: string[];
  };
  conventions: {
    agents_profile: string;
    localisation_convention: string;
    documentation_convention: string;
  };
}

export interface SetupAnalysis {
  analysis_id: string;
  attempts: number;
  model: string;
  source: { revision: string; manifest_sha256: string };
  summary: string;
  warnings: string[];
  proposals: { key: string; value: string; confidence: number; reason: string }[];
  values: ReviewValues;
  component_recommendations: { component_id: string; recommendation: string; reason: string }[];
}

/**
 * POST .../setup/analyze asks the model to propose the values. It is a long call (up to about
 * three minutes), so the caller shows progress and does not cancel it when the person leaves.
 */
export async function analyzeSetup(projectId: string): Promise<SetupAnalysis> {
  return request(`${setupBase(projectId)}/analyze`, { method: 'POST', body: {} });
}

/**
 * POST .../setup/confirm records exactly the values the person confirmed (edited or not).
 * A rejected value answers 422 `invalid_request` with a message that names the field.
 */
export async function confirmSetupValues(
  projectId: string,
  input: { analysis_id: string; values: ReviewValues },
): Promise<{ confirmed: true; values_sha256: string }> {
  return request(`${setupBase(projectId)}/confirm`, { method: 'POST', body: input });
}

export type PlanAction = 'create' | 'change' | 'preserve' | 'conflict';

export interface SetupPlan {
  plan_id: string;
  /** The hash of the plan as shown. Approval names exactly this hash. */
  plan_sha256: string;
  root_identity: string;
  state: string;
  approved: boolean;
  operations: { path: string; action: PlanAction; note?: string }[];
  /** Decisions the person must make. `selected` is null until one is chosen. */
  conflicts: { path: string; options: string[]; selected: string | null }[];
  components: { name: string; version: string }[];
  source: { revision: string } | null;
  summary: { create: number; change: number; preserve: number; conflict: number };
}

/** POST .../setup/plan builds the plan from the confirmed values (a dry run; nothing is written). */
export async function buildSetupPlan(projectId: string): Promise<SetupPlan> {
  const data = await request<{ plan: SetupPlan }>(`${setupBase(projectId)}/plan`, {
    method: 'POST',
    body: {},
  });
  return data.plan;
}

/**
 * GET .../setup/plan answers { plan }. 409 `plan_not_built` when none was built yet; 501
 * `capability_unavailable` when planning is not available on this server.
 */
export async function getSetupPlan(projectId: string, signal?: AbortSignal): Promise<SetupPlan> {
  const data = await request<{ plan: SetupPlan }>(`${setupBase(projectId)}/plan`, { signal });
  return data.plan;
}

/**
 * POST .../setup/conflicts records the person's choice for each conflict. The answer is a NEW
 * plan with a new hash, and any earlier approval is withdrawn.
 */
export async function decideConflicts(
  projectId: string,
  input: { plan_id: string; decisions: { path: string; choice: string }[] },
): Promise<SetupPlan> {
  const data = await request<{ plan: SetupPlan }>(`${setupBase(projectId)}/conflicts`, {
    method: 'POST',
    body: input,
  });
  return data.plan;
}

/**
 * POST .../setup/approve approves exactly the plan with this hash. 409 `plan_changed` means the
 * plan changed since it was shown, so the caller reloads it.
 */
export async function approveSetupPlan(
  projectId: string,
  input: { plan_id: string; plan_sha256: string },
): Promise<{ approval: { approval_id: string; plan_sha256: string; root_identity: string } }> {
  return request(`${setupBase(projectId)}/approve`, { method: 'POST', body: input });
}

export interface SetupChanges {
  added: string[];
  removed: string[];
  changed: string[];
  mode_changed: string[];
  git_index_changed: boolean;
}

export interface SetupApplyResult {
  /** "applied", "needs_rollback" (the project was changed and the apply did not finish), "interrupted" or "failed". */
  status: string;
  plan_id?: string;
  receipt_id?: string;
  journal_id?: string;
  journal_state?: string;
  /** For an applied plan: what changed in the workspace. */
  changes?: SetupChanges;
  /** Paths that changed although the plan did not list them. Worth a warning. */
  unplanned_changes?: string[];
  /** For an unfinished apply: the one recovery the journal permits. */
  variant?: string;
  explanation?: string;
}

/**
 * POST .../setup/apply applies exactly the plan named by `plan_id`, and only once it is
 * approved. Without approval the server refuses and changes nothing. (The server still accepts
 * an optional `plan_sha256` that approves first; the wizard approves explicitly instead.)
 */
export async function applySetup(projectId: string, planId: string): Promise<SetupApplyResult> {
  return request(`${setupBase(projectId)}/apply`, { method: 'POST', body: { plan_id: planId } });
}

export type SetupStep =
  | 'none'
  | 'scanned'
  | 'evidence_approved'
  | 'analyzed'
  | 'confirmed'
  | 'planned'
  | 'approved'
  | 'applied'
  | 'expired';

export interface SetupStatus {
  step: SetupStep;
  session_open: boolean;
  session: { session_id: string; mode: 'create' | 'existing' } | null;
  scan: SetupScan | null;
  analysis: SetupAnalysis | null;
  confirmed_values: ReviewValues | null;
  plan: SetupPlan | null;
}

/** GET .../setup/status lets the page restore the right step after a reload. */
export async function getSetupStatus(projectId: string, signal?: AbortSignal): Promise<SetupStatus> {
  const data = await request<Partial<SetupStatus>>(`${setupBase(projectId)}/status`, { signal });
  const scan = data.scan ? normalizeScan(data.scan) : null;
  const mode = data.session?.mode;
  return {
    step: data.step ?? 'none',
    session_open: data.session_open ?? false,
    session: data.session ?? null,
    scan: scan && mode && !scan.mode ? { ...scan, mode } : scan,
    analysis: data.analysis ?? null,
    confirmed_values: data.confirmed_values ?? null,
    plan: data.plan ?? null,
  };
}

export interface SetupRecovery {
  journal_id: string;
  variant: string;
  /** The one action the journal permits; null when only manual review is possible. */
  allowed_action: 'resume' | 'rollback' | 'discard_staging' | null;
  explanation: string;
  journal_state: string;
  /** True while a server process still owns the apply, so acting now could collide with it. */
  owner_active: boolean;
}

/** GET .../setup/recovery answers { recovery }, null when nothing was interrupted. */
export async function getSetupRecovery(
  projectId: string,
  signal?: AbortSignal,
): Promise<SetupRecovery | null> {
  const data = await request<{ recovery: SetupRecovery | null }>(
    `${setupBase(projectId)}/recovery`,
    { signal },
  );
  return data.recovery ?? null;
}

export interface SetupRecoveryOutcome {
  action: string;
  journal_state: string;
  /** After a rollback: whether the files are exactly as before, or what still differs. */
  restoration?: { exact: boolean; differences: SetupChanges };
}

/** POST .../setup/recover takes the one action the journal permits for `variant`. */
export async function recoverSetup(
  projectId: string,
  input: { journal_id: string; variant: string },
): Promise<SetupRecoveryOutcome> {
  const data = await request<{ recovery: SetupRecoveryOutcome }>(`${setupBase(projectId)}/recover`, {
    method: 'POST',
    body: input,
  });
  return data.recovery;
}

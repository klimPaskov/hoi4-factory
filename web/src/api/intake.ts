// Workspace, upload and GitHub routes are served by crates/server (intake_api.rs). The Setup
// routes below are PENDING: the assumed contract is used until the server serves them, and a 404
// shows a "not available yet" state (ApiError.isUnavailable).
import { request } from './client';

/** Endpoints this client expects but that the server does not serve yet. */
export const PENDING_ENDPOINTS = [
  'POST /api/v1/projects/{id}/setup/scan',
  'GET /api/v1/projects/{id}/setup/plan',
  'POST /api/v1/projects/{id}/setup/apply',
] as const;

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

export interface SetupScan {
  scan_id: string;
  /** Plain-language facts the scan found, for the user to confirm or correct. */
  facts?: { label: string; value: string }[];
  /** True when limits cut the scan short. Incomplete coverage must be shown. */
  incomplete?: boolean;
}

/** PENDING: POST /api/v1/projects/{id}/setup/scan. `mode: "create"` plans a new mod; `"existing"` inspects what was brought in. */
export async function scanSetup(
  projectId: string,
  input: { mode: 'create' | 'existing'; description?: string },
): Promise<SetupScan> {
  return request(`${base(projectId)}/setup/scan`, { method: 'POST', body: input });
}

export type PlanAction = 'create' | 'change' | 'preserve' | 'conflict';

export interface SetupPlan {
  plan_id: string;
  operations: { path: string; action: PlanAction; note?: string }[];
  components?: { name: string; version: string }[];
}

/** PENDING: GET /api/v1/projects/{id}/setup/plan (the dry run; nothing is written). */
export async function getSetupPlan(projectId: string, signal?: AbortSignal): Promise<SetupPlan> {
  const data = await request<{ plan: SetupPlan } | SetupPlan>(`${base(projectId)}/setup/plan`, {
    signal,
  });
  return 'plan' in data ? data.plan : data;
}

export interface SetupApplyResult {
  /** For example "applied", "needs_rollback" or "failed". */
  status: string;
  receipt_id?: string;
}

/** PENDING: POST /api/v1/projects/{id}/setup/apply. Authorizes exactly the plan named by `plan_id`. */
export async function applySetup(projectId: string, planId: string): Promise<SetupApplyResult> {
  return request(`${base(projectId)}/setup/apply`, { method: 'POST', body: { plan_id: planId } });
}

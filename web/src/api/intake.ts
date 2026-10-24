// PENDING ENDPOINTS. Everything in this module talks to routes that are being built in parallel
// (intake and Setup). The paths and shapes below are the assumed contract. Until the server
// serves them, the routes answer 404 and the UI shows a "not available yet" state
// (ApiError.isUnavailable). Adjust the shapes here when the real contract lands.
import { request } from './client';

/** Endpoints this client expects but that did not exist when it was written. */
export const PENDING_ENDPOINTS = [
  'GET /api/v1/game-reference',
  'POST /api/v1/projects/{id}/uploads',
  'GET /api/v1/projects/{id}/github/repositories',
  'POST /api/v1/projects/{id}/github/import',
  'POST /api/v1/projects/{id}/setup/scan',
  'GET /api/v1/projects/{id}/setup/plan',
  'POST /api/v1/projects/{id}/setup/apply',
  'GET /api/v1/projects/{id}/workspace',
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

/** PENDING: GET /api/v1/projects/{id}/workspace */
export async function getWorkspace(projectId: string, signal?: AbortSignal): Promise<WorkspaceSummary> {
  const data = await request<{ workspace: WorkspaceSummary } | WorkspaceSummary>(
    `${base(projectId)}/workspace`,
    { signal },
  );
  return 'workspace' in data ? data.workspace : data;
}

// ----- Upload ------------------------------------------------------------------------------

export interface UploadItem {
  file: File;
  /** Path inside the upload. For a folder this is webkitRelativePath. */
  path: string;
}

/**
 * PENDING: POST /api/v1/projects/{id}/uploads (multipart/form-data).
 * Fields: `kind` ("zip" or "folder"), then one `files` part per file whose filename is the
 * relative path inside the upload.
 */
export async function uploadMod(
  projectId: string,
  kind: 'zip' | 'folder',
  items: UploadItem[],
): Promise<{ upload_id?: string }> {
  const form = new FormData();
  form.append('kind', kind);
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

/** PENDING: GET /api/v1/projects/{id}/github/repositories */
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

/** PENDING: POST /api/v1/projects/{id}/github/import. `ref` is a branch, tag or commit; empty means default. */
export async function importFromGithub(
  projectId: string,
  input: { repository: string; ref?: string },
): Promise<{ import_id?: string }> {
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

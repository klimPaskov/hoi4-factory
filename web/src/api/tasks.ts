// Tasks, model profiles, API keys and usage (stage S03). The routes are built next to the
// client, so a 404 shows the "not available yet" state (ApiError.isUnavailable). Money is
// integer micro-dollars everywhere on the wire.
import { request } from './client';

/** Endpoints a server build may not have yet. A 404 or 501 on these is information, not failure. */
export const PENDING_ENDPOINTS = [
  'GET /model-profiles',
  'GET /account/api-keys',
  'POST /account/api-keys',
  'DELETE /account/api-keys/{id}',
  'GET /account/usage',
  'GET /projects/{id}/tasks',
  'POST /projects/{id}/tasks',
  'GET /projects/{id}/tasks/{task_id}',
  'POST /projects/{id}/tasks/{task_id}/attempts',
  'POST /projects/{id}/tasks/{task_id}/pause',
  'POST /projects/{id}/tasks/{task_id}/resume',
  'POST /projects/{id}/tasks/{task_id}/attempts/{attempt_id}/interrupt',
  'POST /projects/{id}/tasks/{task_id}/grants/{grant_id}/revoke',
  'GET /projects/{id}/tasks/{task_id}/events',
  'GET /projects/{id}/tasks/{task_id}/events/stream',
] as const;

export const MICROS_PER_USD = 1_000_000;

// ----- Model profiles ----------------------------------------------------------------------

export interface ModelProfile {
  id: string;
  label: string;
  provider: string;
  default: boolean;
  operator_allowance: boolean;
  capabilities: Record<string, string>;
}

export interface ModelProfiles {
  profiles: ModelProfile[];
  user_keys_available: boolean;
}

export function getModelProfiles(signal?: AbortSignal): Promise<ModelProfiles> {
  return request('/model-profiles', { signal });
}

// ----- API keys ----------------------------------------------------------------------------

export type KeyProvider = 'anthropic' | 'openai' | 'openai_compatible';

export interface ApiKeyRecord {
  id: string;
  provider: KeyProvider;
  label: string;
  base_url: string | null;
  key_hint: string;
  created_at: string;
  last_used_at: string | null;
}

export interface ApiKeyList {
  available: boolean;
  keys: ApiKeyRecord[];
}

export function listApiKeys(signal?: AbortSignal): Promise<ApiKeyList> {
  return request('/account/api-keys', { signal });
}

export function addApiKey(input: {
  provider: KeyProvider;
  label: string;
  api_key: string;
  base_url?: string;
}): Promise<ApiKeyRecord> {
  return request('/account/api-keys', { method: 'POST', body: input });
}

export async function deleteApiKey(id: string): Promise<void> {
  await request<void>(`/account/api-keys/${encodeURIComponent(id)}`, { method: 'DELETE' });
}

// ----- Usage -------------------------------------------------------------------------------

export interface Usage {
  month: string;
  verified: boolean;
  allowance_micros: number;
  operator_spent_micros: number;
  remaining_allowance_micros: number;
  unknown_count: number;
  user_key_spent_micros: number;
}

export function getUsage(signal?: AbortSignal): Promise<Usage> {
  return request('/account/usage', { signal });
}

// ----- Tasks -------------------------------------------------------------------------------

export type TaskMode = 'read_only' | 'edit';
export type TaskState =
  | 'planned'
  | 'ready'
  | 'running'
  | 'waiting_for_input'
  | 'in_review'
  | 'completed'
  | 'blocked'
  | 'cancelled';

export interface Task {
  id: string;
  project_id: string;
  objective: string;
  mode: TaskMode;
  allowed_paths: string[];
  state: TaskState;
  paused: boolean;
  pause_reason: string | null;
  budget_micros: number;
  max_turns: number;
  model: { profile_id: string; funding: 'operator_allowance' | 'user_key'; key_id: string | null };
  version: number;
  last_seq: number;
  spent_micros: number;
  spend_unknown: boolean;
  created_at: string;
  updated_at: string;
}

export type AttemptState =
  | 'admitted'
  | 'running'
  | 'interrupt_requested'
  | 'reconciling'
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'unknown_effect';

/** Attempt states in which the agent may still be doing something. */
export const ACTIVE_ATTEMPT_STATES: readonly AttemptState[] = [
  'admitted',
  'running',
  'interrupt_requested',
  'reconciling',
];

export interface Grant {
  id: string;
  revocation_epoch: number;
  revoked: boolean;
  operation_classes: string[];
  allowed_roots: string[];
  expires_at: string;
}

export interface AttemptOutcome {
  summary: string;
  changed_files: string[];
  branch: string | null;
  commit: string | null;
  open_questions: string[];
}

export interface Attempt {
  id: string;
  attempt_no: number;
  state: AttemptState;
  grant: Grant;
  bundle: { id: string; manifest_sha256: string };
  error_code: string | null;
  outcome: AttemptOutcome | null;
  started_at: string | null;
  finished_at: string | null;
  version: number;
}

export interface PendingEffect {
  operation_id: string;
  tool: string;
  status: string;
}

export interface TaskDetail {
  task: Task;
  attempts: Attempt[];
  pending_effects: PendingEffect[];
}

export interface RunEvent {
  seq: number;
  event_id: string;
  attempt_id: string | null;
  type: string;
  actor: string;
  summary: string;
  payload: Record<string, unknown>;
  created_at: string;
}

export interface NewTask {
  objective: string;
  mode: TaskMode;
  allowed_paths: string[];
  budget_usd: number;
  model: { profile_id: string; key_id: string | null };
}

const tasksBase = (projectId: string) => `/projects/${encodeURIComponent(projectId)}/tasks`;
const taskPath = (projectId: string, taskId: string) =>
  `${tasksBase(projectId)}/${encodeURIComponent(taskId)}`;

export async function listTasks(projectId: string, signal?: AbortSignal): Promise<Task[]> {
  const data = await request<{ tasks: Task[] }>(tasksBase(projectId), { signal });
  return data.tasks;
}

export function createTask(projectId: string, input: NewTask): Promise<Task> {
  return request(tasksBase(projectId), { method: 'POST', body: input });
}

export function getTask(
  projectId: string,
  taskId: string,
  signal?: AbortSignal,
): Promise<TaskDetail> {
  return request(taskPath(projectId, taskId), { signal });
}

export function startAttempt(
  projectId: string,
  taskId: string,
  expectedVersion: number,
): Promise<{ task: Task; attempt: Attempt }> {
  return request(`${taskPath(projectId, taskId)}/attempts`, {
    method: 'POST',
    body: { expected_version: expectedVersion },
  });
}

export function pauseTask(
  projectId: string,
  taskId: string,
  expectedVersion: number,
  reason?: string,
): Promise<Task> {
  return request(`${taskPath(projectId, taskId)}/pause`, {
    method: 'POST',
    body: reason ? { expected_version: expectedVersion, reason } : { expected_version: expectedVersion },
  });
}

export function resumeTask(
  projectId: string,
  taskId: string,
  expectedVersion: number,
): Promise<Task> {
  return request(`${taskPath(projectId, taskId)}/resume`, {
    method: 'POST',
    body: { expected_version: expectedVersion },
  });
}

export function interruptAttempt(
  projectId: string,
  taskId: string,
  attemptId: string,
): Promise<Attempt> {
  return request(
    `${taskPath(projectId, taskId)}/attempts/${encodeURIComponent(attemptId)}/interrupt`,
    { method: 'POST' },
  );
}

export function revokeGrant(
  projectId: string,
  taskId: string,
  grantId: string,
): Promise<{ id: string; revocation_epoch: number; revoked: boolean }> {
  return request(`${taskPath(projectId, taskId)}/grants/${encodeURIComponent(grantId)}/revoke`, {
    method: 'POST',
  });
}

export function listEvents(
  projectId: string,
  taskId: string,
  after: number,
  limit = 200,
  signal?: AbortSignal,
): Promise<{ events: RunEvent[]; last_seq: number }> {
  return request(`${taskPath(projectId, taskId)}/events?after=${after}&limit=${limit}`, { signal });
}

/** Same-origin URL for EventSource; the session cookie rides along. */
export function eventStreamUrl(projectId: string, taskId: string, after: number): string {
  return `/api/v1${taskPath(projectId, taskId)}/events/stream?after=${after}`;
}

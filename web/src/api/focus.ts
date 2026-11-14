// The focus editor, review, commit and publication routes (stage S04). The focus projection and
// the change card types live in ../focus/types.ts and are shared with the design lab.
import { request, requestBlob } from './client';
import type { HoverMap } from '../focus/GamePreview';
import type { FocusProjection, Verdict } from '../focus/types';

/** One focus operation, the same vocabulary the agent's focus_edit tool uses. */
export type FocusOp =
  | { op: 'move'; focus_id: string; x: number; y: number }
  | { op: 'set_cost'; focus_id: string; cost: number }
  | { op: 'link'; focus_id: string; prerequisites: string[] }
  | { op: 'set_text'; focus_id: string; title: string; description: string }
  | {
      op: 'create';
      focus_id: string;
      x: number;
      y: number;
      cost: number;
      prerequisites?: string[];
      icon?: string;
      title: string;
      description: string;
      reward?: string;
    };

/** A C-02 change proposal as the server returns it. */
export interface Proposal {
  id: string;
  taskId: string;
  actorKind: 'person' | 'native_agent' | 'coding_client';
  actorAccountId: string | null;
  attemptId: string | null;
  treeId: string;
  toolOperation: string;
  toolVersion: string;
  baseProjectionHash: string;
  operations: unknown;
  requestSha256: string;
  before: Record<string, string>;
  after: Record<string, string>;
  changedPaths: string[];
  changedIds: string[];
  summary: {
    added: string[];
    changed: { id: string; fields: { name: string; before: string; after: string }[] }[];
    removed: string[];
    files?: string[];
  };
  diffs: { path: string; diff: string }[];
  proposalHash: string;
  state: 'applied' | 'committed' | 'discarded' | 'reverted';
  afterProjectionId: string | null;
  engineReport: EngineCheck | null;
  findingDelta: FindingDeltaSummary | null;
  createdAt: string;
  updatedAt: string;
}

export interface FindingDeltaSummary {
  introduced: number;
  worsened: number;
  changed: number;
  improved: number;
  unchanged: number;
  resolved: number;
}

export interface EngineCheck {
  verdict: Verdict;
  engine_version: string;
  engine_rev: string;
  game_version: string | null;
  baseline_commit: string;
  new_findings: number;
  baselined_findings: number;
  blocking: { code: string; path: string; line: number; message: string }[];
  unknown: {
    references_resolved: number;
    references_unresolved: number;
    references_unknown: number;
    commands_resolved: number;
    commands_unresolved: number;
    commands_unknown: number;
    unmodelled_script_files: number;
    vocabulary_available: boolean;
    game_layer: boolean;
  };
  elapsed_ms: number;
}

export interface Publication {
  id: string;
  kind: 'commit' | 'push' | 'pull_request' | 'restore';
  idempotencyKey: string;
  state: 'intent' | 'succeeded' | 'failed' | 'unknown';
  intent: Record<string, unknown>;
  result: Record<string, unknown> | null;
  createdAt: string;
  settledAt: string | null;
}

const taskPath = (projectId: string, taskId: string) =>
  `/projects/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(taskId)}`;

/** A fresh idempotency key for one effect. Reuse it only to repeat that same request. */
export function newIdempotencyKey(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

export function getProjection(projectId: string, taskId: string, treeId: string, signal?: AbortSignal): Promise<FocusProjection> {
  return request(`${taskPath(projectId, taskId)}/focus/trees/${encodeURIComponent(treeId)}`, { signal });
}

export function applyOperations(
  projectId: string,
  taskId: string,
  treeId: string,
  expectedProjectionHash: string,
  operations: FocusOp[],
): Promise<{ proposal: Proposal; projection: FocusProjection }> {
  return request(`${taskPath(projectId, taskId)}/focus/trees/${encodeURIComponent(treeId)}/operations`, {
    method: 'POST',
    body: { expected_projection_hash: expectedProjectionHash, operations },
  });
}

/** The hover map of the tree's game render: where each focus is drawn. */
export function getPreviewMap(projectId: string, taskId: string, treeId: string, signal?: AbortSignal): Promise<HoverMap> {
  return request(`${taskPath(projectId, taskId)}/focus/trees/${encodeURIComponent(treeId)}/preview.json`, { signal });
}

/** The tree as the game draws it, as a PNG. */
export function getPreviewImage(projectId: string, taskId: string, treeId: string, signal?: AbortSignal): Promise<Blob> {
  return requestBlob(`${taskPath(projectId, taskId)}/focus/trees/${encodeURIComponent(treeId)}/preview.png`, signal);
}

export function getProposal(projectId: string, taskId: string, proposalId: string, signal?: AbortSignal): Promise<Proposal> {
  return request(`${taskPath(projectId, taskId)}/proposals/${encodeURIComponent(proposalId)}`, { signal });
}

/** Puts an uncommitted proposal's files back. */
export function discardProposal(projectId: string, taskId: string, proposalId: string): Promise<unknown> {
  return request(`${taskPath(projectId, taskId)}/proposals/${encodeURIComponent(proposalId)}/discard`, { method: 'POST', body: {} });
}

export async function listProposals(projectId: string, taskId: string, signal?: AbortSignal): Promise<Proposal[]> {
  const data = await request<{ proposals: Proposal[] }>(`${taskPath(projectId, taskId)}/proposals`, { signal });
  return data.proposals;
}

export function runCheck(projectId: string, taskId: string): Promise<{ check: EngineCheck; finding_delta: { summary: FindingDeltaSummary } }> {
  return request(`${taskPath(projectId, taskId)}/check`, { method: 'POST', body: {} });
}

export function commitProposals(
  projectId: string,
  taskId: string,
  proposals: { id: string; proposal_hash: string }[],
  message: string,
  idempotencyKey: string,
  acceptIncomplete: boolean,
): Promise<{ publication_id: string; result: Record<string, unknown> }> {
  return request(`${taskPath(projectId, taskId)}/commit`, {
    method: 'POST',
    body: { proposals, message, idempotency_key: idempotencyKey, accept_incomplete: acceptIncomplete },
  });
}

export function restoreCommit(projectId: string, taskId: string, publicationId: string, idempotencyKey: string) {
  return request<{ publication_id: string; result: Record<string, unknown> }>(`${taskPath(projectId, taskId)}/restore`, {
    method: 'POST',
    body: { publication_id: publicationId, idempotency_key: idempotencyKey },
  });
}

export async function listPublications(projectId: string, taskId: string, signal?: AbortSignal): Promise<Publication[]> {
  const data = await request<{ publications: Publication[] }>(`${taskPath(projectId, taskId)}/publications`, { signal });
  return data.publications;
}

export function createPublicationGrant(projectId: string, taskId: string) {
  return request<{ grant_id: string; epoch: number; branch: string; repository: string; expires_at: string }>(
    `${taskPath(projectId, taskId)}/publication-grants`,
    { method: 'POST', body: { operation_classes: ['push_owned_branch', 'open_pull_request'] } },
  );
}

export function pushBranch(projectId: string, taskId: string, grantId: string, epoch: number, idempotencyKey: string) {
  return request<{ publication_id: string; result: Record<string, unknown> }>(`${taskPath(projectId, taskId)}/push`, {
    method: 'POST',
    body: { grant_id: grantId, epoch, idempotency_key: idempotencyKey },
  });
}

export function openPullRequest(
  projectId: string,
  taskId: string,
  grantId: string,
  epoch: number,
  idempotencyKey: string,
  title: string,
  body: string,
) {
  return request<{ publication_id: string; result: { pull_request: { number: number; url: string } } }>(
    `${taskPath(projectId, taskId)}/pull-request`,
    { method: 'POST', body: { grant_id: grantId, epoch, idempotency_key: idempotencyKey, title, body } },
  );
}

export function syncBranch(projectId: string, taskId: string) {
  return request<{ external_changes: boolean; changed_paths?: string[]; from?: string; to?: string }>(
    `${taskPath(projectId, taskId)}/sync`,
    { method: 'POST', body: {} },
  );
}

/** Same-origin URL of the task branch as a ZIP; the session cookie rides along. */
export function downloadUrl(projectId: string, taskId: string): string {
  return `/api/v1${taskPath(projectId, taskId)}/download`;
}

import type { Handler } from './helpers';
import { project, signedIn } from './helpers';

/** Shapes the Setup routes answer with, as the server contract describes them. */

export const BASE = '/projects/prj_1';
export const SETUP = `${BASE}/setup`;
export const HASH_A = 'a'.repeat(64);
export const HASH_B = 'b'.repeat(64);
export const ROOT = 'c'.repeat(64);

export function evidenceCandidates() {
  return [
    {
      reference: 'ev-1',
      path: 'descriptor.mod',
      category: 'descriptor',
      key: 'name',
      excerpt: 'name="Test Mod"',
      excerpt_sha256: '1'.repeat(64),
    },
    {
      reference: 'ev-2',
      path: 'common/ideas/a.txt',
      category: 'ideas',
      key: 'ideas',
      excerpt: 'ideas = {\n  country = {}\n}',
      excerpt_sha256: '2'.repeat(64),
    },
  ];
}

export function scanJson(overrides: Record<string, unknown> = {}) {
  return {
    scan_id: 'scan_1',
    session_id: 'sess_1',
    mode: 'existing',
    incomplete: false,
    receipt_id: 'rcpt_scan',
    workspace_unchanged: true,
    facts: [
      { label: 'Mod name', value: 'مود' },
      { label: 'Files', value: '42' },
    ],
    evidence: { required: true, truncated: false, candidates: evidenceCandidates() },
    ...overrides,
  };
}

export function reviewValues() {
  return {
    description: 'A test mod',
    folder_profile: ['common', 'events'],
    identity: {
      display_name: 'Test Mod',
      project_id: 'test_mod',
      author: 'Modder',
      version: '0.1.0',
      supported_game_version: '1.19.*',
      script_prefix: 'tm',
      primary_namespace: 'tm',
      default_branch: 'main',
      descriptor_tags: ['Alternative History', 'Gameplay'],
    },
    conventions: {
      agents_profile: 'standard',
      localisation_convention: 'english-only',
      documentation_convention: 'readme',
    },
  };
}

export function analysisJson() {
  return {
    analysis_id: 'an_1',
    attempts: 2,
    model: 'claude-test-model',
    source: { revision: 'rev1', manifest_sha256: '3'.repeat(64) },
    summary: 'A small focus tree mod.',
    warnings: ['No localisation files were found.'],
    proposals: [
      { key: 'identity.display_name', value: 'Test Mod', confidence: 0.85, reason: 'Named in descriptor.mod' },
    ],
    values: reviewValues(),
    component_recommendations: [
      { component_id: 'hoi4-agent-tools', recommendation: 'install', reason: 'The mod has focus trees.' },
    ],
  };
}

export function planJson(overrides: Record<string, unknown> = {}) {
  return {
    plan_id: 'p1',
    plan_sha256: HASH_A,
    root_identity: ROOT,
    state: 'planned',
    approved: false,
    operations: [
      { path: 'descriptor.mod', action: 'change' },
      { path: 'common/new.txt', action: 'create' },
      { path: 'map/provinces.bmp', action: 'preserve' },
    ],
    conflicts: [],
    components: [{ name: 'hoi4-agent-tools', version: '1.2.3' }],
    source: { revision: 'rev1' },
    summary: { create: 1, change: 1, preserve: 1, conflict: 0 },
    ...overrides,
  };
}

export function appliedJson() {
  return {
    status: 'applied',
    plan_id: 'p1',
    journal_id: 'j1',
    journal_state: 'completed',
    changes: {
      added: ['common/new.txt'],
      removed: [],
      changed: ['descriptor.mod'],
      mode_changed: [],
      git_index_changed: false,
    },
    unplanned_changes: [],
    receipt_id: 'rcpt_apply',
  };
}

/** GET status as the server answers it for a given step. */
export function statusJson(step: string, extra: Record<string, unknown> = {}) {
  const open = step !== 'none' && step !== 'expired';
  return {
    status: {},
    session_open: open,
    step,
    session: open ? { session_id: 'sess_1', mode: 'existing' } : null,
    scan: null,
    analysis: null,
    confirmed_values: null,
    plan: null,
    ...extra,
  };
}

export function recoveryJson(overrides: Record<string, unknown> = {}) {
  return {
    journal_id: 'j1',
    variant: 'rollback_after_failed_apply',
    allowed_action: 'rollback',
    explanation: 'The apply stopped after changing two files.',
    journal_state: 'applying',
    owner_active: false,
    ...overrides,
  };
}

/** Routes for a signed-in owner opening a project whose Setup session is in the given state. */
export function setupRoutes(
  status: Record<string, unknown>,
  extra: Record<string, Handler> = {},
): Record<string, Handler> {
  return {
    ...signedIn(),
    [`GET ${BASE}`]: { json: { project: project() } },
    [`GET ${BASE}/workspace`]: {
      json: { workspace: { has_source: true, origin: 'upload', file_count: 42 } },
    },
    [`GET ${SETUP}/status`]: { json: status },
    [`GET ${SETUP}/recovery`]: { json: { recovery: null } },
    ...extra,
  };
}

/** Everything the wizard sent to one Setup route, in order. */
export function sent(requests: { method: string; path: string; body: unknown }[], method: string, route: string) {
  return requests.filter((r) => r.method === method && r.path === `${SETUP}/${route}`);
}

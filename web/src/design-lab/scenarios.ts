// Sample API answers for the real pages, one scenario per screen state. Fixture mode
// (fixtureMode.ts) serves them in place of the server, so every state can be seen and captured
// without seeding a database or running a model. Shapes follow the server contract, as the
// component tests do.
import type { Account, Project } from '../api/types';
import type { Proposal, Publication } from '../api/focus';
import type { Attempt, RunEvent, Task } from '../api/tasks';
import { AGENT_PROPOSAL, AGENT_PROPOSAL_SAVED, AGENT_SENTENCE, COMMIT_PUBLICATION, OWN_PROPOSAL, PROJECTION, TREE_ID } from './focusFixtures';
import proposedMap from './render/game-proposed.json';
import proposedPicture from './render/game-proposed.webp';

export interface Reply {
  status?: number;
  json?: unknown;
  /** Never answers, to show a loading state. */
  pending?: boolean;
  /** Answers with this bundled file instead of JSON (a picture). */
  asset?: string;
}

export type Routes = Record<string, Reply>;

export interface Scenario {
  group: string;
  /** Where the scenario opens. */
  path: string;
  routes: Routes;
  /** Events the live log receives after it opens. */
  stream?: RunEvent[];
  /** The live stream fails at once, so the log falls back to polling. */
  streamFails?: boolean;
}

const P = '/projects/prj_danube';
const NOW = '2026-11-02T09:40:00Z';

function envelope(code: string, message: string, retryClass = 'terminal') {
  return { error: { code, message, retry_class: retryClass, affected_ids: {}, evidence_refs: [] } };
}

function account(overrides: Partial<Account> = {}): Account {
  return {
    id: 'acc_otto',
    email: 'otto.modder@example.com',
    email_verified: true,
    display_name: 'Archduke Otto',
    github_linked: true,
    has_password: true,
    created_at: '2026-09-28T17:05:00Z',
    ...overrides,
  };
}

function project(overrides: Partial<Project> = {}): Project {
  return {
    id: 'prj_danube',
    name: 'Danubian Federation focus tree',
    visibility: 'public',
    created_at: '2026-10-21T09:05:00Z',
    deleted_at: null,
    delete_after: null,
    role: 'owner',
    ...overrides,
  };
}

const PROJECTS: Project[] = [
  project({ id: 'prj_fc36', name: 'Fractured Crowns: Alternate 1936', visibility: 'private', created_at: '2026-10-29T18:20:00Z' }),
  project(),
  project({ id: 'prj_nordic', name: 'Nordic Union Overhaul', created_at: '2026-10-02T14:40:00Z', role: 'maintainer' }),
  project({
    id: 'prj_pacific',
    name: 'Pacific Theatre naval rework and the 1939 Imperial Japanese Navy refit events',
    visibility: 'private',
    created_at: '2026-09-14T07:12:00Z',
    role: 'contributor',
  }),
  project({ id: 'prj_arab', name: 'الاتحاد العربي', visibility: 'private', created_at: '2026-08-30T20:55:00Z' }),
  project({ id: 'prj_old', name: 'Old test mod (1.12 saves)', visibility: 'private', created_at: '2025-12-03T16:30:00Z', deleted_at: '2026-10-30T08:00:00Z', delete_after: '2026-11-29T08:00:00Z' }),
];

function signedIn(acc: Account = account()): Routes {
  return {
    'GET /auth/me': { json: { account: acc } },
    'GET /game-reference': { json: { available: true, raw_version: '1.19.4.1', dlc_ids: Array.from({ length: 34 }, (_, i) => `dlc${i}`) } },
  };
}

const SIGNED_OUT: Routes = {
  'GET /auth/me': { status: 401, json: envelope('unauthenticated', 'Sign in to continue.', 'requires_authorization') },
};

// ----- Project page and Setup --------------------------------------------------------------

function projectRoutes(workspace: unknown, status: unknown, extra: Routes = {}): Routes {
  return {
    ...signedIn(),
    [`GET ${P}`]: { json: { project: project() } },
    [`GET ${P}/workspace`]: { json: { summary: workspace } },
    [`GET ${P}/setup/status`]: { json: status },
    [`GET ${P}/setup/recovery`]: { json: { recovery: null } },
    [`GET ${P}/github/repositories`]: {
      json: {
        repositories: [
          { full_name: 'otto-modder/danubian-federation', default_branch: 'main', private: false },
          { full_name: 'otto-modder/fractured-crowns', default_branch: 'develop', private: true },
        ],
      },
    },
    ...extra,
  };
}

const UPLOADED = { has_source: true, origin: 'upload', file_count: 1284, ref: 'main', revision: '9c1e4b2', setup_state: 'not_started' };

const SCAN = {
  scan_id: 'scan_1',
  session_id: 'sess_1',
  mode: 'existing',
  incomplete: false,
  receipt_id: 'rcpt_scan',
  workspace_unchanged: true,
  facts: [
    { label: 'Mod name', value: 'Danubian Federation' },
    { label: 'Supported game version', value: '1.19.*' },
    { label: 'Files', value: '1284' },
    { label: 'Focus trees', value: '3' },
    { label: 'Event files', value: '27' },
  ],
  evidence: {
    required: true,
    truncated: false,
    candidates: [
      { reference: 'ev-1', path: 'descriptor.mod', category: 'descriptor', key: 'name', excerpt: 'version="0.4.2"\ntags={\n\t"Alternative History"\n\t"National Focuses"\n}\nname="Danubian Federation"\nsupported_version="1.19.*"', excerpt_sha256: '1'.repeat(64) },
      { reference: 'ev-2', path: 'common/national_focus/danube.txt', category: 'national_focus', key: 'focus_tree', excerpt: 'focus_tree = {\n\tid = danube_focus\n\tcountry = { factor = 0 modifier = { add = 10 tag = AUS } }\n\tfocus = {\n\t\tid = DAN_customs_union\n\t\tcost = 10\n\t}\n}', excerpt_sha256: '2'.repeat(64) },
      { reference: 'ev-3', path: 'localisation/english/danube_l_english.yml', category: 'localisation', key: 'l_english', excerpt: 'l_english:\n DAN_customs_union:0 "The Danubian Customs Union"\n DAN_customs_union_desc:0 "Tariffs between the Danube states must fall."', excerpt_sha256: '3'.repeat(64) },
    ],
  },
};

const VALUES = {
  description: 'An alternate-history focus tree that unites Austria, Hungary and Czechoslovakia in a Danubian Federation.',
  folder_profile: ['common', 'events', 'gfx', 'interface', 'localisation'],
  identity: {
    display_name: 'Danubian Federation',
    project_id: 'danubian_federation',
    author: 'Archduke Otto',
    version: '0.4.2',
    supported_game_version: '1.19.*',
    script_prefix: 'DAN',
    primary_namespace: 'danube',
    default_branch: 'main',
    descriptor_tags: ['Alternative History', 'National Focuses'],
  },
  conventions: { agents_profile: 'standard', localisation_convention: 'english-only', documentation_convention: 'readme' },
};

const ANALYSIS = {
  analysis_id: 'an_1',
  attempts: 1,
  model: 'claude-haiku-5-5',
  source: { revision: '9c1e4b2', manifest_sha256: '4'.repeat(64) },
  summary: 'A mid-sized focus tree mod for Austria with three branches, 27 event files and English localisation only.',
  warnings: ['Two focus icons point to sprites that are not defined in interface/*.gfx.'],
  proposals: [
    { key: 'identity.script_prefix', value: 'DAN', confidence: 0.92, reason: 'Every focus id starts with DAN_.' },
    { key: 'identity.primary_namespace', value: 'danube', confidence: 0.81, reason: 'Events use the danube namespace.' },
  ],
  values: VALUES,
  component_recommendations: [
    { component_id: 'hoi4-agent-tools', recommendation: 'install', reason: 'The mod has focus trees and events the tools can check.' },
  ],
};

const PLAN = {
  plan_id: 'plan_1',
  plan_sha256: 'a'.repeat(64),
  root_identity: 'c'.repeat(64),
  state: 'planned',
  approved: false,
  operations: [
    { path: 'descriptor.mod', action: 'change' },
    { path: 'AGENTS.md', action: 'create' },
    { path: '.hoi4factory/project.json', action: 'create' },
    { path: 'common/national_focus/danube.txt', action: 'preserve' },
    { path: 'README.md', action: 'conflict' },
  ],
  conflicts: [{ path: 'README.md', options: ['keep_existing', 'replace'], selected: null }],
  components: [{ name: 'hoi4-agent-tools', version: '4.6.1' }],
  source: { revision: '9c1e4b2' },
  summary: { create: 2, change: 1, preserve: 1, conflict: 1 },
};

function status(step: string, extra: Record<string, unknown> = {}) {
  return {
    status: {},
    session_open: true,
    step,
    session: { session_id: 'sess_1', mode: 'existing' },
    scan: SCAN,
    analysis: null,
    confirmed_values: null,
    plan: null,
    ...extra,
  };
}

const NO_SETUP = { status: {}, session_open: false, step: 'none', session: null, scan: null, analysis: null, confirmed_values: null, plan: null };

// ----- Tasks -------------------------------------------------------------------------------

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: 'tsk_customs',
    project_id: 'prj_danube',
    objective: 'Add a three-focus branch for the Danubian customs union, with a national spirit and English localisation',
    mode: 'edit',
    allowed_paths: ['common/national_focus', 'common/ideas', 'localisation/english'],
    state: 'ready',
    paused: false,
    pause_reason: null,
    budget_micros: 200_000,
    max_turns: 20,
    model: { profile_id: 'anthropic:claude-haiku-5-5', funding: 'operator_allowance', key_id: null },
    version: 4,
    last_seq: 0,
    spent_micros: 0,
    spend_unknown: false,
    created_at: '2026-11-02T09:12:00Z',
    updated_at: '2026-11-02T09:12:00Z',
    ...overrides,
  };
}

function attempt(overrides: Partial<Attempt> = {}): Attempt {
  return {
    id: 'att_1',
    attempt_no: 1,
    state: 'running',
    grant: {
      id: 'grt_1',
      revocation_epoch: 1,
      revoked: false,
      operation_classes: ['read_file', 'write_file', 'run_read_only_tool'],
      allowed_roots: ['common/national_focus', 'common/ideas', 'localisation/english'],
      expires_at: '2026-11-02T11:15:00Z',
    },
    bundle: { id: 'bun_1', manifest_sha256: '5'.repeat(64) },
    error_code: null,
    outcome: null,
    started_at: '2026-11-02T09:15:00Z',
    finished_at: null,
    version: 2,
    ...overrides,
  };
}

let seq = 0;
function ev(type: string, summary: string, payload: Record<string, unknown> = {}, actor = 'agent'): RunEvent {
  seq += 1;
  return {
    seq,
    event_id: `evt_${seq}`,
    attempt_id: 'att_1',
    type,
    actor,
    summary,
    payload,
    created_at: new Date(Date.parse('2026-11-02T09:15:00Z') + seq * 17_000).toISOString(),
  };
}

function runEvents(): RunEvent[] {
  seq = 0;
  return [
    ev('task_created', 'Task created', {}, 'user'),
    ev('attempt_started', 'Attempt 1 started with a grant for 3 folders', { grant_id: 'grt_1' }, 'host'),
    ev('model_request', 'Asked claude-haiku-5-5 for the next step', { turn: 1 }, 'host'),
    ev('tool_started', 'Reading common/national_focus/danube.txt', { tool: 'read_file', path: 'common/national_focus/danube.txt' }),
    ev('tool_settled', 'Read 412 lines', { tool: 'read_file', bytes: 18234 }),
    ev('tool_started', 'Inspecting the focus tree danube_focus', { tool: 'hoi4.focus_inspect', tree: 'danube_focus' }),
    ev('tool_settled', 'Found 38 focuses in 3 branches, no layout problems', { tool: 'hoi4.focus_inspect', focuses: 38 }),
    ev('model_usage', 'Turn 2 used 6,120 input and 840 output tokens', { input_tokens: 6120, output_tokens: 840, micros: 9_870 }, 'host'),
    ev('tool_started', 'Writing common/ideas/danube_customs.txt', { tool: 'write_file', path: 'common/ideas/danube_customs.txt' }),
  ];
}

const TASKS_LIST: Task[] = [
  task({ id: 'tsk_customs', state: 'running', spent_micros: 41_230, last_seq: 9 }),
  task({ id: 'tsk_events', objective: 'Review the 27 event files for missing localisation keys', mode: 'read_only', allowed_paths: ['events', 'localisation/english'], state: 'in_review', spent_micros: 88_400, budget_micros: 300_000 }),
  task({ id: 'tsk_spirits', objective: 'Rebalance the Habsburg Restoration national spirits', state: 'running', paused: true, pause_reason: 'Paused by Archduke Otto', spent_micros: 12_050 }),
  task({ id: 'tsk_gfx', objective: 'Define the two missing focus icon sprites', state: 'blocked', spent_micros: 200_000, model: { profile_id: 'anthropic:claude-sonnet-5-5', funding: 'user_key', key_id: 'key_claude' } }),
  task({ id: 'tsk_old', objective: 'Rename the DAN_ prefix to DNF_ in every focus', state: 'cancelled', spent_micros: 3_100 }),
];

const PROFILES = {
  profiles: [
    { id: 'anthropic:claude-haiku-5-5', label: 'Claude Haiku 5.5', provider: 'anthropic', default: true, operator_allowance: true, capabilities: {} },
    { id: 'anthropic:claude-sonnet-5-5', label: 'Claude Sonnet 5.5', provider: 'anthropic', default: false, operator_allowance: true, capabilities: {} },
  ],
  user_keys_available: true,
};

const KEYS = {
  available: true,
  keys: [
    { id: 'key_claude', provider: 'anthropic', label: 'My Claude key', base_url: null, key_hint: '7Qx2', created_at: '2026-10-20T10:00:00Z', last_used_at: '2026-11-01T21:14:00Z' },
    { id: 'key_local', provider: 'openai_compatible', label: 'Ollama on the desktop', base_url: 'http://192.168.1.20:11434/v1', key_hint: 'none', created_at: '2026-10-26T19:30:00Z', last_used_at: null },
  ],
};

const USAGE = {
  month: '2026-11',
  verified: true,
  allowance_micros: 1_000_000,
  operator_spent_micros: 141_680,
  remaining_allowance_micros: 858_320,
  unknown_count: 1,
  user_key_spent_micros: 200_000,
};

function taskRoutes(detail: { task: Task; attempts: Attempt[]; pending_effects?: unknown[] }, events: RunEvent[]): Routes {
  return {
    ...signedIn(),
    [`GET ${P}`]: { json: { project: project() } },
    [`GET ${P}/tasks/${detail.task.id}`]: { json: { pending_effects: [], ...detail } },
    [`GET ${P}/tasks/${detail.task.id}/events`]: { json: { events } },
  };
}

function taskScenario(
  overrides: Partial<Task>,
  attempts: Attempt[],
  options: { pending?: unknown[]; events?: RunEvent[]; stream?: RunEvent[]; streamFails?: boolean } = {},
): Scenario {
  const t = task(overrides);
  return {
    group: 'task',
    path: `${P}/tasks/${t.id}`,
    routes: taskRoutes({ task: t, attempts, pending_effects: options.pending ?? [] }, options.events ?? runEvents()),
    stream: options.stream,
    streamFails: options.streamFails,
  };
}

// ----- The focus editor and the change card ------------------------------------------------

const FOCUS_TASK = `${P}/tasks/tsk_customs`;
const TREE = `${FOCUS_TASK}/focus/trees/${TREE_ID}`;
const AGENT_DONE = attempt({
  state: 'succeeded',
  finished_at: '2026-11-02T09:36:00Z',
  outcome: { summary: AGENT_SENTENCE, changed_files: AGENT_PROPOSAL.changedPaths, branch: 'factory/tsk_customs', commit: null, open_questions: [] },
});

/** The routes both pages read: the task, the tree, its picture and the task's proposals. */
function focusRoutes(proposals: Proposal[], attempts: Attempt[], more: Routes = {}, publications: Publication[] = []): Routes {
  const t = task({ state: attempts.some((a) => a.state === 'running') ? 'running' : 'in_review', allowed_paths: ['common/national_focus', 'localisation/english'] });
  return {
    ...taskRoutes({ task: t, attempts }, runEvents()),
    [`GET ${TREE}`]: { json: PROJECTION },
    [`GET ${TREE}/preview.json`]: { json: proposedMap },
    [`GET ${TREE}/preview.png`]: { asset: proposedPicture },
    [`GET ${FOCUS_TASK}/proposals`]: { json: { proposals } },
    [`GET ${FOCUS_TASK}/publications`]: { json: { publications } },
    ...Object.fromEntries(proposals.map((p) => [`GET ${FOCUS_TASK}/proposals/${p.id}`, { json: p }])),
    ...more,
  };
}

function focusScenario(routes: Routes): Scenario {
  return { group: 'focus', path: `${FOCUS_TASK}/focus/${TREE_ID}`, routes };
}

function changeScenario(routes: Routes): Scenario {
  return { group: 'focus', path: `${FOCUS_TASK}/changes/${AGENT_PROPOSAL.id}`, routes };
}

// ----- The scenarios -----------------------------------------------------------------------

export const SCENARIOS: Record<string, Scenario> = {
  'signin': { group: 'auth', path: '/signin', routes: SIGNED_OUT },
  'signin-github-error': { group: 'auth', path: '/signin?error=github_denied', routes: SIGNED_OUT },
  'signup': { group: 'auth', path: '/signup', routes: SIGNED_OUT },
  'forgot-password': { group: 'auth', path: '/forgot-password', routes: SIGNED_OUT },
  'reset-password': { group: 'auth', path: '/reset-password?token=sample', routes: SIGNED_OUT },
  'verify-checking': { group: 'auth', path: '/verify-email?token=sample', routes: { ...SIGNED_OUT, 'POST /auth/verify-email': { pending: true } } },
  'verify-done': { group: 'auth', path: '/verify-email?token=sample', routes: { ...signedIn(), 'POST /auth/verify-email': { json: { account: account() } } } },
  'verify-expired': {
    group: 'auth',
    path: '/verify-email?token=sample',
    routes: { ...signedIn(account({ email_verified: false })), 'POST /auth/verify-email': { status: 400, json: envelope('token_expired', 'The link has expired.') } },
  },
  'connection-error': { group: 'auth', path: '/', routes: { 'GET /auth/me': { status: 503, json: envelope('unavailable', 'Starting up.', 'safe_same_inputs') } } },

  'projects-list': { group: 'projects', path: '/', routes: { ...signedIn(), 'GET /projects': { json: { projects: PROJECTS } } } },
  'projects-empty': { group: 'projects', path: '/', routes: { ...signedIn(), 'GET /projects': { json: { projects: [] } } } },
  'projects-loading': { group: 'projects', path: '/', routes: { ...signedIn(), 'GET /projects': { pending: true } } },
  'projects-error': { group: 'projects', path: '/', routes: { ...signedIn(), 'GET /projects': { status: 500, json: envelope('internal', 'Internal error.', 'safe_same_inputs') } } },
  'projects-unverified': {
    group: 'projects',
    path: '/',
    routes: { ...signedIn(account({ email_verified: false })), 'GET /projects': { json: { projects: PROJECTS.slice(0, 3) } } },
  },
  'not-found': { group: 'projects', path: '/no-such-page', routes: signedIn() },

  'project-empty': { group: 'project', path: P, routes: projectRoutes({ has_source: false }, NO_SETUP) },
  'project-uploaded': { group: 'project', path: P, routes: projectRoutes(UPLOADED, NO_SETUP) },
  'project-read-only': {
    group: 'project',
    path: P,
    routes: { ...projectRoutes(UPLOADED, NO_SETUP), [`GET ${P}`]: { json: { project: project({ role: 'viewer' }) } } },
  },
  'project-pending-deletion': {
    group: 'project',
    path: P,
    routes: { ...projectRoutes(UPLOADED, NO_SETUP), [`GET ${P}`]: { json: { project: project({ deleted_at: '2026-11-01T08:00:00Z', delete_after: '2026-12-01T08:00:00Z' }) } } },
  },
  'project-loading': { group: 'project', path: P, routes: { ...signedIn(), [`GET ${P}`]: { pending: true } } },
  'project-error': { group: 'project', path: P, routes: { ...signedIn(), [`GET ${P}`]: { status: 403, json: envelope('forbidden', 'Not a member.', 'requires_authorization') } } },
  'project-unavailable': {
    group: 'project',
    path: P,
    routes: { ...signedIn(), [`GET ${P}`]: { json: { project: project() } } },
  },

  'setup-evidence': { group: 'setup', path: P, routes: projectRoutes({ ...UPLOADED, setup_state: 'scanned' }, status('scanned')) },
  'setup-analyze': { group: 'setup', path: P, routes: projectRoutes({ ...UPLOADED, setup_state: 'evidence_approved' }, status('evidence_approved')) },
  'setup-confirm': { group: 'setup', path: P, routes: projectRoutes({ ...UPLOADED, setup_state: 'analyzed' }, status('analyzed', { analysis: ANALYSIS })) },
  'setup-plan': {
    group: 'setup',
    path: P,
    routes: projectRoutes({ ...UPLOADED, setup_state: 'planned' }, status('planned', { analysis: ANALYSIS, confirmed_values: VALUES, plan: PLAN }), {
      [`GET ${P}/setup/plan`]: { json: { plan: PLAN } },
    }),
  },
  'setup-apply': {
    group: 'setup',
    path: P,
    routes: projectRoutes({ ...UPLOADED, setup_state: 'approved' }, status('approved', { analysis: ANALYSIS, confirmed_values: VALUES, plan: { ...PLAN, approved: true, state: 'approved', conflicts: [], summary: { ...PLAN.summary, conflict: 0 } } }), {
      [`GET ${P}/setup/plan`]: { json: { plan: { ...PLAN, approved: true, state: 'approved' } } },
    }),
  },
  'setup-applied': {
    group: 'setup',
    path: P,
    routes: projectRoutes({ ...UPLOADED, setup_state: 'applied' }, status('applied', { analysis: ANALYSIS, confirmed_values: VALUES, plan: { ...PLAN, approved: true, state: 'applied' } })),
  },
  'setup-recovery': {
    group: 'setup',
    path: P,
    routes: projectRoutes({ ...UPLOADED, setup_state: 'needs_rollback' }, status('approved', { plan: { ...PLAN, approved: true } }), {
      [`GET ${P}/setup/recovery`]: {
        json: {
          recovery: {
            journal_id: 'j1',
            variant: 'rollback_after_failed_apply',
            allowed_action: 'rollback',
            explanation: 'The apply stopped after changing 2 of 4 files. Rolling back puts every file back exactly as it was.',
            journal_state: 'applying',
            owner_active: false,
          },
        },
      },
    }),
  },

  'tasks-list': {
    group: 'tasks',
    path: `${P}/tasks`,
    routes: { ...signedIn(), [`GET ${P}`]: { json: { project: project() } }, [`GET ${P}/tasks`]: { json: { tasks: TASKS_LIST } }, 'GET /model-profiles': { json: PROFILES }, 'GET /account/api-keys': { json: KEYS } },
  },
  'tasks-empty': {
    group: 'tasks',
    path: `${P}/tasks`,
    routes: { ...signedIn(), [`GET ${P}`]: { json: { project: project() } }, [`GET ${P}/tasks`]: { json: { tasks: [] } }, 'GET /model-profiles': { json: PROFILES }, 'GET /account/api-keys': { json: KEYS } },
  },

  'task-ready': taskScenario({}, [], { events: [] }),
  'task-running': taskScenario({ state: 'running', spent_micros: 41_230, last_seq: 9 }, [attempt()], {
    stream: [
      { seq: 10, event_id: 'evt_10', attempt_id: 'att_1', type: 'tool_settled', actor: 'agent', summary: 'Wrote 46 lines to common/ideas/danube_customs.txt', payload: { bytes: 1840 }, created_at: NOW },
    ],
  }),
  'task-paused': taskScenario(
    { state: 'running', paused: true, pause_reason: 'Paused by Archduke Otto', spent_micros: 41_230, last_seq: 10 },
    [attempt({ state: 'running' })],
    { events: [...runEvents(), ev('task_paused', 'Paused by Archduke Otto. The attempt is suspended after its current step.', {}, 'user'), ev('attempt_suspended', 'Attempt 1 suspended with 1 tool call deferred', { deferred: 1 }, 'host')] },
  ),
  'task-interrupt-requested': taskScenario({ state: 'running', spent_micros: 52_900 }, [attempt({ state: 'interrupt_requested' })]),
  'task-cancelled': taskScenario({ state: 'cancelled', spent_micros: 18_400 }, [attempt({ state: 'cancelled', finished_at: '2026-11-02T09:31:00Z' })]),
  'task-failed': taskScenario({ state: 'blocked', spent_micros: 200_000 }, [
    attempt({ state: 'failed', error_code: 'budget_task_exhausted', finished_at: '2026-11-02T09:38:00Z' }),
  ]),
  'task-unknown-effect': taskScenario({ state: 'blocked', spent_micros: 64_000, spend_unknown: true }, [attempt({ state: 'unknown_effect', finished_at: '2026-11-02T09:33:00Z' })], {
    pending: [
      { operation_id: 'op_7f3a', tool: 'write_file', status: 'unknown' },
      { operation_id: 'op_7f3b', tool: 'hoi4.focus_rewrite', status: 'started' },
    ],
  }),
  'task-in-review': taskScenario({ state: 'in_review', spent_micros: 88_400 }, [
    attempt({ state: 'cancelled', attempt_no: 1, id: 'att_0', finished_at: '2026-11-02T09:20:00Z' }),
    attempt({
      id: 'att_1',
      attempt_no: 2,
      state: 'succeeded',
      finished_at: '2026-11-02T09:36:00Z',
      outcome: {
        summary: 'Added the focuses DAN_customs_union, DAN_common_tariff and DAN_danube_commission, the national spirit DAN_customs_union_idea and their English localisation.',
        changed_files: ['common/national_focus/danube.txt', 'common/ideas/danube_customs.txt', 'localisation/english/danube_l_english.yml'],
        branch: 'factory/tsk_customs-2',
        commit: null,
        open_questions: ['Should the national spirit expire after 365 days or stay until the federation forms?'],
      },
    }),
  ]),
  'task-polling': taskScenario({ state: 'running', spent_micros: 41_230 }, [attempt()], { streamFails: true }),
  'task-with-focus-change': { group: 'task', path: FOCUS_TASK, routes: focusRoutes([AGENT_PROPOSAL], [AGENT_DONE]) },
  'task-loading': { group: 'task', path: `${P}/tasks/tsk_customs`, routes: { ...signedIn(), [`GET ${P}/tasks/tsk_customs`]: { pending: true } } },

  'focus-editor': focusScenario(focusRoutes([AGENT_PROPOSAL_SAVED], [AGENT_DONE])),
  'focus-editor-agent-change': focusScenario(focusRoutes([AGENT_PROPOSAL], [AGENT_DONE])),
  'focus-editor-own-edit': focusScenario(focusRoutes([AGENT_PROPOSAL_SAVED, OWN_PROPOSAL], [AGENT_DONE])),
  'focus-editor-agent-working': focusScenario(focusRoutes([AGENT_PROPOSAL_SAVED], [AGENT_DONE, attempt({ id: 'att_2', attempt_no: 2, state: 'running' })])),
  'focus-editor-check-failed': focusScenario(
    focusRoutes([AGENT_PROPOSAL], [AGENT_DONE], { [`POST ${FOCUS_TASK}/commit`]: { status: 409, json: envelope('commit_blocked', 'The commit check found new problems in the agent\'s changes. Nothing was committed.') } }),
  ),
  'focus-editor-loading': focusScenario({ ...focusRoutes([], [AGENT_DONE]), [`GET ${TREE}`]: { pending: true } }),
  'focus-editor-tools-down': focusScenario({
    ...focusRoutes([], [AGENT_DONE]),
    [`GET ${TREE}`]: { status: 503, json: envelope('tools_unavailable', 'HOI4 Agent Tools are unavailable: the tool server did not start.', 'safe_same_inputs') },
  }),
  'change-card': changeScenario(focusRoutes([AGENT_PROPOSAL], [AGENT_DONE])),
  'change-card-not-checked': changeScenario(
    focusRoutes([AGENT_PROPOSAL], [AGENT_DONE], { [`POST ${FOCUS_TASK}/commit`]: { status: 409, json: envelope('check_incomplete', 'The commit check could not finish. Nothing was committed.') } }),
  ),
  'change-card-check-failed': changeScenario(
    focusRoutes([AGENT_PROPOSAL], [AGENT_DONE], { [`POST ${FOCUS_TASK}/commit`]: { status: 409, json: envelope('commit_blocked', 'The commit check found new problems in the agent\'s changes. Nothing was committed.') } }),
  ),
  'change-card-approved': changeScenario(focusRoutes([AGENT_PROPOSAL_SAVED], [AGENT_DONE], {}, [COMMIT_PUBLICATION])),

  'account': {
    group: 'account',
    path: '/account',
    routes: { ...signedIn(), 'GET /account/api-keys': { json: KEYS }, 'GET /account/usage': { json: USAGE } },
  },
  'account-keys-unavailable': {
    group: 'account',
    path: '/account',
    routes: { ...signedIn(account({ github_linked: false })), 'GET /account/api-keys': { json: { available: false, keys: [] } }, 'GET /account/usage': { json: { ...USAGE, user_key_spent_micros: 0, unknown_count: 0 } } },
  },
  'account-unverified': {
    group: 'account',
    path: '/account',
    routes: {
      ...signedIn(account({ email_verified: false })),
      'GET /account/api-keys': { json: { available: true, keys: [] } },
      'GET /account/usage': { json: { ...USAGE, verified: false, operator_spent_micros: 0, remaining_allowance_micros: 0, unknown_count: 0, user_key_spent_micros: 0 } },
    },
  },
};

export const SCENARIO_GROUPS = ['auth', 'projects', 'project', 'setup', 'tasks', 'task', 'focus', 'account'] as const;

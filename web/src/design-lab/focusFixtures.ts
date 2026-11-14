// Sample data for the focus editor and the change card: a small Swedish industry tree with a
// branch an agent added (four new focuses and one changed cost), in the server's shapes. Nothing
// here is read from a real project.
import type { EngineCheck, Proposal, Publication } from '../api/focus';
import type { Diagnostic, FocusNode, FocusProjection } from '../focus/types';

export const FOCUS_FILE = 'common/national_focus/swe_industry.txt';
export const LOC_FILE = 'localisation/english/swe_focus_l_english.yml';

export interface FocusText {
  title: string;
  description: string;
}

/** Localised texts by focus ID. SWE_arms_export has none: the missing-localisation case. */
export const FOCUS_TEXT: Record<string, FocusText> = {
  SWE_industrial_push: { title: 'Industrial Push', description: 'Sweden has iron and a long peace. Time to turn both into factories.' },
  SWE_steel_mills: { title: 'Expand the Steel Mills', description: 'Bergslagen steel feeds every workshop in the country.' },
  SWE_civilian_factories: { title: 'Civilian Factories', description: 'Private capital builds the next generation of plants.' },
  SWE_ore_exports: { title: 'Ore Exports', description: 'Kiruna ore sells to anyone who pays on time.' },
  SWE_military_reform: { title: 'Military Reform', description: 'The army learns what an industrial state can supply.' },
  SWE_home_defense: { title: 'Home Defense', description: 'Neutrality is only credible behind a strong shield.' },
  SWE_open_trade: { title: 'Open Trade', description: 'Open the ports to every flag and every buyer.' },
  SWE_air_force: { title: 'Build the Air Force', description: 'Saab drawings become squadrons.' },
  SWE_conscription: { title: 'Expand Conscription', description: 'Every valley sends its sons to the colors.' },
  SWE_border_forts: { title: 'Northern Forts', description: 'Concrete in the snow, guns facing east.' },
  SWE_coastal_battery: { title: 'Coastal Batteries', description: 'Guns on every skerry that matters.' },
  SWE_nordic_trade_pact: { title: 'Nordic Trade Pact', description: 'A shared tariff schedule with our neighbors.' },
  SWE_shipping_lanes: { title: 'Safe Shipping Lanes', description: 'Escorted convoys keep the pact alive in wartime.' },
  SWE_customs_union: { title: 'Customs Union', description: 'One border, one tariff, one ledger.' },
  SWE_joint_fleet: { title: 'Joint Merchant Fleet', description: 'Pooled hulls carry pooled cargo.' },
};

interface Seed {
  id: string;
  x: number;
  y: number;
  cost: number;
  line: number;
  prereq?: string[][];
  excl?: string[];
  sprite: string;
  iconOk?: boolean;
  declared?: FocusNode['declared'];
  readOnly?: string;
  pending?: 'added' | 'changed';
}

function make(seed: Seed): FocusNode {
  const text = FOCUS_TEXT[seed.id];
  return {
    id: seed.id,
    label: text?.title ?? seed.id,
    titleKey: seed.id,
    descriptionKey: `${seed.id}_desc`,
    cost: seed.cost,
    icon: { sprite: seed.sprite, resolved: seed.iconOk ?? true },
    x: seed.x,
    y: seed.y,
    declared: seed.declared ?? { mode: 'absolute', x: seed.x, y: seed.y },
    prerequisiteGroups: seed.prereq ?? [],
    mutuallyExclusive: seed.excl ?? [],
    source: { path: FOCUS_FILE, line: seed.line, column: 1 },
    editable: !seed.readOnly,
    notEditableReason: seed.readOnly,
    description: text?.description,
  };
}

// First lines of the four appended blocks in the sample diff (a blank line separates them).
const ADDED_LINES = [182, 191, 200, 209] as const;

export const NODES: FocusNode[] = [
  make({ id: 'SWE_industrial_push', x: 2, y: 0, cost: 10, line: 12, sprite: 'GFX_goal_generic_construction' }),
  make({ id: 'SWE_steel_mills', x: 1, y: 1, cost: 10, line: 26, prereq: [['SWE_industrial_push']], sprite: 'GFX_goal_generic_production' }),
  make({ id: 'SWE_civilian_factories', x: 3, y: 1, cost: 10, line: 40, prereq: [['SWE_industrial_push']], sprite: 'GFX_goal_generic_construct_civ_factory' }),
  make({ id: 'SWE_ore_exports', x: 0, y: 2, cost: 5, line: 54, prereq: [['SWE_steel_mills']], sprite: 'GFX_goal_generic_trade' }),
  make({ id: 'SWE_military_reform', x: 2, y: 2, cost: 10, line: 68, prereq: [['SWE_steel_mills'], ['SWE_civilian_factories']], sprite: 'GFX_goal_generic_army_doctrines' }),
  make({ id: 'SWE_home_defense', x: 4, y: 2, cost: 10, line: 84, prereq: [['SWE_civilian_factories']], excl: ['SWE_open_trade'], sprite: 'GFX_goal_generic_defence' }),
  make({
    id: 'SWE_open_trade',
    x: 6,
    y: 2,
    cost: 7,
    line: 98,
    prereq: [['SWE_civilian_factories']],
    excl: ['SWE_home_defense'],
    sprite: 'GFX_goal_generic_trade',
    pending: 'changed',
  }),
  make({ id: 'SWE_air_force', x: 1, y: 3, cost: 10, line: 112, prereq: [['SWE_military_reform']], sprite: 'GFX_focus_swe_air_force', iconOk: false }),
  make({ id: 'SWE_conscription', x: 3, y: 3, cost: 5, line: 126, prereq: [['SWE_military_reform']], sprite: 'GFX_goal_generic_allies_build_infantry' }),
  make({ id: 'SWE_border_forts', x: 4, y: 4, cost: 10, line: 140, prereq: [['SWE_home_defense', 'SWE_conscription']], sprite: 'GFX_goal_generic_fortify_city' }),
  make({ id: 'SWE_arms_export', x: 5, y: 3, cost: 5, line: 154, prereq: [['SWE_open_trade']], sprite: 'GFX_goal_generic_construct_mil_factory' }),
  make({
    id: 'SWE_coastal_battery',
    x: 3,
    y: 5,
    cost: 10,
    line: 168,
    prereq: [['SWE_border_forts']],
    sprite: 'GFX_goal_generic_occupy_states_coastal',
    declared: { mode: 'relative', x: -1, y: 1, relativeTo: 'SWE_border_forts' },
    readOnly: 'It follows Northern Forts.',
  }),
  make({ id: 'SWE_nordic_trade_pact', x: 7, y: 3, cost: 5, line: ADDED_LINES[0], prereq: [['SWE_open_trade']], sprite: 'GFX_goal_generic_trade', pending: 'added' }),
  make({ id: 'SWE_shipping_lanes', x: 6, y: 4, cost: 5, line: ADDED_LINES[1], prereq: [['SWE_nordic_trade_pact']], sprite: 'GFX_goal_generic_navy_anti_submarine', pending: 'added' }),
  make({ id: 'SWE_customs_union', x: 8, y: 4, cost: 10, line: ADDED_LINES[2], prereq: [['SWE_nordic_trade_pact']], sprite: 'GFX_goal_generic_construct_infrastructure', pending: 'added' }),
  make({
    id: 'SWE_joint_fleet',
    x: 7,
    y: 5,
    cost: 10,
    line: ADDED_LINES[3],
    prereq: [['SWE_shipping_lanes'], ['SWE_customs_union']],
    sprite: 'GFX_goal_generic_navy_doctrines_tactics',
    pending: 'added',
  }),
];

export const DIAGNOSTICS: Diagnostic[] = [
  {
    code: 'focus.icon_missing',
    severity: 'warning',
    message: 'Build the Air Force has no icon, so the game leaves it blank.',
    focusId: 'SWE_air_force',
    source: { path: FOCUS_FILE, line: 114, column: 2 },
  },
  {
    code: 'focus.localisation_missing',
    severity: 'warning',
    message: 'The focus under Open Trade has no title or description yet.',
    focusId: 'SWE_arms_export',
    source: { path: FOCUS_FILE, line: 155, column: 2 },
  },
  {
    code: 'focus.relative_position',
    severity: 'info',
    message: 'Coastal Batteries follows Northern Forts, so it cannot be dragged on its own.',
    focusId: 'SWE_coastal_battery',
    source: { path: FOCUS_FILE, line: 170, column: 2 },
  },
  {
    code: 'layout.line_crossing',
    severity: 'warning',
    message: 'Two lines cross below Nordic Trade Pact.',
    focusId: 'SWE_joint_fleet',
    source: { path: FOCUS_FILE, line: ADDED_LINES[3], column: 1 },
  },
];

export const PROJECTION: FocusProjection = {
  projectionHash: 'sha256:7c1e0d9a52b64f83a0e1c7d94b2f58e6a3d0917be4c2a85f60d1b3e79c4a8f20',
  tool: { name: 'hoi4.focus_inspect', version: '4.11.0' },
  treeId: 'SWE_industry',
  sourcePath: FOCUS_FILE,
  sourceHashes: {
    [FOCUS_FILE]: 'a41f0c3be97d25f6d8031c4ab7e92f50c6d1e8837b4a29f05e3c71d6a8b09c42',
    [LOC_FILE]: '0e9b5d27c1a4f83e6d02b7a95c18f4e3d6a0b92c7e51f8d34a6c0b1e9d7f2a58',
  },
  layoutVerdict: 'revise',
  nodes: NODES,
  diagnostics: DIAGNOSTICS,
  supportedOperations: ['create', 'link', 'move', 'set_cost', 'set_text'],
};

// ----- Proposals ---------------------------------------------------------------------------

function block(node: FocusNode): string[] {
  const out = ['focus = {', `\tid = ${node.id}`, `\ticon = ${node.icon?.sprite ?? ''}`, `\tx = ${node.x}`, `\ty = ${node.y}`];
  for (const group of node.prerequisiteGroups) {
    out.push(`\tprerequisite = { ${group.map((id) => `focus = ${id}`).join(' ')} }`);
  }
  out.push(`\tcost = ${node.cost ?? 0}`, '}');
  return out;
}

const ADDED_IDS = ['SWE_nordic_trade_pact', 'SWE_shipping_lanes', 'SWE_customs_union', 'SWE_joint_fleet'];
const ADDED = NODES.filter((n) => ADDED_IDS.includes(n.id));

function focusDiff(): string {
  const body: string[] = [];
  for (const node of ADDED) {
    body.push('+');
    for (const text of block(node)) body.push(`+${text}`);
  }
  return [
    `--- a/${FOCUS_FILE}`,
    `+++ b/${FOCUS_FILE}`,
    '@@ -101,7 +101,7 @@ focus = {',
    ' \tx = 6',
    ' \ty = 2',
    ' \tprerequisite = { focus = SWE_civilian_factories }',
    ' \tmutually_exclusive = { focus = SWE_home_defense }',
    '-\tcost = 10',
    '+\tcost = 7',
    ' \tcompletion_reward = {',
    `@@ -178,3 +178,${3 + body.length} @@`,
    ' \t\tadd_stability = 0.01',
    ' \t}',
    ' }',
    ...body,
  ].join('\n');
}

function locDiff(): string {
  const rows: string[] = [];
  for (const node of ADDED) {
    const text = FOCUS_TEXT[node.id];
    rows.push(`+ ${node.id}:0 "${text?.title ?? ''}"`, `+ ${node.id}_desc:0 "${text?.description ?? ''}"`);
  }
  return [
    `--- a/${LOC_FILE}`,
    `+++ b/${LOC_FILE}`,
    `@@ -30,2 +30,${2 + rows.length} @@`,
    '  SWE_conscription:0 "Expand Conscription"',
    '  SWE_conscription_desc:0 "Every valley sends its sons to the colors."',
    ...rows,
  ].join('\n');
}

export const TREE_ID = PROJECTION.treeId;
export const AGENT_SENTENCE = 'I added a Nordic trade branch under Open Trade and made Open Trade cheaper.';

const CHECK_OK: EngineCheck = {
  verdict: 'pass',
  engine_version: '4.12.0',
  engine_rev: '73a5d71',
  game_version: '1.16.4',
  baseline_commit: '4d2b9e07a1c35f86d0e49b72a8c1f63d5e07b942',
  new_findings: 0,
  baselined_findings: 14,
  blocking: [],
  unknown: {
    references_resolved: 31,
    references_unresolved: 0,
    references_unknown: 0,
    commands_resolved: 12,
    commands_unresolved: 0,
    commands_unknown: 0,
    unmodelled_script_files: 0,
    vocabulary_available: true,
    game_layer: true,
  },
  elapsed_ms: 840,
};

function proposal(overrides: Partial<Proposal>): Proposal {
  return {
    id: '0b9d2f6e-5a1c-4e37-9c41-7d5a2b8e3f10',
    taskId: 'tsk_customs',
    actorKind: 'native_agent',
    actorAccountId: null,
    attemptId: 'att_1',
    treeId: TREE_ID,
    toolOperation: 'hoi4.focus_rewrite',
    toolVersion: '4.12.0',
    baseProjectionHash: '1f'.repeat(32),
    operations: [],
    requestSha256: '2a'.repeat(32),
    before: { [FOCUS_FILE]: 'a4'.repeat(32), [LOC_FILE]: '0e'.repeat(32) },
    after: { [FOCUS_FILE]: '93'.repeat(32), [LOC_FILE]: 'c7'.repeat(32) },
    changedPaths: [FOCUS_FILE, LOC_FILE],
    changedIds: [...ADDED_IDS, 'SWE_open_trade'],
    summary: {
      added: ADDED_IDS,
      changed: [{ id: 'SWE_open_trade', fields: [{ name: 'cost', before: '10', after: '7' }] }],
      removed: [],
    },
    diffs: [
      { path: FOCUS_FILE, diff: focusDiff() },
      { path: LOC_FILE, diff: locDiff() },
    ],
    proposalHash: '5b'.repeat(32),
    state: 'applied',
    afterProjectionId: null,
    engineReport: CHECK_OK,
    findingDelta: { introduced: 0, worsened: 0, changed: 1, improved: 0, unchanged: 14, resolved: 0 },
    createdAt: '2026-11-02T09:36:00Z',
    updatedAt: '2026-11-02T09:36:00Z',
    ...overrides,
  };
}

/** The agent's branch: four new focuses and one cheaper focus, written but not saved yet. */
export const AGENT_PROPOSAL = proposal({});
export const AGENT_PROPOSAL_SAVED = proposal({ state: 'committed' });

/** A person dragged Ore Exports one row down. */
export const OWN_PROPOSAL = proposal({
  id: '7c4e1a90-2b6d-4f58-8e13-9a0c5d7b2e44',
  actorKind: 'person',
  actorAccountId: 'acc_otto',
  attemptId: null,
  changedPaths: [FOCUS_FILE],
  changedIds: ['SWE_ore_exports'],
  summary: { added: [], changed: [{ id: 'SWE_ore_exports', fields: [{ name: 'y', before: '2', after: '3' }] }], removed: [] },
  diffs: [{ path: FOCUS_FILE, diff: ['--- a/' + FOCUS_FILE, '+++ b/' + FOCUS_FILE, '@@ -56,3 +56,3 @@ focus = {', ' \tx = 0', '-\ty = 2', '+\ty = 3'].join('\n') }],
  engineReport: null,
  findingDelta: null,
});

export const COMMIT_PUBLICATION: Publication = {
  id: 'pub_01',
  kind: 'commit',
  idempotencyKey: 'commit-sample',
  state: 'succeeded',
  intent: { proposals: [{ id: AGENT_PROPOSAL.id }] },
  result: { commit: '8e41c0d7', restore_point: { ref: 'refs/factory/restore/task-0418', oid: '4d2b9e07a1c35f86d0e49b72a8c1f63d5e07b942' } },
  createdAt: '2026-11-02T09:41:00Z',
  settledAt: '2026-11-02T09:41:02Z',
};

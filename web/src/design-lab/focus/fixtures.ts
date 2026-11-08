// Sample data for the focus editor and change card rounds. A small Swedish industry tree, with a
// branch an agent proposed (four new focuses and one changed cost), built on the server's shapes
// (src/focus/types.ts). Nothing here is read from a real project.
import type { ChangeCard, CheckRow, Diagnostic, FocusNode, FocusProjection } from '../../focus/types';

export const GRID_W = 96;
export const GRID_H = 130;

export const FOCUS_FILE = 'common/national_focus/swe_industry.txt';
export const LOC_FILE = 'localisation/english/swe_focus_l_english.yml';
const PROPOSAL = 'prop_01JB7Q4X2M';

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
    pending: seed.pending ? { proposalId: PROPOSAL, actor: 'native_agent', change: seed.pending } : undefined,
  };
}

// First lines of the four appended blocks in the sample diff (a blank line separates them).
const ADDED_LINES = [182, 191, 200, 209] as const;

export const NODES: FocusNode[] = [
  make({ id: 'SWE_industrial_push', x: 2, y: 0, cost: 10, line: 12, sprite: 'GFX_goal_generic_construction' }),
  make({ id: 'SWE_steel_mills', x: 1, y: 1, cost: 10, line: 26, prereq: [['SWE_industrial_push']], sprite: 'GFX_goal_generic_production' }),
  make({ id: 'SWE_civilian_factories', x: 3, y: 1, cost: 10, line: 40, prereq: [['SWE_industrial_push']], sprite: 'GFX_goal_generic_construction' }),
  make({ id: 'SWE_ore_exports', x: 0, y: 2, cost: 5, line: 54, prereq: [['SWE_steel_mills']], sprite: 'GFX_goal_generic_trade' }),
  make({ id: 'SWE_military_reform', x: 2, y: 2, cost: 10, line: 68, prereq: [['SWE_steel_mills'], ['SWE_civilian_factories']], sprite: 'GFX_goal_generic_army_doctrine' }),
  make({ id: 'SWE_home_defense', x: 4, y: 2, cost: 10, line: 84, prereq: [['SWE_civilian_factories']], excl: ['SWE_open_trade'], sprite: 'GFX_goal_generic_defence' }),
  make({
    id: 'SWE_open_trade',
    x: 6,
    y: 2,
    cost: 10,
    line: 98,
    prereq: [['SWE_civilian_factories']],
    excl: ['SWE_home_defense'],
    sprite: 'GFX_goal_generic_trade',
    pending: 'changed',
  }),
  make({ id: 'SWE_air_force', x: 1, y: 3, cost: 10, line: 112, prereq: [['SWE_military_reform']], sprite: 'GFX_focus_swe_air_force', iconOk: false }),
  make({ id: 'SWE_conscription', x: 3, y: 3, cost: 5, line: 126, prereq: [['SWE_military_reform']], sprite: 'GFX_goal_generic_army_manpower' }),
  make({ id: 'SWE_border_forts', x: 4, y: 4, cost: 10, line: 140, prereq: [['SWE_home_defense', 'SWE_conscription']], sprite: 'GFX_goal_generic_fortify' }),
  make({ id: 'SWE_arms_export', x: 5, y: 3, cost: 5, line: 154, prereq: [['SWE_open_trade']], sprite: 'GFX_goal_generic_trade' }),
  make({
    id: 'SWE_coastal_battery',
    x: 3,
    y: 5,
    cost: 10,
    line: 168,
    prereq: [['SWE_border_forts']],
    sprite: 'GFX_goal_generic_navy_coastal',
    declared: { mode: 'relative', x: -1, y: 1, relativeTo: 'SWE_border_forts' },
    readOnly: 'Placed with relative_position_id. Edit it in the source file.',
  }),
  make({ id: 'SWE_nordic_trade_pact', x: 7, y: 3, cost: 5, line: ADDED_LINES[0], prereq: [['SWE_open_trade']], sprite: 'GFX_goal_generic_trade', pending: 'added' }),
  make({ id: 'SWE_shipping_lanes', x: 6, y: 4, cost: 5, line: ADDED_LINES[1], prereq: [['SWE_nordic_trade_pact']], sprite: 'GFX_goal_generic_navy_convoy', pending: 'added' }),
  make({ id: 'SWE_customs_union', x: 8, y: 4, cost: 10, line: ADDED_LINES[2], prereq: [['SWE_nordic_trade_pact']], sprite: 'GFX_goal_generic_trade', pending: 'added' }),
  make({
    id: 'SWE_joint_fleet',
    x: 7,
    y: 5,
    cost: 10,
    line: ADDED_LINES[3],
    prereq: [['SWE_shipping_lanes'], ['SWE_customs_union']],
    sprite: 'GFX_goal_generic_navy_naval_doctrine',
    pending: 'added',
  }),
];

/** The values a changed node had before the proposal, restored when the change is discarded. */
export const PENDING_BEFORE: Record<string, Partial<FocusNode>> = {
  SWE_open_trade: { cost: 6 },
};

export const DIAGNOSTICS: Diagnostic[] = [
  {
    code: 'focus.icon_missing',
    severity: 'warning',
    message: 'Icon GFX_focus_swe_air_force is not defined by the mod or the game.',
    focusId: 'SWE_air_force',
    source: { path: FOCUS_FILE, line: 114, column: 2 },
  },
  {
    code: 'focus.localisation_missing',
    severity: 'warning',
    message: 'No English text for SWE_arms_export. The game would show the key.',
    focusId: 'SWE_arms_export',
    source: { path: FOCUS_FILE, line: 155, column: 2 },
  },
  {
    code: 'focus.relative_position',
    severity: 'info',
    message: 'Placed relative to SWE_border_forts. Moving it here is not supported yet.',
    focusId: 'SWE_coastal_battery',
    source: { path: FOCUS_FILE, line: 170, column: 2 },
  },
  {
    code: 'layout.line_crossing',
    severity: 'warning',
    message: 'Two prerequisite lines cross below the Nordic Trade Pact.',
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
  supportedOperations: ['create', 'link', 'move', 'set_cost'],
};

// ----- Change card ------------------------------------------------------------------------

function block(node: FocusNode): string[] {
  const out = ['focus = {', `\tid = ${node.id}`, `\ticon = ${node.icon?.sprite ?? ''}`, `\tx = ${node.x}`, `\ty = ${node.y}`];
  for (const group of node.prerequisiteGroups) {
    out.push(`\tprerequisite = { ${group.map((id) => `focus = ${id}`).join(' ')} }`);
  }
  out.push(`\tcost = ${node.cost ?? 0}`, '}');
  return out;
}

const ADDED = NODES.filter((n) => n.pending?.change === 'added');

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
    '-\tcost = 6',
    '+\tcost = 10',
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

const CHECKS_OK: CheckRow[] = [
  { name: 'Syntax of the changed files', result: 'passed', detail: 'Parsed 2 files, 0 errors.' },
  { name: 'Reference closure', result: 'passed', detail: 'Every focus, icon and text key this change uses resolves.' },
  { name: 'Lints and override audit', result: 'passed', detail: '0 new findings.' },
  { name: 'Layout', result: 'passed', detail: 'No overlaps. 1 line crossing.' },
  { name: 'Findings baseline', result: 'passed', detail: 'Compared with the baseline taken at import.' },
];

export type CardScenario = 'ready' | 'incomplete' | 'failed' | 'stale' | 'committed';

export function makeCard(scenario: CardScenario): ChangeCard {
  const base: ChangeCard = {
    proposalId: PROPOSAL,
    proposalHash: 'sha256:5be07a31d94c2f68a0c3e19b7d4256f8c1a90e3b6d7f42a58c0e91b3d6f7a204',
    actor: { kind: 'native_agent', name: 'Factory agent' },
    operation: 'hoi4.focus_rewrite',
    summary: {
      added: ADDED.map((n) => n.id),
      changed: [{ id: 'SWE_open_trade', fields: [{ name: 'cost', before: '6', after: '10' }] }],
      removed: [],
    },
    files: [
      {
        path: FOCUS_FILE,
        beforeSha256: 'a41f0c3be97d25f6d8031c4ab7e92f50c6d1e8837b4a29f05e3c71d6a8b09c42',
        afterSha256: '93d6a0b1e7c2584f3a96d1c0e8b7f24a5d3091c6e8f7b24d1a0c95e3b7f6d812',
        unifiedDiff: focusDiff(),
      },
      {
        path: LOC_FILE,
        beforeSha256: '0e9b5d27c1a4f83e6d02b7a95c18f4e3d6a0b92c7e51f8d34a6c0b1e9d7f2a58',
        afterSha256: 'c7f20a95e1d3b846a02c9f17d5e83b60a4c1d7f92e05b38a6d4c1f0e97b2a835',
        unifiedDiff: locDiff(),
      },
    ],
    sourceLinks: [
      ...ADDED.map((n) => ({ focusId: n.id, source: n.source })),
      { focusId: 'SWE_open_trade', source: { path: FOCUS_FILE, line: 105, column: 2 } },
    ],
    findingDelta: { introduced: 0, worsened: 0, changed: 1, improved: 0, unchanged: 14, resolved: 0 },
    closure: { verdict: 'pass', resolved: 31, unresolved: 0, unknown: 0 },
    layoutVerdict: 'pass',
    checks: CHECKS_OK,
    limits: [
      'Source render, not a game check. The game was not started.',
      'Conditional availability and AI weights were not evaluated.',
      'Icons from the game files are shown as placeholders.',
    ],
    restorePoint: { ref: 'refs/factory/restore/task-0418', oid: '4d2b9e07a1c35f86d0e49b72a8c1f63d5e07b942' },
    state: 'pending',
    publication: { canPush: false, reason: 'No publication grant yet. The project owner has to allow pushing to GitHub.' },
  };
  if (scenario === 'incomplete') {
    return {
      ...base,
      findingDelta: { introduced: 0, worsened: 0, changed: 1, improved: 0, unchanged: 14, resolved: 0 },
      closure: { verdict: 'incomplete', resolved: 28, unresolved: 0, unknown: 3 },
      layoutVerdict: 'revise',
      checks: [
        CHECKS_OK[0] as CheckRow,
        { name: 'Reference closure', result: 'unavailable', detail: '3 references could not be resolved because the game reference was not loaded for icons.' },
        { name: 'Lints and override audit', result: 'skipped', detail: 'Skipped because reference closure is incomplete.' },
        { name: 'Layout', result: 'passed', detail: 'No overlaps. 2 line crossings.' },
        { name: 'Findings baseline', result: 'skipped', detail: 'No baseline exists for this project yet.' },
      ],
    };
  }
  if (scenario === 'failed') {
    return {
      ...base,
      findingDelta: { introduced: 1, worsened: 0, changed: 1, improved: 0, unchanged: 14, resolved: 0 },
      checks: [
        CHECKS_OK[0] as CheckRow,
        CHECKS_OK[1] as CheckRow,
        { name: 'Lints and override audit', result: 'failed', detail: '1 new finding: the icon of SWE_joint_fleet is not defined.' },
        CHECKS_OK[3] as CheckRow,
        CHECKS_OK[4] as CheckRow,
      ],
    };
  }
  if (scenario === 'stale') return { ...base, state: 'stale' };
  if (scenario === 'committed') return { ...base, state: 'committed' };
  return base;
}

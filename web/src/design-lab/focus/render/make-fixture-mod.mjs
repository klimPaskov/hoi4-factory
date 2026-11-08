#!/usr/bin/env node
// Writes the design lab's focus fixture (the Swedish industry tree of ../fixtures.ts) as a real mod,
// in two states: `base` (before the agent's proposal) and `proposed` (with its branch and changed
// cost), so the lab can show what HOI4 Agent Tools' game view draws for it (D-50).
//
//   node web/src/design-lab/focus/render/make-fixture-mod.mjs <out-dir>
//
// Original text, no game content. Icons name vanilla sprites, which the render resolves from the
// installed game. GFX_focus_swe_air_force is undefined on purpose (the missing-icon case), and
// SWE_arms_export has no localisation (the missing-text case).
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const out = process.argv[2];
if (!out) {
  console.error('usage: make-fixture-mod.mjs <out-dir>');
  process.exit(2);
}

const BASE = [
  ['SWE_industrial_push', 2, 0, 10, [], [], 'GFX_goal_generic_construction'],
  ['SWE_steel_mills', 1, 1, 10, [['SWE_industrial_push']], [], 'GFX_goal_generic_production'],
  ['SWE_civilian_factories', 3, 1, 10, [['SWE_industrial_push']], [], 'GFX_goal_generic_construct_civ_factory'],
  ['SWE_ore_exports', 0, 2, 5, [['SWE_steel_mills']], [], 'GFX_goal_generic_trade'],
  ['SWE_military_reform', 2, 2, 10, [['SWE_steel_mills'], ['SWE_civilian_factories']], [], 'GFX_goal_generic_army_doctrines'],
  ['SWE_home_defense', 4, 2, 10, [['SWE_civilian_factories']], ['SWE_open_trade'], 'GFX_goal_generic_defence'],
  ['SWE_open_trade', 6, 2, 10, [['SWE_civilian_factories']], ['SWE_home_defense'], 'GFX_goal_generic_trade'],
  ['SWE_air_force', 1, 3, 10, [['SWE_military_reform']], [], 'GFX_focus_swe_air_force'],
  ['SWE_conscription', 3, 3, 5, [['SWE_military_reform']], [], 'GFX_goal_generic_allies_build_infantry'],
  ['SWE_border_forts', 4, 4, 10, [['SWE_home_defense', 'SWE_conscription']], [], 'GFX_goal_generic_fortify_city'],
  ['SWE_arms_export', 5, 3, 5, [['SWE_open_trade']], [], 'GFX_goal_generic_construct_mil_factory'],
];
const RELATIVE = ['SWE_coastal_battery', -1, 1, 'SWE_border_forts', 10, [['SWE_border_forts']], 'GFX_goal_generic_occupy_states_coastal'];
const AGENT = [
  ['SWE_nordic_trade_pact', 7, 3, 5, [['SWE_open_trade']], [], 'GFX_goal_generic_trade'],
  ['SWE_shipping_lanes', 6, 4, 5, [['SWE_nordic_trade_pact']], [], 'GFX_goal_generic_navy_anti_submarine'],
  ['SWE_customs_union', 8, 4, 10, [['SWE_nordic_trade_pact']], [], 'GFX_goal_generic_construct_infrastructure'],
  ['SWE_joint_fleet', 7, 5, 10, [['SWE_shipping_lanes'], ['SWE_customs_union']], [], 'GFX_goal_generic_navy_doctrines_tactics'],
];
const TEXT = {
  SWE_industrial_push: ['Industrial Push', 'Sweden has iron and a long peace. Time to turn both into factories.'],
  SWE_steel_mills: ['Expand the Steel Mills', 'Bergslagen steel feeds every workshop in the country.'],
  SWE_civilian_factories: ['Civilian Factories', 'Private capital builds the next generation of plants.'],
  SWE_ore_exports: ['Ore Exports', 'Kiruna ore sells to anyone who pays on time.'],
  SWE_military_reform: ['Military Reform', 'The army learns what an industrial state can supply.'],
  SWE_home_defense: ['Home Defense', 'Neutrality is only credible behind a strong shield.'],
  SWE_open_trade: ['Open Trade', 'Open the ports to every flag and every buyer.'],
  SWE_air_force: ['Build the Air Force', 'Saab drawings become squadrons.'],
  SWE_conscription: ['Expand Conscription', 'Every valley sends its sons to the colors.'],
  SWE_border_forts: ['Northern Forts', 'Concrete in the snow, guns facing east.'],
  SWE_coastal_battery: ['Coastal Batteries', 'Guns on every skerry that matters.'],
  SWE_nordic_trade_pact: ['Nordic Trade Pact', 'A shared tariff schedule with our neighbors.'],
  SWE_shipping_lanes: ['Safe Shipping Lanes', 'Escorted convoys keep the pact alive in wartime.'],
  SWE_customs_union: ['Customs Union', 'One border, one tariff, one ledger.'],
  SWE_joint_fleet: ['Joint Merchant Fleet', 'Pooled hulls carry pooled cargo.'],
};

function focus([id, x, y, cost, prereq, excl, icon]) {
  const lines = [`\tfocus = {`, `\t\tid = ${id}`, `\t\ticon = ${icon}`];
  for (const group of prereq) lines.push(`\t\tprerequisite = { ${group.map((p) => `focus = ${p}`).join(' ')} }`);
  if (excl.length) lines.push(`\t\tmutually_exclusive = { ${excl.map((p) => `focus = ${p}`).join(' ')} }`);
  lines.push(`\t\tx = ${x}`, `\t\ty = ${y}`, `\t\tcost = ${cost}`, `\t\tsearch_filters = { FOCUS_FILTER_INDUSTRY }`);
  lines.push(`\t\tai_will_do = { factor = 1 }`, `\t\tcompletion_reward = { add_political_power = 25 }`, `\t}`, '');
  return lines.join('\n');
}

function relative([id, x, y, anchor, cost, prereq, icon]) {
  return [
    `\tfocus = {`,
    `\t\tid = ${id}`,
    `\t\ticon = ${icon}`,
    ...prereq.map((g) => `\t\tprerequisite = { ${g.map((p) => `focus = ${p}`).join(' ')} }`),
    `\t\trelative_position_id = ${anchor}`,
    `\t\tx = ${x}`,
    `\t\ty = ${y}`,
    `\t\tcost = ${cost}`,
    `\t\tsearch_filters = { FOCUS_FILTER_INDUSTRY }`,
    `\t\tai_will_do = { factor = 1 }`,
    `\t\tcompletion_reward = { add_political_power = 25 }`,
    `\t}`,
    '',
  ].join('\n');
}

function write(dir, proposed) {
  const base = BASE.map((f) => (proposed && f[0] === 'SWE_open_trade' ? [...f.slice(0, 3), 7, ...f.slice(4)] : f));
  const all = proposed ? [...base, ...AGENT] : base;
  const body = [
    'focus_tree = {',
    '\tid = SWE_industry',
    '\tcountry = { factor = 0 modifier = { add = 10 tag = SWE } }',
    '\tdefault = no',
    '',
    ...all.slice(0, BASE.length).map(focus),
    relative(RELATIVE),
    ...all.slice(BASE.length).map(focus),
    '}',
    '',
  ].join('\n');
  mkdirSync(join(dir, 'common/national_focus'), { recursive: true });
  mkdirSync(join(dir, 'localisation/english'), { recursive: true });
  writeFileSync(join(dir, 'descriptor.mod'), 'name="Design lab fixture"\nsupported_version="1.19.*"\n');
  writeFileSync(join(dir, 'common/national_focus/swe_industry.txt'), body);
  const ids = [...all.map((f) => f[0]), RELATIVE[0]].filter((id) => id !== 'SWE_arms_export');
  const loc = ['﻿l_english:', ...ids.flatMap((id) => [` ${id}:0 "${TEXT[id][0]}"`, ` ${id}_desc:0 "${TEXT[id][1]}"`]), ''];
  writeFileSync(join(dir, 'localisation/english/swe_focus_l_english.yml'), loc.join('\n'));
}

write(join(out, 'base'), false);
write(join(out, 'proposed'), true);
console.log('wrote', join(out, 'base'), 'and', join(out, 'proposed'));

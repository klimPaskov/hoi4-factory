// EXP-02 probe harness. Runs INSIDE the pinned hoi4-agent-tools Linux image.
// Usage: node /probe/harness/run.mjs /probe/cases.json /out [caseIdFilter]
// For every case: fresh workspace copy of one fixture, start the real MCP stdio
// server (dist/bin/stdio.js), focus_inspect, edit the imported plan as the case
// says, focus_rewrite, then capture the resulting bytes and every file change.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { StdioMcpClient } from './mcp-client.mjs';

const [casesPath, outRoot, filter] = process.argv.slice(2);
const probeRoot = path.dirname(casesPath);
const { cases } = JSON.parse(fs.readFileSync(casesPath, 'utf8'));
const SERVER = '/app/dist/bin/stdio.js';
const WORK = '/work';

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

function listFiles(root) {
  const out = {};
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else out[path.relative(root, full)] = sha256(fs.readFileSync(full));
    }
  };
  if (fs.existsSync(root)) walk(root);
  return out;
}

function toolPayload(response) {
  if (response.error) return { protocolError: response.error };
  const result = response.result ?? {};
  let text;
  for (const item of result.content ?? []) if (item.type === 'text') text = (text ?? '') + item.text;
  let parsedText;
  try {
    parsedText = text === undefined ? undefined : JSON.parse(text);
  } catch {
    parsedText = undefined;
  }
  return {
    isError: result.isError === true,
    structured: result.structuredContent,
    text: parsedText === undefined ? text : undefined,
    textJson: parsedText,
    links: (result.content ?? []).filter((item) => item.type === 'resource_link'),
  };
}

function findErrorCode(payload) {
  const candidates = [payload?.structured, payload?.textJson];
  for (const c of candidates) {
    if (!c) continue;
    if (c.error?.code) return c.error.code;
    if (c.code && payload.isError) return c.code;
    if (Array.isArray(c.errors) && c.errors[0]?.code) return c.errors[0].code;
  }
  if (payload?.protocolError) return `JSONRPC_${payload.protocolError.code}`;
  if (payload?.isError) return 'TOOL_ERROR_UNCODED';
  return null;
}

async function readJsonResource(client, uri) {
  const response = await client.request('resources/read', { uri });
  const contents = response.result?.contents ?? [];
  for (const item of contents) {
    if (item.text !== undefined) {
      try {
        return JSON.parse(item.text);
      } catch {
        return undefined;
      }
    }
    if (item.blob !== undefined) {
      try {
        return JSON.parse(Buffer.from(item.blob, 'base64').toString('utf8'));
      } catch {
        return undefined;
      }
    }
  }
  return undefined;
}

function applyOp(plan, op) {
  const next = structuredClone(plan);
  const focuses = next.focuses;
  const pick = () => {
    const hits = focuses.filter((f) => f.id === op.focusId);
    const hit = hits[op.occurrence ?? 0];
    if (hit === undefined) throw new Error(`PLAN_FOCUS_NOT_FOUND:${op.focusId}`);
    return hit;
  };
  switch (op.type) {
    case 'noop':
      return next;
    case 'cost':
      pick().cost = op.value;
      return next;
    case 'move': {
      const f = pick();
      if (op.toFixed) f.position = { mode: 'fixed', x: op.x, y: op.y, pinned: false };
      else f.position = { ...f.position, x: op.x, y: op.y };
      return next;
    }
    case 'link': {
      const f = pick();
      f.prerequisites.groups.push({ operator: 'or', focusIds: op.prerequisites, rawPassthrough: [] });
      return next;
    }
    case 'raw_edit': {
      const f = pick();
      const entry = f.rawPassthrough.find((e) => e.key === op.key);
      if (entry === undefined) throw new Error(`PLAN_RAW_ENTRY_NOT_FOUND:${op.key}`);
      entry.text = op.text;
      return next;
    }
    case 'create': {
      const spec = op.focus;
      const position =
        spec.relative_position_id === undefined
          ? { mode: 'fixed', x: Number(spec.x), y: Number(spec.y), pinned: false }
          : { mode: 'relative', x: Number(spec.x), y: Number(spec.y), relativeTo: spec.relative_position_id, pinned: false };
      focuses.push({
        id: spec.id,
        label: spec.id,
        prerequisites: {
          operator: 'and',
          groups: (spec.prerequisites ?? []).map((ids) => ({ operator: 'or', focusIds: ids, rawPassthrough: [] })),
        },
        mutuallyExclusive: [],
        routeLocks: [],
        position,
        visibility: 'normal',
        convergence: false,
        sharedSupport: false,
        icons: [{ kind: 'static', sprite: 'GFX_exp_icon_created' }],
        localisation: { titleKey: spec.id, descriptionKey: `${spec.id}_desc` },
        ai: { raw: { text: '{ factor = 1 }', referencedFocusIds: [] }, majorRoute: false, strategyIds: [] },
        filters: [],
        links: [],
        cost: Number(spec.cost),
        completionReward: { text: '{ add_political_power = 1 }', referencedFocusIds: [] },
        rawPassthrough: [],
      });
      return next;
    }
    default:
      throw new Error(`unknown op ${op.type}`);
  }
}

function stripSourceLocations(value) {
  // Plans from inspection carry sourceLocation fields; the rewrite schema may not accept them.
  if (Array.isArray(value)) return value.map(stripSourceLocations);
  if (value && typeof value === 'object') {
    const out = {};
    for (const [k, v] of Object.entries(value)) if (k !== 'sourceLocation') out[k] = stripSourceLocations(v);
    return out;
  }
  return value;
}

async function runCase(testCase) {
  const caseDir = path.join(WORK, testCase.id);
  fs.rmSync(caseDir, { recursive: true, force: true });
  const modRoot = path.join(caseDir, 'mods', 'exp02mod');
  const focusDir = path.join(modRoot, 'common', 'national_focus');
  fs.mkdirSync(focusDir, { recursive: true });
  fs.mkdirSync(path.join(caseDir, 'state'), { recursive: true, mode: 0o700 });
  fs.mkdirSync(path.join(caseDir, 'storage'), { recursive: true, mode: 0o700 });
  fs.chmodSync(path.join(caseDir, 'state'), 0o700);
  fs.chmodSync(path.join(caseDir, 'storage'), 0o700);
  const fixtureBytes = fs.readFileSync(path.join(probeRoot, 'fixtures', testCase.fixture));
  const relativePath = `common/national_focus/${testCase.fixture}`;
  const target = path.join(modRoot, relativePath);
  fs.writeFileSync(target, fixtureBytes);
  const config = {
    version: 1,
    serverStateRoot: path.join(caseDir, 'state'),
    workspaceStorageRoot: path.join(caseDir, 'storage'),
    workspaces: [{ id: 'exp02', name: 'EXP-02 probe', root: modRoot, kind: 'mod' }],
  };
  const configPath = path.join(caseDir, 'config.json');
  fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
  fs.cpSync(path.join(probeRoot, 'fixture-support'), modRoot, { recursive: true });
  const beforeFiles = listFiles(modRoot);

  const record = { caseId: testCase.id, fixture: testCase.fixture, op: testCase.op, relativePath };
  const client = new StdioMcpClient('node', [SERVER], {
    cwd: '/app',
    env: { PATH: process.env.PATH, HOME: path.join(caseDir, 'home'), HOI4_AGENT_CONFIG: configPath },
  });
  try {
    const init = await client.initialize();
    record.server = init.result?.serverInfo ?? init.error;
    record.protocolVersion = init.result?.protocolVersion;
    const inspectArgs = { workspaceId: 'exp02', relativePath, treeId: testCase.tree };
    const inspectResponse = await client.request('tools/call', { name: 'hoi4.focus_inspect', arguments: inspectArgs });
    const inspect = toolPayload(inspectResponse);
    record.inspect = {
      args: inspectArgs,
      isError: inspect.isError,
      errorCode: findErrorCode(inspect),
      structured: inspect.structured,
      text: inspect.text,
      links: inspect.links.map(({ uri, name, mimeType }) => ({ uri, name, mimeType })),
    };
    let plan;
    for (const link of inspect.links) {
      if (!(link.mimeType ?? '').includes('json') && !(link.name ?? '').endsWith('.json')) continue;
      const body = await readJsonResource(client, link.uri);
      const found = body?.plans?.find((p) => p.id === testCase.tree);
      if (found) {
        plan = found;
        record.inspect.planArtifact = link.uri;
        break;
      }
    }
    record.inspect.planFound = plan !== undefined;
    if (plan !== undefined) {
      fs.mkdirSync(path.join(outRoot, testCase.id), { recursive: true });
      fs.writeFileSync(path.join(outRoot, testCase.id, 'imported-plan.json'), JSON.stringify(plan, null, 2));
    }
    record.inspect.planFocusIds = plan?.focuses?.map((f) => f.id);
    record.inspect.importedSharedFocusIds = plan?.sharedFocusIds;
    if (testCase.op.type !== 'inspect_only') {
      if (plan === undefined) {
        record.rewrite = { skipped: true, errorCode: 'PROBE_NO_IMPORTED_PLAN', execution: null };
      } else {
        let targetPlan;
        try {
          targetPlan = stripSourceLocations(applyOp(plan, testCase.op));
        } catch (error) {
          record.rewrite = { skipped: true, errorCode: String(error.message).split(':')[0], detail: error.message, execution: null };
        }
        if (targetPlan !== undefined && testCase.op.external) {
          // Simulate an external edit that lands after inspection and before the rewrite.
          const current = fs.readFileSync(target);
          const find = Buffer.from(testCase.op.external.find, 'utf8');
          const at = current.indexOf(find);
          if (at < 0) throw new Error('PROBE_EXTERNAL_EDIT_ANCHOR_MISSING');
          const between = Buffer.concat([
            current.subarray(0, at),
            Buffer.from(testCase.op.external.replace, 'utf8'),
            current.subarray(at + find.length),
          ]);
          fs.writeFileSync(target, between);
          record.externalEdit = { beforeSha256: sha256(current), betweenSha256: sha256(between) };
          fs.mkdirSync(path.join(outRoot, testCase.id), { recursive: true });
          fs.writeFileSync(path.join(outRoot, testCase.id, 'between.bin'), between);
        }
        if (targetPlan !== undefined) {
          const rewriteArgs = { workspaceId: 'exp02', relativePath, treeId: testCase.tree, plan: targetPlan };
          const rewriteResponse = await client.request('tools/call', { name: 'hoi4.focus_rewrite', arguments: rewriteArgs });
          const rewrite = toolPayload(rewriteResponse);
          record.rewrite = {
            isError: rewrite.isError,
            errorCode: findErrorCode(rewrite),
            execution: rewrite.structured?.data?.execution ?? rewrite.structured?.execution ?? null,
            structured: rewrite.structured,
            text: rewrite.text,
            protocolError: rewrite.protocolError,
          };
        }
      }
    }
  } catch (error) {
    record.harnessError = String(error?.stack ?? error);
  } finally {
    record.serverExit = await client.close();
    record.serverStderr = client.stderr.slice(0, 20000);
  }
  const afterBytes = fs.readFileSync(target);
  const afterFiles = listFiles(modRoot);
  record.extraFilesChanged = Object.keys({ ...beforeFiles, ...afterFiles })
    .filter((f) => f !== relativePath && beforeFiles[f] !== afterFiles[f])
    .sort();
  record.inputSha256 = sha256(fixtureBytes);
  record.outputSha256 = sha256(afterBytes);
  const outDir = path.join(outRoot, testCase.id);
  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(path.join(outDir, 'after.bin'), afterBytes);
  for (const extra of record.extraFilesChanged) {
    const full = path.join(modRoot, extra);
    if (fs.existsSync(full)) {
      const dest = path.join(outDir, 'extra', extra);
      fs.mkdirSync(path.dirname(dest), { recursive: true });
      fs.copyFileSync(full, dest);
    }
  }
  fs.writeFileSync(path.join(outDir, 'run.json'), JSON.stringify(record, null, 2));
  return record;
}

const summary = [];
for (const testCase of cases) {
  if (filter && !testCase.id.includes(filter)) continue;
  const r = await runCase(testCase);
  summary.push({
    id: r.caseId,
    inspectError: r.inspect?.errorCode ?? null,
    planFound: r.inspect?.planFound ?? false,
    rewriteError: r.rewrite?.errorCode ?? null,
    execution: r.rewrite?.execution ?? null,
    changed: r.inputSha256 !== r.outputSha256,
    extra: r.extraFilesChanged,
    harnessError: r.harnessError ? r.harnessError.split('\n')[0] : undefined,
  });
  process.stdout.write(`${JSON.stringify(summary.at(-1))}\n`);
}
fs.writeFileSync(path.join(outRoot, 'summary.json'), JSON.stringify(summary, null, 2));

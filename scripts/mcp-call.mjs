#!/usr/bin/env node
// Minimal MCP stdio client with no dependencies, for checks against a real HOI4 Agent Tools
// server (T-ENG-01 in the image, and exploring tool input and output).
//
//   node scripts/mcp-call.mjs <entry.js> <calls.json>
//
// <calls.json> is a JSON array of {"name": "...", "arguments": {...}}. The server starts in the
// current directory (the mod). Each result is printed as one JSON line:
// {"name", "isError", "structuredContent", "content"}. Exit code 0 when every call returned a
// result (an isError result included), 2 on a protocol failure.
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';

const [entry, callsPath] = process.argv.slice(2);
if (!entry || !callsPath) {
  console.error('usage: node scripts/mcp-call.mjs <entry.js> <calls.json>');
  process.exit(2);
}
const calls = JSON.parse(readFileSync(callsPath, 'utf8'));
const child = spawn(process.execPath, [entry], { stdio: ['pipe', 'pipe', 'inherit'] });
let buffer = '';
let nextId = 1;
const pending = new Map();
child.stdout.setEncoding('utf8');
child.stdout.on('data', (chunk) => {
  buffer += chunk;
  let nl;
  while ((nl = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, nl).trim();
    buffer = buffer.slice(nl + 1);
    if (!line) continue;
    const msg = JSON.parse(line);
    if (msg.id !== undefined && pending.has(msg.id)) {
      pending.get(msg.id)(msg);
      pending.delete(msg.id);
    } else if (msg.id !== undefined && msg.method) {
      // A server request (roots, sampling, elicitation): refuse it.
      child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, error: { code: -32601, message: 'not supported' } }) + '\n');
    }
  }
});
child.on('exit', (code) => {
  for (const resolve of pending.values()) resolve({ error: { message: `server exited (${code})` } });
});
function request(method, params) {
  const id = nextId++;
  return new Promise((resolve) => {
    pending.set(id, resolve);
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
  });
}
const init = await request('initialize', {
  protocolVersion: '2025-06-18',
  capabilities: {},
  clientInfo: { name: 'hoi4-factory-mcp-call', version: '1' },
});
if (init.error) {
  console.error(JSON.stringify(init.error));
  process.exit(2);
}
child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
let failed = false;
const results = [];
for (const call of calls) {
  if (call.readArtifactOf !== undefined) {
    // {"readArtifactOf": <index of an earlier call>}: read that result's first artifact.
    const uri = results[call.readArtifactOf]?.structuredContent?.artifacts?.[0]?.uri;
    const res = await request('resources/read', { uri });
    if (res.error) failed = true;
    console.log(JSON.stringify({ resource: uri, error: res.error, contents: res.result?.contents }));
    results.push(null);
    continue;
  }
  const res = await request('tools/call', { name: call.name, arguments: call.arguments ?? {} });
  results.push(res.result ?? null);
  if (res.error) {
    failed = true;
    console.log(JSON.stringify({ name: call.name, protocolError: res.error }));
    continue;
  }
  const r = res.result;
  console.log(JSON.stringify({ name: call.name, isError: r.isError === true, structuredContent: r.structuredContent, content: r.content }));
}
child.stdin.end();
child.kill();
process.exit(failed ? 2 : 0);

#!/usr/bin/env node
// Captures the design lab's fixture scenarios as full-page PNGs, for the before and after record
// in docs/design (ADR-025). Development only. It needs the dev server running
// (web/scripts/docker-npm.sh run dev), Node 22 or newer on the host and a local Chrome or Edge.
// It uses only Node built-ins and the browser's DevTools protocol, so nothing is installed.
//
//   node web/scripts/lab-screenshots.mjs <outDir> [--theme light|dark] [--widths 1280,390]
//        [--base http://127.0.0.1:5173] [scenario ...]
//
// With no scenario names it captures every scenario in web/src/design-lab/scenarios.ts.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const at = args.indexOf(`--${name}`);
  if (at === -1) return fallback;
  const [value] = args.splice(at, 2).slice(1);
  return value;
};
const theme = option('theme', 'light');
const widths = option('widths', '1280,390').split(',').map(Number);
const base = option('base', 'http://127.0.0.1:5173');
const [outDir, ...wanted] = args;
if (!outDir) {
  console.error('usage: lab-screenshots.mjs <outDir> [--theme light|dark] [--widths 1280,390] [scenario ...]');
  process.exit(2);
}

// Scenario names are the two-space-indented quoted keys of SCENARIOS.
const source = readFileSync(join(here, '..', 'src', 'design-lab', 'scenarios.ts'), 'utf8');
const all = [...source.matchAll(/^ {2}'([a-z0-9-]+)': /gm)].map((m) => m[1]);
const names = wanted.length > 0 ? wanted : all;
const unknown = names.filter((n) => !all.includes(n));
if (unknown.length > 0) {
  console.error(`unknown scenarios: ${unknown.join(', ')}`);
  process.exit(2);
}

function findBrowser() {
  const candidates = [
    process.env.CHROME_PATH,
    'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Google/Chrome/Application/chrome.exe',
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
  ];
  const found = candidates.find((path) => path && existsSync(path));
  if (!found) throw new Error('No Chrome or Edge found. Set CHROME_PATH.');
  return found;
}

function launch(binary, profile) {
  return new Promise((resolveLaunch, reject) => {
    const child = spawn(binary, [
      '--headless=new',
      '--disable-gpu',
      '--hide-scrollbars',
      '--no-first-run',
      '--no-default-browser-check',
      '--remote-debugging-port=0',
      `--user-data-dir=${profile}`,
      'about:blank',
    ]);
    let buffer = '';
    child.stderr.on('data', (chunk) => {
      buffer += chunk;
      const match = buffer.match(/DevTools listening on (ws:\/\/[^\s]+)/);
      if (match) resolveLaunch({ child, endpoint: match[1] });
    });
    child.on('exit', (code) => reject(new Error(`browser exited with ${code}: ${buffer}`)));
  });
}

function connect(url) {
  return new Promise((resolveConnect, reject) => {
    const socket = new WebSocket(url);
    let id = 0;
    const waiting = new Map();
    const listeners = new Map();
    socket.addEventListener('message', (message) => {
      const data = JSON.parse(String(message.data));
      if (data.id && waiting.has(data.id)) {
        const { ok, fail } = waiting.get(data.id);
        waiting.delete(data.id);
        if (data.error) fail(new Error(data.error.message));
        else ok(data.result);
      } else if (data.method) {
        for (const listener of listeners.get(data.method) ?? []) listener(data.params);
      }
    });
    socket.addEventListener('error', reject);
    socket.addEventListener('open', () =>
      resolveConnect({
        send(method, params = {}) {
          id += 1;
          socket.send(JSON.stringify({ id, method, params }));
          return new Promise((ok, fail) => waiting.set(id, { ok, fail }));
        },
        once(method) {
          return new Promise((ok) => {
            const list = listeners.get(method) ?? [];
            const listener = (params) => {
              listeners.set(method, (listeners.get(method) ?? []).filter((l) => l !== listener));
              ok(params);
            };
            listeners.set(method, [...list, listener]);
          });
        },
        close: () => socket.close(),
      }),
    );
  });
}

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

const profile = mkdtempSync(join(tmpdir(), 'hf-shots-'));
const { child, endpoint } = await launch(findBrowser(), profile);
try {
  const port = new URL(endpoint).port;
  const target = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
  const page = await connect(target.webSocketDebuggerUrl);
  await page.send('Page.enable');
  // Reduced motion shows every animation's final state: a background tab may never advance them.
  await page.send('Emulation.setEmulatedMedia', {
    features: [
      { name: 'prefers-color-scheme', value: theme },
      { name: 'prefers-reduced-motion', value: 'reduce' },
    ],
  });
  mkdirSync(resolve(outDir), { recursive: true });
  for (const width of widths) {
    const mobile = width < 600;
    await page.send('Emulation.setDeviceMetricsOverride', { width, height: mobile ? 844 : 900, deviceScaleFactor: 1, mobile });
    for (const name of names) {
      const loaded = page.once('Page.loadEventFired');
      await page.send('Page.navigate', { url: `${base}/design-lab/open?lab-scenario=${name}&lab-badge=0` });
      await loaded;
      await sleep(1600);
      const metrics = await page.send('Page.getLayoutMetrics');
      const height = Math.ceil(metrics.cssContentSize.height);
      const shot = await page.send('Page.captureScreenshot', {
        format: 'png',
        captureBeyondViewport: true,
        clip: { x: 0, y: 0, width, height, scale: 1 },
      });
      const file = resolve(outDir, `${name}-${width}-${theme}.png`);
      writeFileSync(file, Buffer.from(shot.data, 'base64'));
      console.log(file);
    }
  }
  page.close();
} finally {
  child.kill();
  await sleep(500);
  rmSync(profile, { recursive: true, force: true });
}

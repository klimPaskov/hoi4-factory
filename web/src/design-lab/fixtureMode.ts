// Fixture mode: the real app on sample API answers, for seeing and capturing every screen state
// (ADR-025). main.tsx installs it only in development. It turns on when a page opens with
// `?lab-scenario=<name>` and stays on for that browser tab until "Leave sample data".
import type { RunEvent } from '../api/tasks';
import i18n from '../i18n';
import labStrings from './lab.en.json';
import { SCENARIOS, type Reply, type Scenario } from './scenarios';
import './fixture.css';

const KEY = 'hf-lab-scenario';
const BADGE_KEY = 'hf-lab-badge';
const API = '/api/v1';

/** Reads `?lab-scenario=` (and `?lab-badge=0`, for screenshots) into this tab's session. */
function readScenario(): string | null {
  try {
    const params = new URLSearchParams(window.location.search);
    const requested = params.get('lab-scenario');
    const badge = params.get('lab-badge');
    if (requested !== null || badge !== null) {
      params.delete('lab-scenario');
      params.delete('lab-badge');
      const search = params.toString();
      window.history.replaceState(null, '', `${window.location.pathname}${search ? `?${search}` : ''}${window.location.hash}`);
    }
    if (badge !== null) window.sessionStorage.setItem(BADGE_KEY, badge);
    if (requested === '') window.sessionStorage.removeItem(KEY);
    else if (requested !== null) window.sessionStorage.setItem(KEY, requested);
    return window.sessionStorage.getItem(KEY);
  } catch {
    return null;
  }
}

/** Matches "GET /projects/prj_1/tasks" against the scenario, ignoring the query string. */
function reply(scenario: Scenario, method: string, path: string): Reply {
  const found = scenario.routes[`${method} ${path}`];
  if (found) return found;
  // Writes the scenario does not describe succeed quietly, so buttons can be tried.
  if (method !== 'GET') return { status: 200, json: {} };
  return { status: 404, json: { error: { code: 'not_found', message: 'Not found.', retry_class: 'terminal', affected_ids: {}, evidence_refs: [] } } };
}

function installFetch(scenario: Scenario) {
  const realFetch = window.fetch.bind(window);
  window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(input instanceof Request ? input.url : String(input), window.location.origin);
    if (url.origin !== window.location.origin || !url.pathname.startsWith(API)) return realFetch(input, init);
    const method = (init?.method ?? (input instanceof Request ? input.method : 'GET')).toUpperCase();
    const answer = reply(scenario, method, url.pathname.slice(API.length));
    if (answer.pending) return new Promise<Response>(() => {});
    // A short delay, so loading states flash the way they do against a real server.
    await new Promise((resolve) => setTimeout(resolve, 150));
    const status = answer.status ?? 200;
    return new Response(status === 204 ? null : JSON.stringify(answer.json ?? {}), {
      status,
      headers: { 'Content-Type': 'application/json' },
    });
  };
}

/** Plays the scenario's live events, or fails at once so the log falls back to polling. */
function installEventSource(scenario: Scenario) {
  class SampleEventSource extends EventTarget {
    onerror: ((event: Event) => void) | null = null;
    readonly url: string;
    private timers: ReturnType<typeof setTimeout>[] = [];
    constructor(url: string) {
      super();
      this.url = url;
      if (scenario.streamFails) {
        this.timers.push(setTimeout(() => this.onerror?.(new Event('error')), 300));
        return;
      }
      (scenario.stream ?? []).forEach((event: RunEvent, index) => {
        this.timers.push(
          setTimeout(() => this.dispatchEvent(new MessageEvent('run_event', { data: JSON.stringify(event) })), 1500 * (index + 1)),
        );
      });
    }
    close() {
      this.timers.forEach(clearTimeout);
    }
  }
  window.EventSource = SampleEventSource as unknown as typeof EventSource;
}

function showBadge(name: string) {
  const badge = document.createElement('div');
  badge.className = 'lab-fixture-badge';
  badge.setAttribute('role', 'note');
  const text = document.createElement('span');
  text.textContent = i18n.t('designLab.fixtures.badge', { name });
  const leave = document.createElement('button');
  leave.type = 'button';
  leave.textContent = i18n.t('designLab.fixtures.leave');
  leave.addEventListener('click', () => {
    window.sessionStorage.removeItem(KEY);
    window.location.assign('/design-lab/screens');
  });
  badge.append(text, leave);
  document.body.append(badge);
}

export function installFixtureMode(): void {
  const name = readScenario();
  const scenario = name ? SCENARIOS[name] : undefined;
  if (!name || !scenario) return;
  // /design-lab/open?lab-scenario=<name> goes straight to the scenario's page (used by
  // web/scripts/lab-screenshots.mjs).
  if (window.location.pathname === '/design-lab/open') window.history.replaceState(null, '', scenario.path);
  i18n.addResourceBundle('en', 'translation', labStrings, true, false);
  installFetch(scenario);
  installEventSource(scenario);
  if (window.sessionStorage.getItem(BADGE_KEY) !== '0') showBadge(name);
}

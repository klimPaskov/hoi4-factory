import { render } from '@testing-library/react';
import { I18nextProvider } from 'react-i18next';
import { MemoryRouter } from 'react-router';
import { vi } from 'vitest';
import { AppRoutes } from '../App';
import type { Account, Project } from '../api/types';
import { AuthProvider } from '../auth';
import i18n from '../i18n';

export interface MockResponse {
  status?: number;
  json?: unknown;
  headers?: Record<string, string>;
}

export interface RecordedRequest {
  method: string;
  path: string;
  body: unknown;
}

export type Handler = MockResponse | ((request: RecordedRequest) => MockResponse);

export function envelope(
  code: string,
  message: string,
  extra: { retry_class?: string; affected_ids?: Record<string, string> } = {},
) {
  return {
    error: {
      code,
      message,
      retry_class: extra.retry_class ?? 'terminal',
      affected_ids: extra.affected_ids ?? {},
      evidence_refs: [],
    },
  };
}

/**
 * Stands in for the server. Routes are "METHOD /path" (path relative to /api/v1, no query).
 * Anything unlisted answers 404 with the standard envelope, like the real fallback route.
 */
export function mockApi(routes: Record<string, Handler>) {
  const requests: RecordedRequest[] = [];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input), 'http://localhost');
    const method = init?.method ?? 'GET';
    const path = url.pathname.replace(/^\/api\/v1/, '');
    let body: unknown = undefined;
    if (typeof init?.body === 'string') body = JSON.parse(init.body);
    else if (init?.body) body = init.body;
    const recorded = { method, path, body };
    requests.push(recorded);
    const handler = routes[`${method} ${path}`];
    const response: MockResponse = handler
      ? typeof handler === 'function'
        ? handler(recorded)
        : handler
      : { status: 404, json: envelope('not_found', 'Not found.') };
    const status = response.status ?? 200;
    return new Response(status === 204 ? null : JSON.stringify(response.json ?? {}), {
      status,
      headers: { 'Content-Type': 'application/json', ...response.headers },
    });
  });
  vi.stubGlobal('fetch', fetchMock);
  return { requests, fetchMock };
}

export function account(overrides: Partial<Account> = {}): Account {
  return {
    id: 'acc_1',
    email: 'modder@example.com',
    email_verified: true,
    display_name: 'Modder',
    github_linked: false,
    has_password: true,
    created_at: '2026-10-01T10:00:00Z',
    ...overrides,
  };
}

export function project(overrides: Partial<Project> = {}): Project {
  return {
    id: 'prj_1',
    name: 'Test Mod',
    visibility: 'private',
    created_at: '2026-10-02T10:00:00Z',
    deleted_at: null,
    delete_after: null,
    role: 'owner',
    ...overrides,
  };
}

export const SIGNED_OUT: Record<string, Handler> = {
  'GET /auth/me': { status: 401, json: envelope('unauthenticated', 'Sign in to continue.', { retry_class: 'requires_authorization' }) },
};

export function signedIn(acc: Account = account()): Record<string, Handler> {
  return {
    'GET /auth/me': { json: { account: acc } },
    'GET /game-reference': { json: { raw_version: '1.19.3.0', dlc_ids: ['a', 'b', 'c'] } },
  };
}

export function renderApp(route: string) {
  return render(
    <I18nextProvider i18n={i18n}>
      <MemoryRouter initialEntries={[route]}>
        <AuthProvider>
          <AppRoutes />
        </AuthProvider>
      </MemoryRouter>
    </I18nextProvider>,
  );
}

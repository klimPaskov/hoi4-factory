import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Attempt, RunEvent, Task } from '../api/tasks';
import { envelope, mockApi, project, renderApp, signedIn, type Handler } from './helpers';

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: 'tsk_1',
    project_id: 'prj_1',
    objective: 'Add a focus for Côte d’Ivoire',
    mode: 'edit',
    allowed_paths: ['common/national_focus'],
    state: 'ready',
    paused: false,
    pause_reason: null,
    budget_micros: 200_000,
    max_turns: 20,
    model: { profile_id: 'anthropic:claude-haiku-5-5', funding: 'operator_allowance', key_id: null },
    version: 3,
    last_seq: 0,
    spent_micros: 1_234,
    spend_unknown: false,
    created_at: '2026-10-24T10:00:00Z',
    updated_at: '2026-10-24T10:00:00Z',
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
      revocation_epoch: 2,
      revoked: false,
      operation_classes: ['read_file', 'write_file'],
      allowed_roots: ['common/national_focus'],
      expires_at: '2026-10-24T12:00:00Z',
    },
    bundle: { id: 'bun_1', manifest_sha256: 'abc' },
    error_code: null,
    outcome: null,
    started_at: '2026-10-24T10:01:00Z',
    finished_at: null,
    version: 1,
    ...overrides,
  };
}

function event(seq: number, summary = `Event ${seq}`): RunEvent {
  return {
    seq,
    event_id: `evt_${seq}`,
    attempt_id: 'att_1',
    type: 'tool_call',
    actor: 'agent',
    summary,
    payload: { n: seq },
    created_at: '2026-10-24T10:02:00Z',
  };
}

class FakeEventSource {
  static instances: FakeEventSource[] = [];
  url: string;
  closed = false;
  onerror: (() => void) | null = null;
  listeners: Record<string, ((message: MessageEvent<string>) => void)[]> = {};
  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }
  addEventListener(type: string, listener: (message: MessageEvent<string>) => void) {
    (this.listeners[type] ??= []).push(listener);
  }
  close() {
    this.closed = true;
  }
  emit(ev: RunEvent) {
    for (const l of this.listeners.run_event ?? []) l({ data: JSON.stringify(ev) } as MessageEvent<string>);
  }
}

afterEach(() => {
  FakeEventSource.instances = [];
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const base = (): Record<string, Handler> => ({
  ...signedIn(),
  'GET /projects/prj_1': { json: { project: project() } },
});

describe('task list and create form', () => {
  const profiles = {
    profiles: [
      { id: 'anthropic:claude-haiku-5-5', label: 'Claude Haiku 5.5', provider: 'anthropic', default: true, operator_allowance: true, capabilities: {} },
      { id: 'openai:gpt-x', label: 'GPT X', provider: 'openai', default: false, operator_allowance: true, capabilities: {} },
    ],
    user_keys_available: true,
  };
  const keys = {
    available: true,
    keys: [
      { id: 'key_1', provider: 'anthropic', label: 'My Claude', base_url: null, key_hint: 'abcd', created_at: '2026-10-20T10:00:00Z', last_used_at: null },
    ],
  };

  it('lists tasks and links from the project page', async () => {
    mockApi({
      ...base(),
      'GET /projects/prj_1/tasks': { json: { tasks: [task({ paused: true, state: 'running' })] } },
      'GET /model-profiles': { json: profiles },
      'GET /account/api-keys': { json: keys },
    });
    const first = renderApp('/projects/prj_1');
    expect(await screen.findByRole('link', { name: 'Open tasks' })).toHaveAttribute('href', '/projects/prj_1/tasks');
    first.unmount();
    renderApp('/projects/prj_1/tasks');
    const link = await screen.findByRole('link', { name: /Côte d/ });
    expect(link).toHaveAttribute('href', '/projects/prj_1/tasks/tsk_1');
    expect(screen.getByText(/Paused/)).toBeInTheDocument();
    expect(screen.getByText(/Spent \$0\.0012 of \$0\.20/)).toBeInTheDocument();
  });

  it('submits the contract body for an edit task funded by a saved key', async () => {
    const { requests } = mockApi({
      ...base(),
      'GET /projects/prj_1/tasks': { json: { tasks: [] } },
      'GET /model-profiles': { json: profiles },
      'GET /account/api-keys': { json: keys },
      'POST /projects/prj_1/tasks': { status: 201, json: task() },
    });
    renderApp('/projects/prj_1/tasks');
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText(/What should the agent do/), 'Add a focus');
    // Allowed paths only show for edit tasks.
    expect(screen.queryByLabelText(/Allowed paths/)).not.toBeInTheDocument();
    await user.click(screen.getByRole('radio', { name: /Edit: the agent may change files/ }));
    expect(screen.getByText(/never to your main mod files/)).toBeInTheDocument();
    await user.type(screen.getByLabelText(/Allowed paths/), 'common/national_focus\n  events/x.txt ');
    expect(screen.getByLabelText(/Budget in dollars/)).toHaveValue('0.20');
    await user.selectOptions(screen.getByLabelText('Pay with'), 'key_1');
    await user.click(screen.getByRole('button', { name: 'Create task' }));
    await waitFor(() => expect(requests.some((r) => r.method === 'POST')).toBe(true));
    expect(requests.find((r) => r.method === 'POST')?.body).toEqual({
      objective: 'Add a focus',
      mode: 'edit',
      allowed_paths: ['common/national_focus', 'events/x.txt'],
      budget_usd: 0.2,
      model: { profile_id: 'anthropic:claude-haiku-5-5', key_id: 'key_1' },
    });
  });

  it('sends a read-only task with the free allowance and no paths', async () => {
    const { requests } = mockApi({
      ...base(),
      'GET /projects/prj_1/tasks': { json: { tasks: [] } },
      'GET /model-profiles': { json: profiles },
      'GET /account/api-keys': { status: 404, json: envelope('not_found', 'Not found.') },
      'POST /projects/prj_1/tasks': { status: 201, json: task({ mode: 'read_only' }) },
    });
    renderApp('/projects/prj_1/tasks');
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText(/What should the agent do/), 'Look around');
    await user.click(screen.getByRole('button', { name: 'Create task' }));
    await waitFor(() => expect(requests.some((r) => r.method === 'POST')).toBe(true));
    expect(requests.find((r) => r.method === 'POST')?.body).toEqual({
      objective: 'Look around',
      mode: 'read_only',
      allowed_paths: [],
      budget_usd: 0.2,
      model: { profile_id: 'anthropic:claude-haiku-5-5', key_id: null },
    });
  });

  it('asks for allowed paths before an edit task is sent', async () => {
    const { requests } = mockApi({
      ...base(),
      'GET /projects/prj_1/tasks': { json: { tasks: [] } },
      'GET /model-profiles': { json: profiles },
      'GET /account/api-keys': { json: keys },
    });
    renderApp('/projects/prj_1/tasks');
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText(/What should the agent do/), 'Edit');
    await user.click(screen.getByRole('radio', { name: /Edit: the agent/ }));
    await user.click(screen.getByRole('button', { name: 'Create task' }));
    expect(await screen.findByText('Enter at least one allowed path for an edit task.')).toBeInTheDocument();
    expect(requests.some((r) => r.method === 'POST')).toBe(false);
  });

  it('shows the not-available state when the task routes answer 404', async () => {
    mockApi(base());
    renderApp('/projects/prj_1/tasks');
    expect(await screen.findByText('Tasks is not available yet')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});

describe('task detail', () => {
  function detailRoutes(extra: Record<string, Handler> = {}, detail = {}): Record<string, Handler> {
    return {
      ...base(),
      'GET /projects/prj_1/tasks/tsk_1': {
        json: { task: task(), attempts: [attempt()], pending_effects: [], ...detail },
      },
      'GET /projects/prj_1/tasks/tsk_1/events': { json: { events: [event(1), event(2)], last_seq: 2 } },
      ...extra,
    };
  }

  it('renders the task, its attempt, grant and the event log', async () => {
    vi.stubGlobal('EventSource', FakeEventSource);
    mockApi(
      detailRoutes(
        {},
        {
          attempts: [
            attempt({
              state: 'interrupt_requested',
            }),
            attempt({
              id: 'att_0',
              attempt_no: 0,
              state: 'succeeded',
              outcome: { summary: 'Added a focus', changed_files: ['common/national_focus/a.txt'], branch: 'task/1', commit: 'deadbeef', open_questions: [] },
            }),
          ],
          pending_effects: [{ operation_id: 'op_9', tool: 'write_file', status: 'unknown' }],
        },
      ),
    );
    renderApp('/projects/prj_1/tasks/tsk_1');
    expect(await screen.findByRole('heading', { name: /Côte d/ })).toBeInTheDocument();
    // The server's word is shown as is: a requested stop is not a stop.
    expect(screen.getByText(/Stop requested: the agent has not confirmed/)).toBeInTheDocument();
    expect(screen.getByText('Succeeded')).toBeInTheDocument();
    expect(screen.getByText('Added a focus')).toBeInTheDocument();
    expect(screen.getAllByText('read_file, write_file')).toHaveLength(2);
    expect(screen.getByText(/write_file \(unknown\), operation op_9/)).toBeInTheDocument();
    expect(await screen.findByText('Event 2')).toBeInTheDocument();
    expect(screen.getByText('Event 1')).toBeInTheDocument();
    await waitFor(() => expect(FakeEventSource.instances).toHaveLength(1));
    expect(FakeEventSource.instances[0]?.url).toBe('/api/v1/projects/prj_1/tasks/tsk_1/events/stream?after=2');
    // The raw payload sits behind a disclosure.
    expect(screen.getAllByText('Details')).toHaveLength(2);
  });

  it('fills a gap from /events?after= when the stream skips a seq, and ignores duplicates', async () => {
    vi.stubGlobal('EventSource', FakeEventSource);
    const { requests } = mockApi(
      detailRoutes({
        'GET /projects/prj_1/tasks/tsk_1/events': (r) =>
          r.search.includes('after=0')
            ? { json: { events: [event(1), event(2)], last_seq: 2 } }
            : { json: { events: [event(3), event(4), event(5)], last_seq: 5 } },
      }),
    );
    renderApp('/projects/prj_1/tasks/tsk_1');
    await screen.findByText('Event 2');
    await waitFor(() => expect(FakeEventSource.instances).toHaveLength(1));
    const source = FakeEventSource.instances[0]!;
    act(() => source.emit(event(2, 'Duplicate of 2')));
    act(() => source.emit(event(5)));
    expect(await screen.findByText('Event 3')).toBeInTheDocument();
    expect(screen.getByText('Event 5')).toBeInTheDocument();
    expect(screen.queryByText('Duplicate of 2')).not.toBeInTheDocument();
    const fills = requests.filter((r) => r.path.endsWith('/events') && r.search.startsWith('?after=2'));
    expect(fills.length).toBeGreaterThan(0);
    // An in-order event just appends.
    act(() => source.emit(event(6)));
    expect(await screen.findByText('Event 6')).toBeInTheDocument();
  });

  it('falls back to polling every 3 seconds when the stream fails', async () => {
    vi.stubGlobal('EventSource', FakeEventSource);
    const intervals = vi.spyOn(globalThis, 'setInterval');
    let calls = 0;
    mockApi(
      detailRoutes({
        'GET /projects/prj_1/tasks/tsk_1/events': (r) => {
          if (r.search.includes('after=0')) return { json: { events: [event(1)], last_seq: 1 } };
          calls += 1;
          return { json: { events: [event(2, 'Polled event')], last_seq: 2 } };
        },
      }),
    );
    renderApp('/projects/prj_1/tasks/tsk_1');
    await screen.findByText('Event 1');
    await waitFor(() => expect(FakeEventSource.instances).toHaveLength(1));
    act(() => FakeEventSource.instances[0]!.onerror?.());
    expect(await screen.findByText(/checked every few seconds/)).toBeInTheDocument();
    expect(FakeEventSource.instances[0]!.closed).toBe(true);
    const poll = intervals.mock.calls.find((c) => c[1] === 3000);
    expect(poll).toBeDefined();
    await act(async () => {
      (poll![0] as () => void)();
    });
    expect(await screen.findByText('Polled event')).toBeInTheDocument();
    expect(calls).toBeGreaterThan(0);
  });

  it('starts an attempt with the task version, and refetches after a version conflict', async () => {
    vi.stubGlobal('EventSource', FakeEventSource);
    let gets = 0;
    const { requests } = mockApi(
      detailRoutes({
        'GET /projects/prj_1/tasks/tsk_1': () => {
          gets += 1;
          return { json: { task: task({ version: gets === 1 ? 3 : 4 }), attempts: [], pending_effects: [] } };
        },
        'POST /projects/prj_1/tasks/tsk_1/attempts': {
          status: 409,
          json: envelope('version_conflict', 'Version changed.', { retry_class: 'refresh_preconditions' }),
        },
      }),
    );
    renderApp('/projects/prj_1/tasks/tsk_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Start attempt' }));
    expect(await screen.findByText(/Someone else changed this task/)).toBeInTheDocument();
    expect(requests.find((r) => r.path.endsWith('/attempts'))?.body).toEqual({ expected_version: 3 });
    await waitFor(() => expect(gets).toBeGreaterThanOrEqual(2));
  });

  it('pauses, resumes, interrupts and revokes with the contract calls', async () => {
    vi.stubGlobal('EventSource', FakeEventSource);
    const { requests } = mockApi(
      detailRoutes({
        'POST /projects/prj_1/tasks/tsk_1/pause': { json: task({ paused: true }) },
        'POST /projects/prj_1/tasks/tsk_1/attempts/att_1/interrupt': { json: attempt({ state: 'interrupt_requested' }) },
        'POST /projects/prj_1/tasks/tsk_1/grants/grt_1/revoke': { json: { id: 'grt_1', revocation_epoch: 3, revoked: true } },
      }),
    );
    renderApp('/projects/prj_1/tasks/tsk_1');
    const user = userEvent.setup();
    // With an active attempt there is no Start button.
    await screen.findByRole('button', { name: 'Pause' });
    expect(screen.queryByRole('button', { name: 'Start attempt' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Pause' }));
    await user.click(await screen.findByRole('button', { name: 'Interrupt' }));
    await user.click(await screen.findByRole('button', { name: 'Revoke grant' }));
    await waitFor(() => expect(requests.some((r) => r.path.endsWith('/revoke'))).toBe(true));
    expect(requests.find((r) => r.path.endsWith('/pause'))?.body).toEqual({ expected_version: 3 });
    expect(requests.some((r) => r.path.endsWith('/attempts/att_1/interrupt'))).toBe(true);
  });

  it('offers Resume and hides Start while the task is paused', async () => {
    vi.stubGlobal('EventSource', FakeEventSource);
    mockApi(
      detailRoutes(
        {},
        { task: task({ paused: true, pause_reason: 'checking cost' }), attempts: [] },
      ),
    );
    renderApp('/projects/prj_1/tasks/tsk_1');
    expect(await screen.findByRole('button', { name: 'Resume' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Start attempt' })).not.toBeInTheDocument();
    expect(screen.getByText(/This task is paused: checking cost/)).toBeInTheDocument();
  });

  it('shows the not-available state on 404', async () => {
    mockApi(base());
    renderApp('/projects/prj_1/tasks/tsk_1');
    expect(await screen.findByText('Tasks is not available yet')).toBeInTheDocument();
  });
});

describe('API keys and usage on the account page', () => {
  const usage = {
    month: '2026-10',
    verified: true,
    allowance_micros: 1_000_000,
    operator_spent_micros: 1_234,
    remaining_allowance_micros: 998_766,
    unknown_count: 0,
    user_key_spent_micros: 0,
  };
  const key = { id: 'key_1', provider: 'anthropic', label: 'My Claude', base_url: null, key_hint: 'abcd', created_at: '2026-10-20T10:00:00Z', last_used_at: null };

  it('shows the allowance left and says when some usage is not known yet', async () => {
    mockApi({
      ...signedIn(),
      'GET /account/usage': { json: { ...usage, unknown_count: 2 } },
      'GET /account/api-keys': { json: { available: true, keys: [] } },
    });
    renderApp('/account');
    expect(await screen.findByText(/Free monthly allowance: \$0\.9988 of \$1\.00 left/)).toBeInTheDocument();
    expect(screen.getByText(/2 uses are not known yet and are counted at their reserved amount/)).toBeInTheDocument();
  });

  it('adds a key with a password field that is cleared afterwards, and never shows the key', async () => {
    let listed = [] as unknown[];
    const { requests } = mockApi({
      ...signedIn(),
      'GET /account/usage': { json: usage },
      'GET /account/api-keys': () => ({ json: { available: true, keys: listed } }),
      'POST /account/api-keys': () => {
        listed = [key];
        return { status: 201, json: key };
      },
    });
    renderApp('/account');
    const user = userEvent.setup();
    const field = await screen.findByLabelText('API key');
    expect(field).toHaveAttribute('type', 'password');
    expect(field).toHaveAttribute('autocomplete', 'off');
    await user.type(screen.getByLabelText(/Name for this key/), 'My Claude');
    await user.type(field, 'sk-secret-value');
    await user.click(screen.getByRole('button', { name: 'Save key' }));
    expect(await screen.findByRole('heading', { name: 'My Claude' })).toBeInTheDocument();
    expect(screen.getByLabelText('API key')).toHaveValue('');
    expect(screen.getByText(/Ends in abcd/)).toBeInTheDocument();
    expect(document.body.textContent).not.toContain('sk-secret-value');
    expect(requests.find((r) => r.method === 'POST')?.body).toEqual({
      provider: 'anthropic',
      label: 'My Claude',
      api_key: 'sk-secret-value',
    });
  });

  it('requires a server address for an OpenAI-compatible key, clears the key when refused', async () => {
    const { requests } = mockApi({
      ...signedIn(),
      'GET /account/usage': { json: usage },
      'GET /account/api-keys': { json: { available: true, keys: [] } },
      'POST /account/api-keys': { status: 400, json: envelope('destination_blocked', 'blocked') },
    });
    renderApp('/account');
    const user = userEvent.setup();
    await user.selectOptions(await screen.findByLabelText('Provider'), 'openai_compatible');
    await user.type(screen.getByLabelText(/Name for this key/), 'Local');
    await user.type(screen.getByLabelText('API key'), 'k-1');
    await user.click(screen.getByRole('button', { name: 'Save key' }));
    expect(await screen.findByText('Enter the server address.')).toBeInTheDocument();
    await user.type(screen.getByLabelText(/Server address/), 'http://10.0.0.1/v1');
    await user.click(screen.getByRole('button', { name: 'Save key' }));
    expect(await screen.findByText(/That address is not allowed/)).toBeInTheDocument();
    expect(screen.getByLabelText('API key')).toHaveValue('');
    expect(requests.find((r) => r.method === 'POST')?.body).toEqual({
      provider: 'openai_compatible',
      label: 'Local',
      api_key: 'k-1',
      base_url: 'http://10.0.0.1/v1',
    });
  });

  it('deletes a key after a confirmation', async () => {
    let listed = [key] as unknown[];
    const { requests } = mockApi({
      ...signedIn(),
      'GET /account/usage': { json: usage },
      'GET /account/api-keys': () => ({ json: { available: true, keys: listed } }),
      'DELETE /account/api-keys/key_1': () => {
        listed = [];
        return { status: 204 };
      },
    });
    renderApp('/account');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Delete' }));
    expect(requests.some((r) => r.method === 'DELETE')).toBe(false);
    const confirm = screen.getByRole('group', { name: 'Confirm deleting this key' });
    await user.click(within(confirm).getByRole('button', { name: 'Yes, delete the key' }));
    expect(await screen.findByText('You have not saved any keys.')).toBeInTheDocument();
    expect(requests.some((r) => r.method === 'DELETE' && r.path === '/account/api-keys/key_1')).toBe(true);
  });

  it('says keys cannot be saved when the server has no key encryption', async () => {
    mockApi({
      ...signedIn(),
      'GET /account/usage': { json: usage },
      'GET /account/api-keys': { json: { available: false, keys: [] } },
    });
    renderApp('/account');
    expect(await screen.findByText(/cannot save API keys/)).toBeInTheDocument();
    expect(screen.queryByLabelText('API key')).not.toBeInTheDocument();
    expect(screen.getByText(/Free monthly allowance/)).toBeInTheDocument();
  });

  it('shows the not-available state when the routes answer 404', async () => {
    mockApi(signedIn());
    renderApp('/account');
    expect(await screen.findByText('API keys is not available yet')).toBeInTheDocument();
    expect(screen.getByText('Usage is not available yet')).toBeInTheDocument();
  });
});

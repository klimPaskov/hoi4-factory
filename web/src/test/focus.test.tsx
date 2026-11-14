import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Proposal } from '../api/focus';
import type { Attempt, Task } from '../api/tasks';
import { AGENT_PROPOSAL, AGENT_PROPOSAL_SAVED, AGENT_SENTENCE, COMMIT_PUBLICATION, OWN_PROPOSAL, PROJECTION, TREE_ID } from '../design-lab/focusFixtures';
import map from '../design-lab/render/game-proposed.json';
import { envelope, mockApi, project, renderApp, signedIn, type Handler } from './helpers';

const TASK = '/projects/prj_1/tasks/tsk_1';
const TREE = `${TASK}/focus/trees/${TREE_ID}`;
const EDITOR = `${TASK}/focus/${TREE_ID}`;

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: 'tsk_1',
    project_id: 'prj_1',
    objective: 'Add a Nordic trade branch',
    mode: 'edit',
    allowed_paths: ['common/national_focus', 'localisation/english'],
    state: 'in_review',
    paused: false,
    pause_reason: null,
    budget_micros: 200_000,
    max_turns: 20,
    model: { profile_id: 'anthropic:claude-haiku-5-5', funding: 'operator_allowance', key_id: null },
    version: 7,
    last_seq: 12,
    spent_micros: 1_234,
    spend_unknown: false,
    created_at: '2026-11-02T09:00:00Z',
    updated_at: '2026-11-02T09:36:00Z',
    ...overrides,
  };
}

function attempt(overrides: Partial<Attempt> = {}): Attempt {
  return {
    id: 'att_1',
    attempt_no: 1,
    state: 'succeeded',
    grant: { id: 'grt_1', revocation_epoch: 1, revoked: false, operation_classes: ['write_file'], allowed_roots: ['common/national_focus'], expires_at: '2026-11-02T12:00:00Z' },
    bundle: { id: 'bun_1', manifest_sha256: 'abc' },
    error_code: null,
    outcome: { summary: AGENT_SENTENCE, changed_files: [], branch: null, commit: null, open_questions: [] },
    started_at: '2026-11-02T09:30:00Z',
    finished_at: '2026-11-02T09:36:00Z',
    version: 1,
    ...overrides,
  };
}

/** The routes both pages read. `proposals` is read on every call, so a test can change it. */
function routes(proposals: () => Proposal[], more: Record<string, Handler> = {}, attempts: Attempt[] = [attempt()]): Record<string, Handler> {
  return {
    ...signedIn(),
    'GET /projects/prj_1': { json: { project: project() } },
    [`GET ${TASK}`]: { json: { task: task(), attempts, pending_effects: [] } },
    [`GET ${TASK}/events`]: { json: { events: [], last_seq: 0 } },
    [`GET ${TREE}`]: { json: PROJECTION },
    [`GET ${TREE}/preview.json`]: { json: map },
    [`GET ${TREE}/preview.png`]: { json: {} },
    [`GET ${TASK}/proposals`]: () => ({ json: { proposals: proposals() } }),
    [`GET ${TASK}/proposals/${AGENT_PROPOSAL.id}`]: () => ({ json: proposals().find((p) => p.id === AGENT_PROPOSAL.id) ?? AGENT_PROPOSAL }),
    [`GET ${TASK}/publications`]: { json: { publications: [] } },
    ...more,
  };
}

beforeEach(() => {
  // jsdom has no layout, object URLs or media queries. The pages only need them to exist.
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    },
  );
  vi.stubGlobal('matchMedia', () => ({ matches: true, addEventListener() {}, removeEventListener() {} }));
  URL.createObjectURL = vi.fn(() => 'blob:picture');
  URL.revokeObjectURL = vi.fn();
  Element.prototype.setPointerCapture = () => {};
});

describe('the focus editor', () => {
  it('shows what the agent did in its own sentence and saves it on Approve', async () => {
    let list = [AGENT_PROPOSAL];
    const api = mockApi(
      routes(() => list, {
        [`POST ${TASK}/commit`]: () => {
          list = [AGENT_PROPOSAL_SAVED];
          return { json: { publication_id: 'pub_1', result: {} } };
        },
      }),
    );
    renderApp(EDITOR);
    expect(await screen.findByText(AGENT_SENTENCE)).toBeInTheDocument();
    // New and changed focuses are named as such on the picture.
    expect(await screen.findByRole('button', { name: 'Nordic Trade Pact, new' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Open Trade, changed' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Approve' }));
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument());
    const commit = api.requests.find((r) => r.method === 'POST' && r.path === `${TASK}/commit`);
    expect(commit?.body).toMatchObject({ proposals: [{ id: AGENT_PROPOSAL.id, proposal_hash: AGENT_PROPOSAL.proposalHash }], accept_incomplete: false });
  });

  it('shows no layout numbers, internal names or lists by default', async () => {
    mockApi(routes(() => [AGENT_PROPOSAL]));
    renderApp(EDITOR);
    await screen.findByText(AGENT_SENTENCE);
    const page = document.querySelector('.fx-page') as HTMLElement;
    expect(within(page).queryByRole('spinbutton')).not.toBeInTheDocument();
    expect(within(page).queryByRole('list')).not.toBeInTheDocument();
    expect(page.textContent).not.toMatch(/SWE_|Column|Row\b/);
  });

  it('moves a focus with the arrow keys as one move operation on the tree it looked at', async () => {
    const api = mockApi(
      routes(() => [], {
        [`POST ${TREE}/operations`]: { status: 201, json: { proposal: OWN_PROPOSAL, projection: PROJECTION } },
      }),
    );
    renderApp(EDITOR);
    const focus = await screen.findByRole('button', { name: 'Ore Exports' });
    focus.focus();
    await userEvent.keyboard('{ArrowDown}');
    await waitFor(() => expect(api.requests.some((r) => r.path === `${TREE}/operations`)).toBe(true));
    const sent = api.requests.find((r) => r.path === `${TREE}/operations`);
    expect(sent?.body).toEqual({ expected_projection_hash: PROJECTION.projectionHash, operations: [{ op: 'move', focus_id: 'SWE_ore_exports', x: 0, y: 3 }] });
    // A person's own edit is saved or undone, not approved.
    expect(await screen.findByRole('button', { name: 'Save' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo' })).toBeInTheDocument();
    expect(screen.getByText('Changed: Ore Exports.')).toBeInTheDocument();
  });

  it('changes the cost from the card of the selected focus', async () => {
    const api = mockApi(
      routes(() => [], {
        [`POST ${TREE}/operations`]: { status: 201, json: { proposal: OWN_PROPOSAL, projection: PROJECTION } },
      }),
    );
    renderApp(EDITOR);
    await userEvent.click(await screen.findByRole('button', { name: 'Military Reform' }));
    const card = screen.getByRole('complementary', { name: 'Military Reform' });
    const cost = within(card).getByLabelText('Cost');
    await userEvent.clear(cost);
    await userEvent.type(cost, '5{Enter}');
    await waitFor(() => expect(api.requests.some((r) => r.path === `${TREE}/operations`)).toBe(true));
    expect(api.requests.find((r) => r.path === `${TREE}/operations`)?.body).toMatchObject({ operations: [{ op: 'set_cost', focus_id: 'SWE_military_reform', cost: 5 }] });
  });

  it('changes a title from the card and sends only what changed', async () => {
    const api = mockApi(
      routes(() => [], {
        [`POST ${TREE}/operations`]: { status: 201, json: { proposal: OWN_PROPOSAL, projection: PROJECTION } },
      }),
    );
    renderApp(EDITOR);
    await userEvent.click(await screen.findByRole('button', { name: 'Military Reform' }));
    const card = screen.getByRole('complementary', { name: 'Military Reform' });
    const title = within(card).getByLabelText('Title');
    await userEvent.clear(title);
    await userEvent.type(title, 'Army Reform');
    await userEvent.click(within(card).getByLabelText('Description'));
    await waitFor(() => expect(api.requests.some((r) => r.path === `${TREE}/operations`)).toBe(true));
    expect(api.requests.find((r) => r.path === `${TREE}/operations`)?.body).toMatchObject({
      operations: [{ op: 'set_text', focus_id: 'SWE_military_reform', title: 'Army Reform' }],
    });
    expect((api.requests.find((r) => r.path === `${TREE}/operations`)?.body as { operations: object[] }).operations[0]).not.toHaveProperty('description');
  });

  it('asks the agent for a change as a new attempt with the request', async () => {
    const api = mockApi(
      routes(() => [], {
        [`POST ${TASK}/attempts`]: { status: 201, json: { task: task(), attempt: attempt({ state: 'running', outcome: null }) } },
      }),
    );
    renderApp(EDITOR);
    await userEvent.type(await screen.findByLabelText('Ask the agent for a change'), 'Add a naval branch');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => expect(api.requests.some((r) => r.method === 'POST' && r.path === `${TASK}/attempts`)).toBe(true));
    expect(api.requests.find((r) => r.method === 'POST' && r.path === `${TASK}/attempts`)?.body).toEqual({ expected_version: 7, instruction: 'Add a naval branch' });
  });

  it('undoes open changes newest first', async () => {
    const second = { ...OWN_PROPOSAL, id: 'second' };
    let list: Proposal[] = [OWN_PROPOSAL, second];
    const api = mockApi(
      routes(() => list, {
        [`POST ${TASK}/proposals/second/discard`]: () => {
          list = list.filter((p) => p.id !== 'second');
          return { json: {} };
        },
        [`POST ${TASK}/proposals/${OWN_PROPOSAL.id}/discard`]: () => {
          list = [];
          return { json: {} };
        },
      }),
    );
    renderApp(EDITOR);
    await userEvent.click(await screen.findByRole('button', { name: 'Undo' }));
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Undo' })).not.toBeInTheDocument());
    const discards = api.requests.filter((r) => r.path.endsWith('/discard')).map((r) => r.path);
    expect(discards).toEqual([`${TASK}/proposals/second/discard`, `${TASK}/proposals/${OWN_PROPOSAL.id}/discard`]);
  });

  it('says what stands in the way when the check fails, and keeps only Reject', async () => {
    mockApi(
      routes(() => [AGENT_PROPOSAL], {
        [`POST ${TASK}/commit`]: { status: 409, json: envelope('commit_blocked', 'The commit check found new problems.') },
      }),
    );
    renderApp(EDITOR);
    await userEvent.click(await screen.findByRole('button', { name: 'Approve' }));
    expect(await screen.findByText('A check found a problem with this change.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Reject' })).toBeInTheDocument();
  });

  it('locks the tree and says so while the agent works', async () => {
    mockApi(routes(() => [], {}, [attempt({ state: 'running', outcome: null, finished_at: null })]));
    renderApp(EDITOR);
    expect(await screen.findByText('The agent is working on it.')).toBeInTheDocument();
    expect(screen.getByLabelText('Ask the agent for a change')).toBeDisabled();
  });

  it('explains a tree that cannot be loaded and offers to try again', async () => {
    mockApi(routes(() => [], { [`GET ${TREE}`]: { status: 503, json: envelope('tools_unavailable', 'HOI4 Agent Tools are unavailable.', { retry_class: 'safe_same_inputs' }) } }));
    renderApp(EDITOR);
    expect(await screen.findByRole('heading', { name: 'Could not open this focus tree' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});

describe('the change card', () => {
  const CARD = `${TASK}/changes/${AGENT_PROPOSAL.id}`;

  it('shows the result, one sentence and two actions, with no before and after', async () => {
    mockApi(routes(() => [AGENT_PROPOSAL]));
    renderApp(CARD);
    expect(await screen.findByText(AGENT_SENTENCE)).toBeInTheDocument();
    expect(screen.getAllByText('Open Trade: cost 10 to 7').length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: 'Approve' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Reject' })).toBeInTheDocument();
    expect(screen.queryByText('Before')).not.toBeInTheDocument();
    expect(screen.queryByText('After')).not.toBeInTheDocument();
    // The diff, the checks and the limits exist, behind the one closed Details toggle.
    const details = screen.getByText('Details').closest('details');
    expect(details).not.toHaveAttribute('open');
    expect(within(details as HTMLElement).getByText('Changes by file')).toBeInTheDocument();
    expect(within(details as HTMLElement).getByText('Checks')).toBeInTheDocument();
    expect(within(details as HTMLElement).getByText('Limits and restore point')).toBeInTheDocument();
  });

  it('binds the approval to the proposal hash and shows the saved state', async () => {
    let list = [AGENT_PROPOSAL];
    const api = mockApi(
      routes(() => list, {
        [`POST ${TASK}/commit`]: () => {
          list = [AGENT_PROPOSAL_SAVED];
          return { json: { publication_id: COMMIT_PUBLICATION.id, result: COMMIT_PUBLICATION.result } };
        },
        [`GET ${TASK}/publications`]: () => ({ json: { publications: list[0]?.state === 'committed' ? [COMMIT_PUBLICATION] : [] } }),
      }),
    );
    renderApp(CARD);
    await userEvent.click(await screen.findByRole('button', { name: 'Approve' }));
    expect(await screen.findByText('Approved. The change is saved in your project.')).toBeInTheDocument();
    expect(api.requests.find((r) => r.path === `${TASK}/commit`)?.body).toMatchObject({ proposals: [{ id: AGENT_PROPOSAL.id, proposal_hash: AGENT_PROPOSAL.proposalHash }], accept_incomplete: false });
    expect(screen.getByText('A restore point was saved, so this change can be undone.')).toBeInTheDocument();
  });

  it('asks again before accepting a check that could not finish', async () => {
    let calls = 0;
    const api = mockApi(
      routes(() => [AGENT_PROPOSAL], {
        [`POST ${TASK}/commit`]: () => {
          calls += 1;
          return calls === 1 ? { status: 409, json: envelope('check_incomplete', 'The check could not finish.') } : { json: { publication_id: 'pub_1', result: {} } };
        },
      }),
    );
    renderApp(CARD);
    await userEvent.click(await screen.findByRole('button', { name: 'Approve' }));
    expect(await screen.findByText('Part of this change could not be checked.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Approve anyway' }));
    await waitFor(() => expect(calls).toBe(2));
    const commits = api.requests.filter((r) => r.path === `${TASK}/commit`).map((r) => (r.body as { accept_incomplete: boolean }).accept_incomplete);
    expect(commits).toEqual([false, true]);
  });

  it('puts the files back on Reject', async () => {
    let list = [AGENT_PROPOSAL];
    const api = mockApi(
      routes(() => list, {
        [`POST ${TASK}/proposals/${AGENT_PROPOSAL.id}/discard`]: () => {
          list = [{ ...AGENT_PROPOSAL, state: 'discarded' }];
          return { json: {} };
        },
      }),
    );
    renderApp(CARD);
    await userEvent.click(await screen.findByRole('button', { name: 'Reject' }));
    expect(await screen.findByText('Rejected. Nothing was changed.')).toBeInTheDocument();
    expect(api.requests.some((r) => r.method === 'POST' && r.path.endsWith('/discard'))).toBe(true);
  });
});

describe('the task page', () => {
  it('links to the focus tree and to the change that waits for review', async () => {
    mockApi(routes(() => [AGENT_PROPOSAL]));
    renderApp(TASK);
    const tree = await screen.findByRole('link', { name: 'Open the focus tree' });
    expect(tree).toHaveAttribute('href', EDITOR);
    expect(screen.getByRole('link', { name: 'Review the change' })).toHaveAttribute('href', `${TASK}/changes/${AGENT_PROPOSAL.id}`);
  });
});

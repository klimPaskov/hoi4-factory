import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { envelope, mockApi, renderApp, type MockResponse } from './helpers';
import {
  analysisJson,
  appliedJson,
  HASH_A,
  HASH_B,
  planJson,
  recoveryJson,
  reviewValues,
  scanJson,
  SETUP,
  sent,
  setupRoutes,
  statusJson,
} from './setupFixtures';

const NONE = statusJson('none');

/** The step the person is working on now. */
function currentStep() {
  return document.querySelector('[aria-current="step"] h3');
}

/** Holds back the answer to one route until `release()` is called. */
function gate(fetchMock: ReturnType<typeof mockApi>['fetchMock'], route: string) {
  const original = fetchMock.getMockImplementation();
  let release: () => void = () => undefined;
  const closed = new Promise<void>((resolve) => {
    release = resolve;
  });
  fetchMock.mockImplementation(async (input, init) => {
    if (String(input).endsWith(route)) await closed;
    return original!(input, init);
  });
  return release;
}

const scanned = statusJson('evidence_approved', { scan: scanJson() });
const analyzed = statusJson('analyzed', { scan: scanJson(), analysis: analysisJson() });
const confirmed = statusJson('confirmed', {
  scan: scanJson(),
  analysis: analysisJson(),
  confirmed_values: reviewValues(),
});

function conflictPlan(overrides: Record<string, unknown> = {}) {
  return planJson({
    operations: [
      { path: 'descriptor.mod', action: 'conflict', note: 'differs' },
      { path: 'common/new.txt', action: 'create' },
    ],
    conflicts: [{ path: 'descriptor.mod', options: ['keep', 'replace', 'rename'], selected: null }],
    summary: { create: 1, change: 0, preserve: 0, conflict: 1 },
    ...overrides,
  });
}

describe('the Setup wizard', () => {
  it('walks every step with explicit confirmations and approves the exact plan shown', async () => {
    const { requests } = mockApi(
      setupRoutes(NONE, {
        [`POST ${SETUP}/scan`]: { json: scanJson() },
        [`POST ${SETUP}/evidence`]: { json: { approved: 2 } },
        [`POST ${SETUP}/analyze`]: { json: analysisJson() },
        [`POST ${SETUP}/confirm`]: { json: { confirmed: true, values_sha256: '4'.repeat(64) } },
        [`POST ${SETUP}/plan`]: { json: { plan: planJson() } },
        [`POST ${SETUP}/approve`]: {
          json: { approval: { approval_id: 'ap_1', plan_sha256: HASH_A, root_identity: 'c'.repeat(64) } },
        },
        [`POST ${SETUP}/apply`]: {
          json: { ...appliedJson(), unplanned_changes: ['extra.txt'] },
        },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();

    // 1. Scan.
    await user.click(await screen.findByRole('button', { name: 'Scan the mod' }));
    expect(await screen.findByText('مود')).toHaveAttribute('dir', 'auto');
    expect(sent(requests, 'POST', 'scan')[0]?.body).toEqual({ mode: 'existing' });
    expect(currentStep()).toHaveTextContent('2. Approve excerpts');

    // 2. Evidence: everything starts checked, nothing is sent until the explicit button.
    expect(screen.getByText(/sent to a third-party model provider/)).toBeInTheDocument();
    const excerpt = screen.getByText('name="Test Mod"');
    expect(excerpt.tagName).toBe('PRE');
    expect(excerpt).toHaveAttribute('dir', 'auto');
    expect(screen.getByRole('checkbox', { name: 'ev-1' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'ev-2' })).toBeChecked();
    expect(sent(requests, 'POST', 'evidence')).toHaveLength(0);
    await user.click(screen.getByRole('checkbox', { name: 'ev-2' }));
    expect(
      screen.getByRole('button', { name: 'Approve this excerpt and send it to the model' }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('checkbox', { name: 'ev-2' }));
    await user.click(
      screen.getByRole('button', { name: 'Approve these 2 excerpts and send them to the model' }),
    );
    expect(await screen.findByText('You approved 2 excerpts for the model.')).toBeInTheDocument();
    expect(sent(requests, 'POST', 'evidence')[0]?.body).toEqual({
      scan_id: 'scan_1',
      references: ['ev-1', 'ev-2'],
    });

    // 3. Analysis.
    expect(currentStep()).toHaveTextContent('3. Analyze');
    await user.click(screen.getByRole('button', { name: 'Analyze with Claude' }));
    expect(await screen.findByText('Claude analyzed the mod (claude-test-model, 2 attempts).')).toBeInTheDocument();

    // 4. Confirm values: pre-filled, editable, and clear that nothing is written yet.
    expect(currentStep()).toHaveTextContent('4. Confirm values');
    expect(screen.getByText(/Nothing is written to your mod yet/)).toBeInTheDocument();
    expect(screen.getByText('No localisation files were found.')).toBeInTheDocument();
    expect(screen.getByText('85%')).toBeInTheDocument();
    expect(screen.getByText('Named in descriptor.mod')).toBeInTheDocument();
    expect(screen.getByText(/hoi4-agent-tools/)).toBeInTheDocument();
    expect(screen.getByLabelText('Display name')).toHaveValue('Test Mod');
    expect(screen.getByLabelText('Folders')).toHaveValue('common\nevents');
    expect(screen.getByLabelText('Descriptor tags')).toHaveValue('Alternative History, Gameplay');
    const name = screen.getByLabelText('Display name');
    await user.clear(name);
    await user.type(name, 'Edited Mod');
    await user.click(screen.getByRole('button', { name: 'Confirm exactly these values' }));
    expect(await screen.findByText(/Values confirmed/)).toBeInTheDocument();
    const confirmBody = sent(requests, 'POST', 'confirm')[0]?.body as {
      analysis_id: string;
      values: ReturnType<typeof reviewValues>;
    };
    expect(confirmBody.analysis_id).toBe('an_1');
    expect(confirmBody.values.identity.display_name).toBe('Edited Mod');
    expect(confirmBody.values.identity.descriptor_tags).toEqual(['Alternative History', 'Gameplay']);
    expect(confirmBody.values.folder_profile).toEqual(['common', 'events']);

    // 5. Plan.
    expect(sent(requests, 'POST', 'plan')).toHaveLength(0);
    await user.click(screen.getByRole('button', { name: 'Build the plan' }));
    expect(await screen.findByText('descriptor.mod')).toBeInTheDocument();
    expect(screen.getByText('hoi4-agent-tools')).toBeInTheDocument();
    expect(screen.getAllByTitle(HASH_A).length).toBeGreaterThan(0);
    expect(screen.getAllByText('a'.repeat(12)).length).toBeGreaterThan(0);

    // 6. Approve and apply: disabled until the plan is marked as reviewed.
    expect(currentStep()).toHaveTextContent('6. Approve and apply');
    expect(screen.getByText('2 files will be written to the mod.')).toBeInTheDocument();
    const apply = screen.getByRole('button', { name: 'Approve and apply this exact plan' });
    expect(apply).toBeDisabled();
    await user.click(apply);
    expect(sent(requests, 'POST', 'approve')).toHaveLength(0);
    await user.click(screen.getByRole('checkbox', { name: 'I reviewed this plan' }));
    expect(apply).toBeEnabled();
    await user.click(apply);

    expect(await screen.findByText('Done. The plan was applied.')).toBeInTheDocument();
    expect(screen.getByText(/1 added, 1 changed, 0 removed\./)).toBeInTheDocument();
    expect(screen.getByText('Changes that were not in the plan')).toBeInTheDocument();
    expect(screen.getByText('extra.txt')).toBeInTheDocument();
    expect(sent(requests, 'POST', 'approve')[0]?.body).toEqual({ plan_id: 'p1', plan_sha256: HASH_A });
    expect(sent(requests, 'POST', 'apply')[0]?.body).toEqual({ plan_id: 'p1' });
    const order = requests.map((r) => r.path.replace(`${SETUP}/`, ''));
    expect(order.indexOf('approve')).toBeLessThan(order.indexOf('apply'));
    expect(currentStep()).toBeNull();
  });

  it('shows an incomplete scan loudly and lets the person scan again', async () => {
    let scans = 0;
    const { requests } = mockApi(
      setupRoutes(NONE, {
        [`POST ${SETUP}/scan`]: () => {
          scans += 1;
          return { json: scanJson({ incomplete: scans === 1 }) };
        },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Scan the mod' }));
    expect(await screen.findByText('The scan is incomplete')).toBeInTheDocument();
    expect(screen.getByText(/Treat anything missing as unchecked/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Scan again' }));
    await waitFor(() => expect(screen.queryByText('The scan is incomplete')).not.toBeInTheDocument());
    expect(sent(requests, 'POST', 'scan')).toHaveLength(2);
  });

  it('warns when no excerpt is approved and still needs the explicit button', async () => {
    const { requests } = mockApi(
      setupRoutes(statusJson('scanned', { scan: scanJson() }), {
        [`POST ${SETUP}/evidence`]: { json: { approved: 0 } },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Select none' }));
    expect(screen.getByText(/You are approving no excerpts/)).toBeInTheDocument();
    expect(sent(requests, 'POST', 'evidence')).toHaveLength(0);
    await user.click(screen.getByRole('button', { name: 'Continue without sending any excerpts' }));
    expect(await screen.findByText('You approved no excerpts. Claude works from the scan facts only.')).toBeInTheDocument();
    expect(sent(requests, 'POST', 'evidence')[0]?.body).toEqual({ scan_id: 'scan_1', references: [] });
  });

  it('starts the wizard from a newly created mod, which needs no excerpts', async () => {
    const { requests } = mockApi(
      setupRoutes(NONE, {
        [`POST ${SETUP}/scan`]: {
          json: scanJson({
            mode: 'create',
            evidence: { required: false, truncated: false, candidates: [] },
          }),
        },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText(/What is the mod about/), 'A focus tree');
    await user.click(screen.getByRole('button', { name: 'Create the mod' }));
    expect(await screen.findByRole('button', { name: 'Analyze with Claude' })).toBeInTheDocument();
    expect(sent(requests, 'POST', 'scan')[0]?.body).toEqual({ mode: 'create', description: 'A focus tree' });
    expect(screen.queryByRole('heading', { name: /Approve excerpts/ })).not.toBeInTheDocument();
    expect(currentStep()).toHaveTextContent('2. Analyze');
  });

  it('restores the step after a reload from the server status', async () => {
    mockApi(setupRoutes(confirmed));
    renderApp('/projects/prj_1');
    expect(await screen.findByRole('button', { name: 'Build the plan' })).toBeInTheDocument();
    expect(currentStep()).toHaveTextContent('5. Plan');
    expect(screen.getByText(/Nothing has been written to the mod yet/)).toBeInTheDocument();
  });
});

describe('the analysis step', () => {
  it('shows progress while the long call runs and keeps going when it succeeds', async () => {
    const { fetchMock, requests } = mockApi(
      setupRoutes(scanned, { [`POST ${SETUP}/analyze`]: { json: analysisJson() } }),
    );
    const release = gate(fetchMock, '/setup/analyze');
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Analyze with Claude' }));

    const working = await screen.findByText(/Claude is analyzing the mod/);
    expect(working.closest('[role="status"]')).not.toBeNull();
    expect(screen.getByText(/may take a minute or two/)).toBeInTheDocument();
    expect(screen.getByText(/the analysis is not cancelled/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Analyzing…' })).toBeDisabled();
    expect(await screen.findByText('1 second so far', {}, { timeout: 3000 })).toBeInTheDocument();

    release();
    expect(await screen.findByRole('button', { name: 'Confirm exactly these values' })).toBeInTheDocument();
    expect(screen.queryByText(/so far/)).not.toBeInTheDocument();
    expect(sent(requests, 'POST', 'analyze')).toHaveLength(1);
  });

  it('says analysis is not configured when the server has no model', async () => {
    mockApi(
      setupRoutes(scanned, {
        [`POST ${SETUP}/analyze`]: {
          status: 501,
          json: envelope('capability_unavailable', 'No model.'),
        },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Analyze with Claude' }));
    expect(await screen.findByText(/Analysis is not configured on this server/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try analyzing again' })).toBeEnabled();
  });

  it('reports a failed model call and can try again', async () => {
    let calls = 0;
    mockApi(
      setupRoutes(scanned, {
        [`POST ${SETUP}/analyze`]: (): MockResponse => {
          calls += 1;
          return calls === 1
            ? {
                status: 502,
                json: envelope('analysis_model_failed', 'Bad model output.', {
                  retry_class: 'safe_same_inputs',
                }),
              }
            : { json: analysisJson() };
        },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Analyze with Claude' }));
    expect(await screen.findByText(/Claude could not produce a usable analysis/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Try analyzing again' }));
    expect(await screen.findByRole('button', { name: 'Confirm exactly these values' })).toBeInTheDocument();
    expect(screen.queryByText(/Claude could not produce/)).not.toBeInTheDocument();
  });

  it('offers a fresh scan when the excerpts were not approved on the server', async () => {
    mockApi(
      setupRoutes(scanned, {
        [`POST ${SETUP}/analyze`]: {
          status: 409,
          json: envelope('evidence_not_approved', 'Approve first.'),
        },
        [`POST ${SETUP}/scan`]: { json: scanJson() },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Analyze with Claude' }));
    expect(await screen.findByText(/excerpts were not approved on the server/)).toBeInTheDocument();
    // The first "Scan again" belongs to step 1; the one beside the error restarts too.
    const [, restart] = screen.getAllByRole('button', { name: 'Scan again' });
    await user.click(restart!);
    expect(
      await screen.findByRole('button', { name: 'Approve these 2 excerpts and send them to the model' }),
    ).toBeInTheDocument();
  });
});

describe('confirming values', () => {
  it('shows the field the server rejected next to the form and keeps the edits', async () => {
    mockApi(
      setupRoutes(analyzed, {
        [`POST ${SETUP}/confirm`]: {
          status: 422,
          json: envelope('invalid_request', 'script_prefix must be lowercase.', {
            affected_ids: { field: 'identity.script_prefix' },
          }),
        },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    const prefix = await screen.findByLabelText('Script prefix');
    await user.clear(prefix);
    await user.type(prefix, 'TM');
    await user.click(screen.getByRole('button', { name: 'Confirm exactly these values' }));
    expect(await screen.findByText('script_prefix must be lowercase.')).toBeInTheDocument();
    expect(screen.getByText(/Some values were not accepted/)).toBeInTheDocument();
    expect(screen.getByLabelText('Script prefix')).toHaveAttribute('aria-invalid', 'true');
    expect(screen.getByLabelText('Script prefix')).toHaveValue('TM');
  });

  it('lets the values be edited again until a plan exists', async () => {
    const { requests } = mockApi(
      setupRoutes(confirmed, {
        [`POST ${SETUP}/confirm`]: { json: { confirmed: true, values_sha256: '5'.repeat(64) } },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Edit the values' }));
    expect(currentStep()).toHaveTextContent('Confirm values');
    await user.click(screen.getByRole('button', { name: 'Confirm exactly these values' }));
    expect(await screen.findByRole('button', { name: 'Build the plan' })).toBeInTheDocument();
    expect(sent(requests, 'POST', 'confirm')).toHaveLength(1);
  });
});

describe('conflicts and approval', () => {
  it('gives a conflict decision a new plan hash and approves that hash, not the old one', async () => {
    const { requests } = mockApi(
      setupRoutes(
        statusJson('planned', {
          scan: scanJson(),
          analysis: analysisJson(),
          confirmed_values: reviewValues(),
          plan: conflictPlan(),
        }),
        {
          [`POST ${SETUP}/conflicts`]: {
            json: {
              plan: planJson({
                plan_sha256: HASH_B,
                conflicts: [{ path: 'descriptor.mod', options: ['keep', 'replace', 'rename'], selected: 'replace' }],
              }),
            },
          },
          [`POST ${SETUP}/approve`]: {
            json: { approval: { approval_id: 'ap_2', plan_sha256: HASH_B, root_identity: 'c'.repeat(64) } },
          },
          [`POST ${SETUP}/apply`]: { json: appliedJson() },
        },
      ),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();

    // Unresolved conflicts block approval: there is no approve button yet.
    expect(await screen.findByText(/1 conflict still needs a decision/)).toBeInTheDocument();
    expect(currentStep()).toHaveTextContent('5. Plan');
    expect(screen.queryByRole('button', { name: 'Approve and apply this exact plan' })).not.toBeInTheDocument();
    const decide = screen.getByRole('button', { name: 'Apply decisions' });
    expect(decide).toBeDisabled();
    expect(screen.getAllByTitle(HASH_A).length).toBeGreaterThan(0);

    await user.click(screen.getByRole('radio', { name: "Replace it with Setup's version" }));
    expect(decide).toBeEnabled();
    await user.click(decide);

    expect(await screen.findByText(/it now has a new hash \(bbbbbbbbbbbb\)/)).toBeInTheDocument();
    expect(screen.getByText(/any earlier approval was withdrawn/)).toBeInTheDocument();
    expect(sent(requests, 'POST', 'conflicts')[0]?.body).toEqual({
      plan_id: 'p1',
      decisions: [{ path: 'descriptor.mod', choice: 'replace' }],
    });
    expect(screen.getAllByTitle(HASH_B).length).toBeGreaterThan(0);
    expect(screen.queryAllByTitle(HASH_A)).toHaveLength(0);
    expect(currentStep()).toHaveTextContent('6. Approve and apply');

    await user.click(screen.getByRole('checkbox', { name: 'I reviewed this plan' }));
    await user.click(screen.getByRole('button', { name: 'Approve and apply this exact plan' }));
    expect(await screen.findByText('Done. The plan was applied.')).toBeInTheDocument();
    const approvals = sent(requests, 'POST', 'approve');
    expect(approvals).toHaveLength(1);
    expect(approvals[0]?.body).toEqual({ plan_id: 'p1', plan_sha256: HASH_B });
  });

  it('does not reuse an approval after a decision changes the plan', async () => {
    const approvedPlan = planJson({
      approved: true,
      conflicts: [{ path: 'descriptor.mod', options: ['keep', 'replace'], selected: 'keep' }],
    });
    const { requests } = mockApi(
      setupRoutes(
        statusJson('approved', {
          scan: scanJson(),
          analysis: analysisJson(),
          confirmed_values: reviewValues(),
          plan: approvedPlan,
        }),
        {
          [`POST ${SETUP}/conflicts`]: {
            json: {
              plan: planJson({
                plan_sha256: HASH_B,
                approved: false,
                conflicts: [{ path: 'descriptor.mod', options: ['keep', 'replace'], selected: 'replace' }],
              }),
            },
          },
          [`POST ${SETUP}/approve`]: {
            json: { approval: { approval_id: 'ap_3', plan_sha256: HASH_B, root_identity: 'c'.repeat(64) } },
          },
          [`POST ${SETUP}/apply`]: { json: appliedJson() },
        },
      ),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    expect(await screen.findByText('This exact plan is already approved.')).toBeInTheDocument();
    await user.click(screen.getByRole('radio', { name: "Replace it with Setup's version" }));
    await user.click(screen.getByRole('button', { name: 'Apply decisions' }));
    expect(await screen.findByText(/earlier approval was withdrawn/)).toBeInTheDocument();
    expect(screen.queryByText('This exact plan is already approved.')).not.toBeInTheDocument();
    expect(screen.getByText('This plan is not approved yet.')).toBeInTheDocument();
    await user.click(screen.getByRole('checkbox', { name: 'I reviewed this plan' }));
    await user.click(screen.getByRole('button', { name: 'Approve and apply this exact plan' }));
    expect(await screen.findByText('Done. The plan was applied.')).toBeInTheDocument();
    expect(sent(requests, 'POST', 'approve')[0]?.body).toEqual({ plan_id: 'p1', plan_sha256: HASH_B });
  });

  it('reloads the plan and asks for a new review when it changed since it was shown', async () => {
    let planReads = 0;
    const { requests } = mockApi(
      setupRoutes(
        statusJson('planned', {
          scan: scanJson(),
          analysis: analysisJson(),
          confirmed_values: reviewValues(),
          plan: planJson(),
        }),
        {
          [`POST ${SETUP}/approve`]: {
            status: 409,
            json: envelope('plan_changed', 'The plan changed.', { retry_class: 'refresh_preconditions' }),
          },
          [`GET ${SETUP}/plan`]: () => {
            planReads += 1;
            return { json: { plan: planJson({ plan_sha256: HASH_B }) } };
          },
        },
      ),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('checkbox', { name: 'I reviewed this plan' }));
    await user.click(screen.getByRole('button', { name: 'Approve and apply this exact plan' }));
    expect(await screen.findByText(/The plan changed since you reviewed it/)).toBeInTheDocument();
    expect(planReads).toBe(1);
    expect(screen.getAllByTitle(HASH_B).length).toBeGreaterThan(0);
    expect(screen.getByRole('checkbox', { name: 'I reviewed this plan' })).not.toBeChecked();
    expect(screen.getByRole('button', { name: 'Approve and apply this exact plan' })).toBeDisabled();
    expect(sent(requests, 'POST', 'apply')).toHaveLength(0);
  });
});

describe('a lost session', () => {
  const lost = (code: string) => ({
    status: 409,
    json: envelope(code, 'Gone.'),
  });

  it('asks for a new scan when the session ended while confirming', async () => {
    const { requests } = mockApi(
      setupRoutes(analyzed, {
        [`POST ${SETUP}/confirm`]: lost('setup_session_expired'),
        [`POST ${SETUP}/scan`]: { json: scanJson() },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Confirm exactly these values' }));
    expect(
      await screen.findByText(
        'Your setup session ended (the server restarted or it timed out). Scan the mod again to continue.',
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Confirm exactly these values' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Scan again' }));
    expect(await screen.findByText('مود')).toBeInTheDocument();
    expect(currentStep()).toHaveTextContent('2. Approve excerpts');
    expect(sent(requests, 'POST', 'scan')).toHaveLength(1);
    expect(
      screen.queryByText(/Your setup session ended/),
    ).not.toBeInTheDocument();
  });

  it('treats a missing session on analyze the same way', async () => {
    mockApi(setupRoutes(scanned, { [`POST ${SETUP}/analyze`]: lost('setup_session_missing') }));
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Analyze with Claude' }));
    expect(await screen.findByText(/Your setup session ended/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Scan again' })).toBeInTheDocument();
  });

  it('shows the same prompt when the status says the session expired', async () => {
    mockApi(setupRoutes(statusJson('expired')));
    renderApp('/projects/prj_1');
    expect(await screen.findByText(/Your setup session ended/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Scan again' })).toBeInTheDocument();
  });
});

describe('recovery', () => {
  it('offers the one allowed action, asks to confirm, and shows the outcome', async () => {
    let recovered = false;
    const { requests } = mockApi(
      setupRoutes(confirmed, {
        [`GET ${SETUP}/recovery`]: () => ({ json: { recovery: recovered ? null : recoveryJson() } }),
        [`POST ${SETUP}/recover`]: () => {
          recovered = true;
          return {
            json: {
              recovery: {
                action: 'rollback',
                journal_state: 'rolled_back',
                restoration: {
                  exact: false,
                  differences: {
                    added: [],
                    removed: [],
                    changed: ['descriptor.mod'],
                    mode_changed: [],
                    git_index_changed: false,
                  },
                },
              },
            },
          };
        },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();

    const panel = (await screen.findByRole('heading', { name: 'An earlier change was not finished' })).closest('section')!;
    expect(within(panel).getByText('The apply stopped after changing two files.')).toBeInTheDocument();
    expect(within(panel).getByText('applying')).toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Resume the apply' })).not.toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Discard the unfinished files' })).not.toBeInTheDocument();

    // Planning is paused while a recovery is pending.
    expect(screen.getByRole('button', { name: 'Build the plan' })).toBeDisabled();
    expect(screen.getByText(/has to be recovered first, so planning is paused/)).toBeInTheDocument();

    await user.click(within(panel).getByRole('button', { name: 'Roll back the change' }));
    expect(sent(requests, 'POST', 'recover')).toHaveLength(0);
    expect(screen.getByText(/Roll back the interrupted change\?/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Yes, roll back' }));

    expect(await screen.findByText('Recovery finished')).toBeInTheDocument();
    expect(sent(requests, 'POST', 'recover')[0]?.body).toEqual({
      journal_id: 'j1',
      variant: 'rollback_after_failed_apply',
    });
    expect(screen.getByText(/not exactly as they were before/)).toBeInTheDocument();
    expect(screen.getByText('descriptor.mod')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'An earlier change was not finished' })).not.toBeInTheDocument();
  });

  it('offers no button when only a manual review is possible', async () => {
    mockApi(
      setupRoutes(NONE, {
        [`GET ${SETUP}/recovery`]: { json: { recovery: recoveryJson({ allowed_action: null }) } },
      }),
    );
    renderApp('/projects/prj_1');
    const heading = await screen.findByRole('heading', { name: 'An earlier change was not finished' });
    const panel = heading.closest('section')!;
    expect(within(panel).getByText(/A person has to review the mod and the journal by hand/)).toBeInTheDocument();
    expect(within(panel).queryAllByRole('button')).toHaveLength(0);
  });

  it('shows the recovery after an apply that needs a rollback', async () => {
    let applied = false;
    mockApi(
      setupRoutes(
        statusJson('planned', {
          scan: scanJson(),
          analysis: analysisJson(),
          confirmed_values: reviewValues(),
          plan: planJson(),
        }),
        {
          [`POST ${SETUP}/approve`]: {
            json: { approval: { approval_id: 'ap_1', plan_sha256: HASH_A, root_identity: 'c'.repeat(64) } },
          },
          [`POST ${SETUP}/apply`]: () => {
            applied = true;
            return {
              json: {
                status: 'needs_rollback',
                journal_id: 'j1',
                variant: 'rollback_after_failed_apply',
                explanation: 'A file could not be written.',
              },
            };
          },
          [`GET ${SETUP}/recovery`]: () => ({ json: { recovery: applied ? recoveryJson() : null } }),
        },
      ),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('checkbox', { name: 'I reviewed this plan' }));
    await user.click(screen.getByRole('button', { name: 'Approve and apply this exact plan' }));
    expect(await screen.findByRole('heading', { name: 'An earlier change was not finished' })).toBeInTheDocument();
    expect(screen.getByText(/Setup stopped part way/)).toBeInTheDocument();
    expect(screen.getByText(/A file could not be written\./)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Roll back the change' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Approve and apply this exact plan' })).toBeDisabled();
  });

  it('checks for an unfinished change after a network failure during apply', async () => {
    let applyStarted = false;
    const { fetchMock, requests } = mockApi(
      setupRoutes(
        statusJson('planned', {
          scan: scanJson(),
          analysis: analysisJson(),
          confirmed_values: reviewValues(),
          plan: planJson({ approved: true }),
        }),
        {
          [`GET ${SETUP}/recovery`]: () => ({ json: { recovery: applyStarted ? recoveryJson() : null } }),
        },
      ),
    );
    const original = fetchMock.getMockImplementation()!;
    fetchMock.mockImplementation(async (input, init) => {
      if (String(input).endsWith('/setup/apply')) {
        applyStarted = true;
        throw new TypeError('fetch failed');
      }
      return original(input, init);
    });
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('checkbox', { name: 'I reviewed this plan' }));
    await user.click(screen.getByRole('button', { name: 'Approve and apply this exact plan' }));
    expect(await screen.findByRole('heading', { name: 'An earlier change was not finished' })).toBeInTheDocument();
    expect(screen.getAllByText(/Could not reach the server/).length).toBeGreaterThan(0);
    const recoveryReads = requests.filter((r) => r.method === 'GET' && r.path === `${SETUP}/recovery`);
    expect(recoveryReads.length).toBeGreaterThanOrEqual(2);
  });
});

describe('planning without a plan builder', () => {
  it('says so calmly when the server cannot plan', async () => {
    mockApi(
      setupRoutes(confirmed, {
        [`POST ${SETUP}/plan`]: {
          status: 501,
          json: envelope('capability_unavailable', 'No planner.'),
        },
      }),
    );
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Build the plan' }));
    expect(await screen.findByText('This server does not have that part set up yet.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Build the plan' })).toBeEnabled();
  });
});

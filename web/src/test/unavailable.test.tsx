import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { account, mockApi, project, renderApp, signedIn } from './helpers';

const base = () => ({
  ...signedIn(),
  'GET /projects/prj_1': { json: { project: project() } },
});

describe('endpoints that are not built yet', () => {
  it('shows a friendly state, not an error, when the workspace route answers 404', async () => {
    mockApi(base());
    renderApp('/projects/prj_1');
    expect(await screen.findByText('Workspace status is not available yet')).toBeInTheDocument();
    // It is an information state: nothing is announced as a failure.
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('keeps the form and explains when creating a mod hits a missing route', async () => {
    mockApi(base());
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    const description = await screen.findByLabelText(/What is the mod about/);
    await user.type(description, "A focus tree for Côte d'Ivoire");
    await user.click(screen.getByRole('button', { name: 'Create the mod' }));
    expect(await screen.findByText('Creating a new mod is not available yet')).toBeInTheDocument();
    expect(screen.getByLabelText(/What is the mod about/)).toHaveValue("A focus tree for Côte d'Ivoire");
  });

  it('explains the missing upload route and keeps the three paths reachable', async () => {
    mockApi(base());
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    expect(await screen.findByRole('radio', { name: /Create new/ })).toBeChecked();
    await user.click(screen.getByRole('radio', { name: /Upload ZIP or folder/ }));
    const file = new File(['PK'], 'my-mod.zip', { type: 'application/zip' });
    await user.upload(screen.getByLabelText('Choose a ZIP file'), file);
    expect(screen.getByText(/1 file selected/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Upload' }));
    expect(await screen.findByText('Upload is not available yet')).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /Import from GitHub/ })).toBeInTheDocument();
  });

  it('asks to connect GitHub before listing repositories', async () => {
    mockApi(base());
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('radio', { name: /Import from GitHub/ }));
    expect(screen.getByRole('link', { name: 'Connect GitHub' })).toHaveAttribute(
      'href',
      '/api/v1/auth/github/start?link=1',
    );
  });

  it('says the repository list is not available yet when GitHub is connected', async () => {
    mockApi({
      ...base(),
      'GET /auth/me': { json: { account: account({ github_linked: true }) } },
    });
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('radio', { name: /Import from GitHub/ }));
    expect(await screen.findByText('GitHub import is not available yet')).toBeInTheDocument();
  });

  it('scans a mod that is present and shows the dry run before anything is applied', async () => {
    const { requests } = mockApi({
      ...base(),
      'GET /projects/prj_1/workspace': { json: { workspace: { has_source: true, origin: 'upload', file_count: 1234 } } },
      'POST /projects/prj_1/setup/scan': { json: { scan_id: 's1', facts: [{ label: 'Mod name', value: 'مود' }] } },
      'GET /projects/prj_1/setup/plan': {
        json: {
          plan: {
            plan_id: 'p1',
            operations: [
              { path: 'descriptor.mod', action: 'change' },
              { path: 'common/new.txt', action: 'create' },
              { path: 'map/provinces.bmp', action: 'preserve' },
            ],
          },
        },
      },
      'POST /projects/prj_1/setup/apply': { json: { status: 'applied' } },
    });
    renderApp('/projects/prj_1');
    const user = userEvent.setup();
    expect(await screen.findByText('1,234')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Scan the mod' }));
    expect(await screen.findByText('مود')).toHaveAttribute('dir', 'auto');
    await user.click(screen.getByRole('button', { name: 'Show what Setup would change' }));
    expect(await screen.findByText('descriptor.mod')).toBeInTheDocument();
    expect(requests.some((r) => r.path.endsWith('/setup/apply'))).toBe(false);
    await user.click(screen.getByRole('button', { name: 'Apply this plan' }));
    await user.click(screen.getByRole('button', { name: 'Yes, apply it' }));
    expect(await screen.findByText('Done. The plan was applied.')).toBeInTheDocument();
    expect(requests.find((r) => r.path.endsWith('/setup/apply'))?.body).toEqual({ plan_id: 'p1' });
  });
});

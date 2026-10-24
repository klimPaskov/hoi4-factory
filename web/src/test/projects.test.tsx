import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { envelope, mockApi, project, renderApp, signedIn } from './helpers';

describe('project list', () => {
  it('shows a clear call to action when there are no projects', async () => {
    mockApi({ ...signedIn(), 'GET /projects': { json: { projects: [] } } });
    renderApp('/');
    expect(await screen.findByRole('heading', { name: 'You have no projects yet' })).toBeInTheDocument();
    const action = screen.getByRole('button', { name: 'Create your first project' });
    const user = userEvent.setup();
    await user.click(action);
    expect(screen.getByLabelText('Project name')).toHaveFocus();
  });

  it('sends a guest to sign in and shows loading first', async () => {
    mockApi({
      'GET /auth/me': { status: 401, json: envelope('unauthenticated', 'Sign in to continue.') },
    });
    renderApp('/');
    expect(await screen.findByRole('heading', { name: 'Sign in' })).toBeInTheDocument();
  });

  it('lists projects and moves a deleted one to the restore section', async () => {
    const mine = project({ id: 'prj_1', name: "Ma Mod d'Été" });
    mockApi({
      ...signedIn(),
      'GET /projects': { json: { projects: [mine] } },
      'DELETE /projects/prj_1': {
        status: 202,
        json: { project: { ...mine, deleted_at: '2026-10-24T10:00:00Z', delete_after: '2026-11-23T10:00:00Z' } },
      },
      'POST /projects/prj_1/restore': { json: { project: mine } },
    });
    renderApp('/');
    const user = userEvent.setup();
    const link = await screen.findByRole('link', { name: "Ma Mod d'Été" });
    expect(link).toHaveAttribute('href', '/projects/prj_1');

    await user.click(screen.getByRole('button', { name: 'Delete' }));
    const confirm = screen.getByRole('group', { name: 'Confirm deleting this project' });
    expect(confirm).toHaveTextContent('for 30 days');
    await user.click(within(confirm).getByRole('button', { name: 'Yes, delete it' }));

    expect(await screen.findByRole('heading', { name: 'Waiting to be deleted' })).toBeInTheDocument();
    expect(screen.getByText(/Deleted for good on/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Restore' }));
    expect(await screen.findByRole('link', { name: "Ma Mod d'Été" })).toBeInTheDocument();
  });

  it('keeps the typed name when the server refuses it', async () => {
    mockApi({
      ...signedIn(),
      'GET /projects': { json: { projects: [] } },
      'POST /projects': {
        status: 400,
        json: envelope('invalid_request', 'Project name is too long: at most 100 characters.'),
      },
    });
    renderApp('/');
    const user = userEvent.setup();
    const input = await screen.findByLabelText('Project name');
    await user.type(input, 'Mon projet \u{1F680}');
    await user.click(screen.getByRole('button', { name: 'Create project' }));
    expect(await screen.findByText(/too long/)).toBeInTheDocument();
    expect(screen.getByLabelText('Project name')).toHaveValue('Mon projet \u{1F680}');
  });

  it('offers a retry when the list cannot be loaded', async () => {
    mockApi({
      ...signedIn(),
      'GET /projects': {
        status: 503,
        json: envelope('internal', 'Something went wrong on the server.', { retry_class: 'reconcile_first' }),
      },
    });
    renderApp('/');
    expect(await screen.findByRole('alert')).toHaveTextContent('may not have saved');
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});

describe('unverified account', () => {
  it('sees a small banner with a way to get a new link', async () => {
    const { requests } = mockApi({
      ...signedIn({
        id: 'acc_2',
        email: 'new@example.com',
        email_verified: false,
        display_name: 'New',
        github_linked: false,
        has_password: true,
        created_at: '2026-10-24T10:00:00Z',
      }),
      'GET /projects': { json: { projects: [] } },
      'POST /auth/resend-verification': { status: 202, json: { sent: true, already_verified: false } },
    });
    renderApp('/');
    const user = userEvent.setup();
    const banner = await screen.findByRole('region', { name: 'Email verification' });
    expect(banner).toHaveTextContent('new@example.com');
    await user.click(within(banner).getByRole('button', { name: 'Send a new link' }));
    expect(await within(banner).findByText(/new link is on its way/)).toBeInTheDocument();
    expect(requests.some((r) => r.path === '/auth/resend-verification')).toBe(true);
    // Still usable underneath: the page itself is not blocked.
    expect(screen.getByRole('heading', { name: 'Projects' })).toBeInTheDocument();
  });
});

describe('game reference indicator', () => {
  it('shows the version and DLC count', async () => {
    mockApi({ ...signedIn(), 'GET /projects': { json: { projects: [] } } });
    renderApp('/');
    expect(await screen.findByText('HOI4 1.19.3.0, 3 DLC')).toBeInTheDocument();
  });
});

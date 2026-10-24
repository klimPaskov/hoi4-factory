import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { ApiError, NETWORK_ERROR } from '../api/client';
import { describeError } from '../errors';
import { SIGNED_OUT, envelope, mockApi, renderApp } from './helpers';

async function submitSignIn(email: string, password: string) {
  const user = userEvent.setup();
  await user.type(await screen.findByLabelText('Email address'), email);
  await user.type(screen.getByLabelText('Password'), password);
  await user.click(screen.getByRole('button', { name: 'Sign in' }));
}

describe('sign-in error mapping', () => {
  it('explains bad credentials in plain words and keeps what was typed', async () => {
    mockApi({
      ...SIGNED_OUT,
      'POST /auth/signin': {
        status: 401,
        json: envelope('invalid_credentials', 'The email address or password is not correct.'),
      },
    });
    renderApp('/signin');
    await submitSignIn('modder@example.com', 'wrong password');

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('That email and password do not match');
    expect(alert).toHaveTextContent('reset your password');
    // The server's own wording is not what the user sees.
    expect(alert).not.toHaveTextContent('The email address or password is not correct.');
    expect(screen.getByLabelText('Email address')).toHaveValue('modder@example.com');
    expect(screen.getByLabelText('Password')).toHaveValue('wrong password');
  });

  it('tells the user how long to wait when rate limited', async () => {
    mockApi({
      ...SIGNED_OUT,
      'POST /auth/signin': {
        status: 429,
        headers: { 'Retry-After': '12' },
        json: envelope('rate_limited', 'Too many attempts.', { retry_class: 'safe_same_inputs' }),
      },
    });
    renderApp('/signin');
    await submitSignIn('modder@example.com', 'whatever pass');
    expect(await screen.findByRole('alert')).toHaveTextContent('Wait 12 seconds and try again.');
  });

  it('reports an unreachable server and keeps the form', async () => {
    const { fetchMock } = mockApi({ ...SIGNED_OUT });
    renderApp('/signin');
    await screen.findByLabelText('Email address');
    fetchMock.mockRejectedValueOnce(new TypeError('Failed to fetch'));
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Email address'), 'modder@example.com');
    await user.type(screen.getByLabelText('Password'), 'some password');
    await user.click(screen.getByRole('button', { name: 'Sign in' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Check your connection');
    expect(screen.getByLabelText('Email address')).toHaveValue('modder@example.com');
  });

  it('checks empty fields before calling the server', async () => {
    const { requests } = mockApi({ ...SIGNED_OUT });
    renderApp('/signin');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Sign in' }));
    expect(await screen.findByText('Enter your email address.')).toBeInTheDocument();
    expect(screen.getByText('Enter your password.')).toBeInTheDocument();
    expect(requests.some((r) => r.path === '/auth/signin')).toBe(false);
  });

  it('shows a GitHub failure left in the address', async () => {
    mockApi({ ...SIGNED_OUT });
    renderApp('/signin?error=github_denied');
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('GitHub sign-in was cancelled'));
  });

  it('links the GitHub button straight to the server', async () => {
    mockApi({ ...SIGNED_OUT });
    renderApp('/signin');
    const link = await screen.findByRole('link', { name: 'Continue with GitHub' });
    expect(link).toHaveAttribute('href', '/api/v1/auth/github/start');
  });
});

describe('describeError', () => {
  const make = (code: string, extra: Partial<ConstructorParameters<typeof ApiError>[0]> = {}) =>
    new ApiError({ status: 400, code, message: 'server text', ...extra });

  it.each([
    ['unauthenticated', 'Your session has ended'],
    ['email_taken', 'already exists'],
    ['email_not_verified', 'Verify your email address first'],
    ['forbidden', 'does not allow that'],
    ['not_found', 'could not find that'],
    ['grace_period_over', '30 days'],
    ['github_state_mismatch', 'Start again'],
    ['invalid_token', 'not valid any more'],
    [NETWORK_ERROR, 'Check your connection'],
  ])('maps %s to guidance', (code, expected) => {
    expect(describeError(make(code)).message).toContain(expected);
  });

  it('uses the retry class for a code it does not know', () => {
    const described = describeError(make('brand_new_code', { retryClass: 'reconcile_first' }));
    expect(described.message).toContain('may or may not have saved');
    expect(described.canRetry).toBe(true);
  });

  it('shows the server wording for input problems, which names the exact field', () => {
    const described = describeError(make('invalid_request', { message: 'Use a password of at least 10 characters.' }));
    expect(described.message).toBe('Use a password of at least 10 characters.');
  });

  it('treats a bare 502 from a proxy as a server problem', () => {
    const described = describeError(new ApiError({ status: 502, code: 'http_502', message: 'Bad Gateway' }));
    expect(described.message).toContain('server had a problem');
    expect(described.canRetry).toBe(true);
  });
});

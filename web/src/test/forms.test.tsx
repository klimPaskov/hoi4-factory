import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { SIGNED_OUT, envelope, mockApi, renderApp } from './helpers';

const NAME = "Zoë O'Brien \u{1F600} مرحبا";

describe('forms keep what was typed when the server rejects a field', () => {
  it('sign-up: a 422 on one field leaves every field as typed and marks the right one', async () => {
    const { requests } = mockApi({
      ...SIGNED_OUT,
      'POST /auth/signup': {
        status: 422,
        json: envelope('validation_failed', 'That address belongs to a domain we do not accept.', {
          affected_ids: { field: 'email' },
        }),
      },
    });
    renderApp('/signup');
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText('Email address'), 'zoe@example.com');
    await user.type(screen.getByLabelText(/Display name/), NAME);
    await user.type(screen.getByLabelText('Password'), 'correct horse battery');
    await user.click(screen.getByRole('button', { name: 'Create account' }));

    const email = await screen.findByLabelText('Email address');
    expect(email).toHaveAttribute('aria-invalid', 'true');
    expect(email).toHaveAccessibleDescription(/domain we do not accept/);
    expect(email).toHaveFocus();

    expect(email).toHaveValue('zoe@example.com');
    expect(screen.getByLabelText(/Display name/)).toHaveValue(NAME);
    expect(screen.getByLabelText('Password')).toHaveValue('correct horse battery');

    // Apostrophes, accents, emoji and right-to-left text reach the server byte for byte.
    const sent = requests.find((r) => r.path === '/auth/signup');
    expect(sent?.body).toEqual({
      email: 'zoe@example.com',
      password: 'correct horse battery',
      display_name: NAME,
    });
    // User-written text reads in its own direction.
    expect(screen.getByLabelText(/Display name/)).toHaveAttribute('dir', 'auto');
  });

  it('sign-up: a field problem without a field name is placed by its message', async () => {
    mockApi({
      ...SIGNED_OUT,
      'POST /auth/signup': {
        status: 400,
        json: envelope('invalid_request', 'Enter a valid email address.'),
      },
    });
    renderApp('/signup');
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText('Email address'), 'a@b');
    await user.type(screen.getByLabelText('Password'), 'long enough password');
    await user.click(screen.getByRole('button', { name: 'Create account' }));
    const email = await screen.findByLabelText('Email address');
    expect(email).toHaveAccessibleDescription('Enter a valid email address.');
    expect(screen.getByLabelText('Password')).toHaveValue('long enough password');
  });

  it('sign-up: a short password is caught before any request', async () => {
    const { requests } = mockApi({ ...SIGNED_OUT });
    renderApp('/signup');
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText('Email address'), 'a@example.com');
    await user.type(screen.getByLabelText('Password'), 'short');
    await user.click(screen.getByRole('button', { name: 'Create account' }));
    expect(await screen.findByText('Use a password of at least 10 characters.')).toBeInTheDocument();
    expect(requests.some((r) => r.path === '/auth/signup')).toBe(false);
  });

  it('password reset: a rejected token keeps the typed passwords and offers a new link', async () => {
    mockApi({
      ...SIGNED_OUT,
      'POST /auth/password-reset/confirm': {
        status: 400,
        json: envelope('invalid_token', 'This link is invalid or has expired. Request a new one.', {
          retry_class: 'requires_authorization',
        }),
      },
    });
    renderApp('/reset-password?token=abc');
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText('New password'), 'a brand new passphrase');
    await user.type(screen.getByLabelText('Repeat the new password'), 'a brand new passphrase');
    await user.click(screen.getByRole('button', { name: 'Save new password' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('not valid any more');
    expect(screen.getByRole('link', { name: 'Ask for a new reset link' })).toBeInTheDocument();
    expect(screen.getByLabelText('New password')).toHaveValue('a brand new passphrase');
  });
});

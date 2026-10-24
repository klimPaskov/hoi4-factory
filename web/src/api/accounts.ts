// Accounts API. All of these endpoints exist in crates/accounts (routes under /api/v1).
import { request } from './client';
import type { Account, Project, Visibility } from './types';

export const GITHUB_SIGN_IN_URL = '/api/v1/auth/github/start';
export const GITHUB_LINK_URL = '/api/v1/auth/github/start?link=1';

export async function getMe(signal?: AbortSignal): Promise<Account> {
  const data = await request<{ account: Account }>('/auth/me', { signal });
  return data.account;
}

export async function signUp(input: {
  email: string;
  password: string;
  display_name?: string;
}): Promise<{ account: Account; verification_email_sent: boolean }> {
  return request('/auth/signup', { method: 'POST', body: input });
}

export async function signIn(input: { email: string; password: string }): Promise<Account> {
  const data = await request<{ account: Account }>('/auth/signin', { method: 'POST', body: input });
  return data.account;
}

export async function signOut(): Promise<void> {
  await request<void>('/auth/signout', { method: 'POST' });
}

export async function signOutEverywhere(): Promise<{ sessions_ended: number }> {
  return request('/auth/signout-all', { method: 'POST' });
}

export async function verifyEmail(token: string): Promise<Account> {
  const data = await request<{ account: Account }>('/auth/verify-email', {
    method: 'POST',
    body: { token },
  });
  return data.account;
}

export async function resendVerification(): Promise<{ sent: boolean; already_verified: boolean }> {
  return request('/auth/resend-verification', { method: 'POST' });
}

export async function requestPasswordReset(email: string): Promise<void> {
  await request('/auth/password-reset/request', { method: 'POST', body: { email } });
}

export async function confirmPasswordReset(token: string, newPassword: string): Promise<void> {
  await request('/auth/password-reset/confirm', {
    method: 'POST',
    body: { token, new_password: newPassword },
  });
}

export async function listProjects(
  includeDeleted: boolean,
  signal?: AbortSignal,
): Promise<Project[]> {
  const query = includeDeleted ? '?include_deleted=true' : '';
  const data = await request<{ projects: Project[] }>(`/projects${query}`, { signal });
  return data.projects;
}

export async function getProject(id: string, signal?: AbortSignal): Promise<Project> {
  const data = await request<{ project: Project }>(`/projects/${encodeURIComponent(id)}`, {
    signal,
  });
  return data.project;
}

export async function createProject(
  name: string,
  visibility: Visibility = 'private',
): Promise<Project> {
  const data = await request<{ project: Project }>('/projects', {
    method: 'POST',
    body: { name, visibility },
  });
  return data.project;
}

export async function renameProject(id: string, name: string): Promise<Project> {
  const data = await request<{ project: Project }>(`/projects/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    body: { name },
  });
  return data.project;
}

/** Marks the project for deletion. It stays restorable until `delete_after`. */
export async function deleteProject(id: string): Promise<Project> {
  const data = await request<{ project: Project }>(`/projects/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
  return data.project;
}

export async function restoreProject(id: string): Promise<Project> {
  const data = await request<{ project: Project }>(
    `/projects/${encodeURIComponent(id)}/restore`,
    { method: 'POST' },
  );
  return data.project;
}

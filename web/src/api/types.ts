// Shapes returned by the accounts API (crates/accounts). Dates are RFC 3339 strings.

export type Role = 'owner' | 'maintainer' | 'contributor' | 'viewer';
export type Visibility = 'private' | 'public';

export interface Account {
  id: string;
  email: string;
  email_verified: boolean;
  display_name: string;
  github_linked: boolean;
  has_password: boolean;
  created_at: string;
}

export interface Project {
  id: string;
  name: string;
  visibility: Visibility;
  created_at: string;
  /** Set while the project waits out its 30-day deletion grace period. */
  deleted_at: string | null;
  /** End of the grace period. */
  delete_after: string | null;
  /** The caller's role; null for read-only access to a public project. */
  role: Role | null;
}

export type RetryClass =
  | 'safe_same_inputs'
  | 'refresh_preconditions'
  | 'reconcile_first'
  | 'requires_authorization'
  | 'terminal';

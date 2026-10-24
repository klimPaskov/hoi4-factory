import { ApiError, NETWORK_ERROR } from './api/client';
import i18n from './i18n';

export interface DescribedError {
  /** What went wrong and what to do next, in the user's words. */
  message: string;
  /** Extra detail from the server, shown smaller. Only when it adds something. */
  detail?: string;
  /** Repeating the same request can succeed (or re-reading state is the next step). */
  canRetry: boolean;
  /** The user has to sign in before anything else helps. */
  needsSignIn: boolean;
}

/** Codes whose server message names the exact problem with the input, so it is worth showing. */
const SERVER_MESSAGE_CODES = new Set(['invalid_request', 'validation_failed']);

function status5xx(status: number): boolean {
  return status >= 500 && status <= 599;
}

/**
 * Maps an error to words the user can act on. The code comes from the error envelope; the
 * translation file has one entry per known code and one per retry class as the fallback.
 */
export function describeError(error: unknown): DescribedError {
  if (!(error instanceof ApiError)) {
    return { message: i18n.t('errors.unexpected'), canRetry: true, needsSignIn: false };
  }
  const options = { seconds: error.retryAfterSeconds ?? 30, count: error.retryAfterSeconds ?? 30 };
  const codeKey = `errors.code.${error.code}`;
  const needsSignIn = error.code === 'unauthenticated';
  const canRetry =
    error.code === NETWORK_ERROR ||
    error.retryClass === 'safe_same_inputs' ||
    error.retryClass === 'refresh_preconditions' ||
    error.retryClass === 'reconcile_first' ||
    status5xx(error.status);

  if (SERVER_MESSAGE_CODES.has(error.code) || error.status === 422) {
    return {
      message: error.message || i18n.t('errors.code.invalid_request'),
      canRetry: false,
      needsSignIn: false,
    };
  }
  if (i18n.exists(codeKey, options)) {
    return { message: i18n.t(codeKey, options), canRetry, needsSignIn };
  }
  if (status5xx(error.status) && !error.retryClass) {
    return { message: i18n.t('errors.code.http_5xx'), canRetry: true, needsSignIn: false };
  }
  const classKey = `errors.class.${error.retryClass ?? 'unknown'}`;
  const known = error.message && error.message !== 'Not found.' ? error.message : undefined;
  return {
    message: i18n.t(i18n.exists(classKey) ? classKey : 'errors.class.unknown'),
    detail: known,
    canRetry,
    needsSignIn,
  };
}

/**
 * Which form field a rejected request is about. The server names it in `affected_ids.field`
 * when it can. Until every endpoint does, the message text is matched against `patterns`.
 */
export function fieldForError<F extends string>(
  error: unknown,
  patterns: Record<F, RegExp>,
): F | null {
  if (!(error instanceof ApiError)) return null;
  const named = error.affectedIds.field;
  if (named && named in patterns) return named as F;
  if (error.code === 'email_taken' || error.code === 'account_exists') {
    return 'email' in patterns ? ('email' as F) : null;
  }
  if (SERVER_MESSAGE_CODES.has(error.code) || error.status === 422) {
    for (const [field, pattern] of Object.entries<RegExp>(patterns)) {
      if (pattern.test(error.message)) return field as F;
    }
  }
  return null;
}

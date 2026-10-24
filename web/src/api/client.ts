import type { RetryClass } from './types';

export const API_PREFIX = '/api/v1';

/** The common error envelope: `{ error: { code, message, retry_class, affected_ids, evidence_refs } }`. */
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly retryClass: RetryClass | null;
  readonly affectedIds: Record<string, string>;
  readonly evidenceRefs: unknown[];
  readonly retryAfterSeconds: number | null;

  constructor(init: {
    status: number;
    code: string;
    message: string;
    retryClass?: RetryClass | null;
    affectedIds?: Record<string, string>;
    evidenceRefs?: unknown[];
    retryAfterSeconds?: number | null;
  }) {
    super(init.message);
    this.name = 'ApiError';
    this.status = init.status;
    this.code = init.code;
    this.retryClass = init.retryClass ?? null;
    this.affectedIds = init.affectedIds ?? {};
    this.evidenceRefs = init.evidenceRefs ?? [];
    this.retryAfterSeconds = init.retryAfterSeconds ?? null;
  }

  /** The endpoint is not built yet (or the route is unknown to this server build). */
  get isUnavailable(): boolean {
    return this.status === 404 || this.status === 405 || this.status === 501;
  }
}

/** Code used when the request never reached the server. */
export const NETWORK_ERROR = 'network_error';

type Listener = () => void;
let unauthenticatedListener: Listener | null = null;

/** The auth provider registers here so a 401 on any call returns the app to signed-out. */
export function onUnauthenticated(listener: Listener | null): void {
  unauthenticatedListener = listener;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function stringMap(value: unknown): Record<string, string> {
  const out: Record<string, string> = {};
  if (isRecord(value)) {
    for (const [key, v] of Object.entries(value)) {
      if (typeof v === 'string') out[key] = v;
    }
  }
  return out;
}

async function toApiError(response: Response): Promise<ApiError> {
  let body: unknown = null;
  try {
    body = await response.json();
  } catch {
    body = null;
  }
  const envelope = isRecord(body) && isRecord(body.error) ? body.error : null;
  const retryAfter = Number(response.headers.get('Retry-After'));
  return new ApiError({
    status: response.status,
    code: typeof envelope?.code === 'string' ? envelope.code : `http_${response.status}`,
    message: typeof envelope?.message === 'string' ? envelope.message : response.statusText,
    retryClass:
      typeof envelope?.retry_class === 'string' ? (envelope.retry_class as RetryClass) : null,
    affectedIds: stringMap(envelope?.affected_ids),
    evidenceRefs: Array.isArray(envelope?.evidence_refs) ? envelope.evidence_refs : [],
    retryAfterSeconds: Number.isFinite(retryAfter) && retryAfter > 0 ? retryAfter : null,
  });
}

export interface RequestOptions {
  method?: 'GET' | 'POST' | 'PATCH' | 'DELETE';
  /** JSON body. */
  body?: unknown;
  /** Multipart body; the browser sets the boundary. */
  form?: FormData;
  signal?: AbortSignal;
}

/** One typed round trip to the server. Resolves with the parsed JSON body (or undefined for 204). */
export async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const headers: Record<string, string> = { Accept: 'application/json' };
  let body: BodyInit | undefined;
  if (options.form) {
    body = options.form;
  } else if (options.body !== undefined) {
    headers['Content-Type'] = 'application/json';
    body = JSON.stringify(options.body);
  }
  let response: Response;
  try {
    response = await fetch(`${API_PREFIX}${path}`, {
      method: options.method ?? 'GET',
      headers,
      body,
      credentials: 'same-origin',
      signal: options.signal,
    });
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === 'AbortError') throw cause;
    throw new ApiError({
      status: 0,
      code: NETWORK_ERROR,
      message: 'The server could not be reached.',
      retryClass: 'safe_same_inputs',
    });
  }
  if (!response.ok) {
    const error = await toApiError(response);
    if (response.status === 401 && error.code === 'unauthenticated') unauthenticatedListener?.();
    throw error;
  }
  if (response.status === 204) return undefined as T;
  const text = await response.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

export function isAbort(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError';
}

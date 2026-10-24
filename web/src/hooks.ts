import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import { isAbort } from './api/client';

export type AsyncState<T> =
  | { status: 'loading'; data: null; error: null }
  | { status: 'ready'; data: T; error: null }
  | { status: 'error'; data: null; error: unknown };

/**
 * Runs `load` on mount and again on `reload()`. The state starts as loading, so give the
 * consuming component a `key` that changes with whatever the load depends on.
 */
export function useAsync<T>(load: (signal: AbortSignal) => Promise<T>) {
  const loadRef = useRef(load);
  useEffect(() => {
    loadRef.current = load;
  });
  const [state, setState] = useState<AsyncState<T>>({
    status: 'loading',
    data: null,
    error: null,
  });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    loadRef.current(controller.signal).then(
      (data) => {
        if (!controller.signal.aborted) setState({ status: 'ready', data, error: null });
      },
      (error: unknown) => {
        if (controller.signal.aborted || isAbort(error)) return;
        setState({ status: 'error', data: null, error });
      },
    );
    return () => controller.abort();
  }, [attempt]);

  const reload = useCallback(() => {
    setState({ status: 'loading', data: null, error: null });
    setAttempt((n) => n + 1);
  }, []);

  const replace = useCallback((data: T) => {
    setState({ status: 'ready', data, error: null });
  }, []);

  return { ...state, reload, replace };
}

/** Sets the browser tab title for the current page. */
export function usePageTitle(title: string): void {
  useEffect(() => {
    const previous = document.title;
    document.title = title ? `${title} · HOI4 Factory` : 'HOI4 Factory';
    return () => {
      document.title = previous;
    };
  }, [title]);
}

/**
 * After a failed submit, moves focus to the first invalid field, or to the form-level alert.
 * Bump `version` once per failed submit.
 */
export function useFocusOnError(formRef: RefObject<HTMLElement | null>, version: number): void {
  useEffect(() => {
    if (version === 0) return;
    const form = formRef.current;
    if (!form) return;
    const target = form.querySelector<HTMLElement>(
      '[aria-invalid="true"], [data-form-error]',
    );
    target?.focus();
  }, [formRef, version]);
}

export type ThemeChoice = 'system' | 'light' | 'dark';
const THEME_KEY = 'hf-theme';

export function readTheme(): ThemeChoice {
  try {
    const stored = window.localStorage.getItem(THEME_KEY);
    if (stored === 'light' || stored === 'dark') return stored;
  } catch {
    // Storage can be blocked. The system theme is a fine default.
  }
  return 'system';
}

export function applyTheme(choice: ThemeChoice): void {
  const root = document.documentElement;
  if (choice === 'system') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', choice);
  try {
    if (choice === 'system') window.localStorage.removeItem(THEME_KEY);
    else window.localStorage.setItem(THEME_KEY, choice);
  } catch {
    // Not persisted; still applied for this visit.
  }
}

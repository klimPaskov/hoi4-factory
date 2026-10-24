import { useId, type ReactNode, type Ref } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { describeError } from '../errors';

// ----- Messages --------------------------------------------------------------------------

export function Alert({
  kind,
  children,
  focusable,
}: {
  kind: 'error' | 'success' | 'info' | 'warning';
  children: ReactNode;
  /** Marks the alert as the place focus moves to after a failed submit. */
  focusable?: boolean;
}) {
  return (
    <div
      className={`alert alert-${kind}`}
      role={kind === 'error' ? 'alert' : 'status'}
      tabIndex={focusable ? -1 : undefined}
      data-form-error={focusable ? '' : undefined}
    >
      {children}
    </div>
  );
}

/** A failed request, in words the user can act on. */
export function ErrorMessage({
  error,
  onRetry,
  focusable,
}: {
  error: unknown;
  onRetry?: () => void;
  focusable?: boolean;
}) {
  const { t } = useTranslation();
  const described = describeError(error);
  return (
    <Alert kind="error" focusable={focusable}>
      <p>{described.message}</p>
      {described.detail ? <p className="muted small">{described.detail}</p> : null}
      {described.needsSignIn ? (
        <p>
          <Link to="/signin">{t('common.signIn')}</Link>
        </p>
      ) : null}
      {onRetry && described.canRetry ? (
        <p>
          <button type="button" className="button secondary" onClick={onRetry}>
            {t('common.tryAgain')}
          </button>
        </p>
      ) : null}
    </Alert>
  );
}

export function Loading({ label }: { label?: string }) {
  const { t } = useTranslation();
  return (
    <p className="loading" role="status">
      <span className="spinner" aria-hidden="true" />
      {label ?? t('common.loading')}
    </p>
  );
}

export function EmptyState({
  title,
  children,
  action,
}: {
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <h2>{title}</h2>
      {children ? <p>{children}</p> : null}
      {action}
    </div>
  );
}

/** Shown when an endpoint answers 404 because that part of the server is not built yet. */
export function NotAvailableYet({ what }: { what: string }) {
  const { t } = useTranslation();
  return (
    <div className="unavailable" role="status">
      <p>
        <strong>{t('unavailable.title', { what })}</strong>
      </p>
      <p className="muted">{t('unavailable.body')}</p>
    </div>
  );
}

// ----- Form fields -----------------------------------------------------------------------

interface FieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: 'text' | 'email' | 'password' | 'search';
  autoComplete?: string;
  hint?: string;
  error?: string;
  /** Shown beside the label when the field may stay empty. */
  optional?: boolean;
  multiline?: boolean;
  /** `auto` lets right-to-left text read correctly. Use it for anything a person writes. */
  dir?: 'auto' | 'ltr';
  inputRef?: Ref<HTMLInputElement>;
  maxLength?: number;
  spellCheck?: boolean;
  rows?: number;
}

export function Field({
  label,
  value,
  onChange,
  type = 'text',
  autoComplete,
  hint,
  error,
  optional,
  multiline,
  dir = 'auto',
  inputRef,
  maxLength,
  spellCheck,
  rows = 3,
}: FieldProps) {
  const { t } = useTranslation();
  const id = useId();
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;
  const describedBy = [hint ? hintId : null, error ? errorId : null].filter(Boolean).join(' ');
  const invalid = error ? (true as const) : undefined;
  const described = describedBy || undefined;
  return (
    <div className={`field${error ? ' has-error' : ''}`}>
      <label htmlFor={id}>
        {label}
        {optional ? <span className="muted"> {t('common.optional')}</span> : null}
      </label>
      {hint ? (
        <p id={hintId} className="hint">
          {hint}
        </p>
      ) : null}
      {multiline ? (
        <textarea
          id={id}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          dir={dir}
          autoComplete={autoComplete}
          maxLength={maxLength}
          spellCheck={spellCheck}
          rows={rows}
          aria-invalid={invalid}
          aria-describedby={described}
        />
      ) : (
        <input
          id={id}
          ref={inputRef}
          type={type}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          dir={dir}
          autoComplete={autoComplete}
          maxLength={maxLength}
          spellCheck={spellCheck}
          aria-invalid={invalid}
          aria-describedby={described}
        />
      )}
      {error ? (
        <p id={errorId} className="field-error">
          {error}
        </p>
      ) : null}
    </div>
  );
}

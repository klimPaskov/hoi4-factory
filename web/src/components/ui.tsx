import { useEffect, useId, useRef, type ButtonHTMLAttributes, type ReactNode, type Ref } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { describeError } from '../errors';
import type { IconName } from './iconNames';

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
  icon,
}: {
  title: string;
  children?: ReactNode;
  action?: ReactNode;
  /** Shows the folder illustration above the title. */
  icon?: boolean;
}) {
  return (
    <div className="empty">
      {icon ? (
        <svg className="empty-icon" viewBox="0 0 48 48" aria-hidden="true" focusable="false">
          <path d="M6 14a4 4 0 0 1 4-4h10l4 4h14a4 4 0 0 1 4 4v18a4 4 0 0 1-4 4H10a4 4 0 0 1-4-4z" />
          <path className="empty-icon-accent" d="M23 21h2v5h5v2h-5v5h-2v-5h-5v-2h5z" />
        </svg>
      ) : null}
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

// ----- Brand -----------------------------------------------------------------------------

/**
 * The HOI4 Factory mark: a sawtooth factory roof in a rounded-square outline, drawn in the text
 * color (near-black on light, near-white on dark) with no fill, so the theme's background shows
 * through. Decorative, the name sits beside it.
 */
export function BrandMark() {
  return (
    <svg className="brand-mark" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
      <rect className="brand-mark-frame" x="1.25" y="1.25" width="29.5" height="29.5" rx="7.5" />
      <path className="brand-mark-fg" d="M7 24.5V15.5l4.7-3.6v3.6l4.7-3.6v3.6l4.7-3.6V8h3.9v16.5z" />
    </svg>
  );
}

// ----- Buttons ---------------------------------------------------------------------------

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'danger-outline';

export function Button({
  variant = 'secondary',
  size = 'md',
  block,
  icon,
  className,
  type = 'button',
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: 'md' | 'sm';
  block?: boolean;
  icon?: IconName;
  ref?: Ref<HTMLButtonElement>;
}) {
  const classes = ['button', variant, size === 'sm' ? 'small-button' : '', block ? 'block' : '', className ?? '']
    .filter(Boolean)
    .join(' ');
  return (
    <button type={type} className={classes} {...rest}>
      {icon ? <Icon name={icon} size="sm" /> : null}
      {children}
    </button>
  );
}

/** One of the generated icons. Decorative: whatever it sits beside carries the meaning. */
export function Icon({ name, size }: { name: IconName; size?: 'sm' | 'lg' }) {
  return <span className={`icon icon-${name}${size ? ` icon-${size}` : ''}`} aria-hidden="true" />;
}

export function ButtonRow({ children }: { children: ReactNode }) {
  return <div className="button-row">{children}</div>;
}

// ----- Page structure --------------------------------------------------------------------

/** The top of a page: an optional way back, the one h1, a short line under it and actions. */
export function PageHeader({
  title,
  subtitle,
  back,
  actions,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  back?: { to: string; label: string };
  actions?: ReactNode;
}) {
  return (
    <>
      {back ? (
        <p className="breadcrumb">
          <Link to={back.to}>{back.label}</Link>
        </p>
      ) : null}
      <div className="page-header">
        <div>
          <h1>{title}</h1>
          {subtitle ? <p className="page-subtitle">{subtitle}</p> : null}
        </div>
        {actions ? <div className="page-actions">{actions}</div> : null}
      </div>
    </>
  );
}

/** A titled section on a surface. The heading labels the section for assistive technology. */
export function Card({
  title,
  actions,
  className,
  children,
}: {
  title?: ReactNode;
  actions?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className={`card${className ? ` ${className}` : ''}`} aria-labelledby={title ? id : undefined}>
      {title ? (
        <div className="card-head">
          <h2 id={id}>{title}</h2>
          {actions}
        </div>
      ) : null}
      {children}
    </section>
  );
}

export type Tone = 'neutral' | 'success' | 'warning' | 'danger' | 'info' | 'accent';

/** A short status label. The dot pulses for `live` (and stays still with reduced motion). */
export function Badge({ tone = 'neutral', live, children }: { tone?: Tone; live?: boolean; children: ReactNode }) {
  return <span className={`badge${tone === 'neutral' ? '' : ` badge-${tone}`}${live ? ' badge-live' : ''}`}>{children}</span>;
}

/** Label and value pairs. */
export function Facts({ items }: { items: { label: ReactNode; value: ReactNode; key?: string }[] }) {
  return (
    <dl className="facts">
      {items.map((item, index) => (
        <div className="fact" key={item.key ?? index}>
          <dt>{item.label}</dt>
          <dd>{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Placeholder rows while a list loads. The label is read out, the bars are not. */
export function Skeleton({ label, rows = 4 }: { label: string; rows?: number }) {
  return (
    <div className="skeleton" role="status">
      <span className="sr-only">{label}</span>
      {Array.from({ length: rows }, (_, index) => (
        <span key={index} aria-hidden="true" />
      ))}
    </div>
  );
}

// ----- Step indicator --------------------------------------------------------------------

export type StepState = 'done' | 'current' | 'upcoming' | 'error';

/** Where a multi-step flow stands: numbered circles on a line, done steps checked. */
export function StepIndicator({ label, steps }: { label: string; steps: { label: string; state: StepState }[] }) {
  return (
    <ol className="steps" aria-label={label}>
      {steps.map((step, index) => (
        <li key={step.label} className={`step step-${step.state}`} aria-current={step.state === 'current' ? 'step' : undefined}>
          <span className="step-dot" aria-hidden="true">
            {step.state === 'done' ? <Icon name="check-circle" size="sm" /> : index + 1}
          </span>
          <span className="step-label">{step.label}</span>
        </li>
      ))}
    </ol>
  );
}

// ----- Dialog ----------------------------------------------------------------------------

/**
 * A modal dialog on the native <dialog>: the browser traps focus, closes it with Escape and
 * returns focus to whatever opened it. `onClose` runs however it closes.
 */
export function Dialog({
  open,
  title,
  onClose,
  actions,
  children,
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  actions: ReactNode;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      if (typeof dialog.showModal === 'function') dialog.showModal();
      else dialog.setAttribute('open', '');
    } else if (!open && dialog.open) {
      if (typeof dialog.close === 'function') dialog.close();
      else dialog.removeAttribute('open');
    }
  }, [open]);
  return (
    <dialog ref={ref} className="dialog" aria-labelledby={titleId} onClose={onClose}>
      <div className="dialog-body">
        <h2 id={titleId}>{title}</h2>
        {children}
      </div>
      <div className="dialog-actions">{actions}</div>
    </dialog>
  );
}

// ----- More form fields ------------------------------------------------------------------

export function SelectField({
  label,
  value,
  onChange,
  options,
  hint,
  error,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  hint?: string;
  error?: string;
}) {
  const id = useId();
  const describedBy = [hint ? `${id}-hint` : null, error ? `${id}-error` : null].filter(Boolean).join(' ') || undefined;
  return (
    <div className={`field${error ? ' has-error' : ''}`}>
      <label htmlFor={id}>{label}</label>
      {hint ? (
        <p id={`${id}-hint`} className="hint">
          {hint}
        </p>
      ) : null}
      <select
        id={id}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      {error ? (
        <p id={`${id}-error`} className="field-error">
          {error}
        </p>
      ) : null}
    </div>
  );
}

/** Radio buttons in a fieldset whose legend names the choice. */
export function ChoiceGroup({
  legend,
  value,
  onChange,
  options,
}: {
  legend: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string; hint?: string }[];
}) {
  const name = useId();
  return (
    <fieldset className="plain-fieldset">
      <legend>{legend}</legend>
      {options.map((option) => (
        <label key={option.value} className="choice">
          <input type="radio" name={name} value={option.value} checked={value === option.value} onChange={() => onChange(option.value)} />
          <span>
            {option.label}
            {option.hint ? <span className="hint"> {option.hint}</span> : null}
          </span>
        </label>
      ))}
    </fieldset>
  );
}

export function CheckboxField({
  label,
  checked,
  onChange,
}: {
  label: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="choice">
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      <span>{label}</span>
    </label>
  );
}

// ----- Meters, chips, segmented choices --------------------------------------------------

/** A labelled bar for spend, allowance or progress. `value` and `max` are plain numbers. */
export function Meter({ label, value, max, text }: { label: string; value: number; max: number; text: ReactNode }) {
  // A native <meter>: the browser draws the fill from `value`, so no inline style is needed.
  return (
    <div className="meter">
      <div className="meter-head">
        <span className="meter-label">{label}</span>
        <span className="meter-text">{text}</span>
      </div>
      <meter className="meter-bar" min={0} max={max} value={Math.min(value, max)} aria-label={label} />
    </div>
  );
}

/** A small token, such as a folder path. With `onRemove` it gets a remove button. */
export function Chip({ children, onRemove, removeLabel }: { children: ReactNode; onRemove?: () => void; removeLabel?: string }) {
  return (
    <span className="chip">
      <span className="chip-text">{children}</span>
      {onRemove ? (
        <button type="button" className="chip-remove" onClick={onRemove} aria-label={removeLabel}>
          <Icon name="error" size="sm" />
        </button>
      ) : null}
    </span>
  );
}

/** Two to four exclusive options shown as joined buttons. Radio inputs underneath. */
export function SegmentedControl({
  legend,
  value,
  onChange,
  options,
}: {
  legend: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
}) {
  const name = useId();
  return (
    <fieldset className="segmented">
      <legend>{legend}</legend>
      <div className="segmented-options">
        {options.map((option) => (
          <label key={option.value} className={value === option.value ? 'is-selected' : undefined}>
            <input type="radio" name={name} value={option.value} checked={value === option.value} onChange={() => onChange(option.value)} />
            {option.label}
          </label>
        ))}
      </div>
    </fieldset>
  );
}

/** A headline number with its label, for summary rows at the top of a page. */
export function StatCard({ label, value, detail, icon }: { label: string; value: ReactNode; detail?: ReactNode; icon?: IconName }) {
  return (
    <div className="stat">
      <p className="stat-label">
        {icon ? <Icon name={icon} size="sm" /> : null}
        {label}
      </p>
      <p className="stat-value">{value}</p>
      {detail ? <p className="stat-detail">{detail}</p> : null}
    </div>
  );
}

/** A vertical list of events, newest last, each with an icon, a summary and a time. */
export function Timeline({
  label,
  items,
}: {
  label: string;
  items: { key: string; icon: IconName; title: ReactNode; meta?: ReactNode; detail?: ReactNode; tone?: Tone }[];
}) {
  return (
    <ol className="timeline" aria-label={label}>
      {items.map((item) => (
        <li key={item.key} className={`timeline-item${item.tone ? ` timeline-${item.tone}` : ''}`}>
          <span className="timeline-icon">
            <Icon name={item.icon} size="sm" />
          </span>
          <div className="timeline-body">
            <p className="timeline-title">{item.title}</p>
            {item.meta ? <p className="timeline-meta">{item.meta}</p> : null}
            {item.detail}
          </div>
        </li>
      ))}
    </ol>
  );
}

const FLAG_COLORS = 8;

/**
 * A small flag-style tricolor picked from a name, so each project is easy to spot. Abstract on
 * purpose: never a real national flag or symbol.
 */
export function FlagAvatar({ name }: { name: string }) {
  let hash = 0;
  for (const char of name) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) >>> 0;
  const first = hash % FLAG_COLORS;
  const second = (first + 1 + ((hash >>> 3) % (FLAG_COLORS - 1))) % FLAG_COLORS;
  let third = (hash >>> 7) % FLAG_COLORS;
  if (third === second) third = first;
  return (
    <span className={`flag flag-${(hash >>> 11) % 3 === 0 ? 'v' : 'h'}`} aria-hidden="true">
      <span className={`flag-c${first}`} />
      <span className={`flag-c${second}`} />
      <span className={`flag-c${third}`} />
    </span>
  );
}

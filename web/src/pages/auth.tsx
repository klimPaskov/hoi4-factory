import { useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { Link, Navigate, useLocation, useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import {
  GITHUB_SIGN_IN_URL,
  confirmPasswordReset,
  requestPasswordReset,
  resendVerification,
  signIn,
  signUp,
  verifyEmail,
} from '../api/accounts';
import { ApiError } from '../api/client';
import { useAuth } from '../auth';
import { Alert, ErrorMessage, Field, Loading } from '../components/ui';
import { describeError, fieldForError } from '../errors';
import { useFocusOnError, usePageTitle } from '../hooks';

const MIN_PASSWORD = 10;
const MAX_PASSWORD = 256;

function AuthCard({ title, children }: { title: string; children: ReactNode }) {
  usePageTitle(title);
  return (
    <section className="card auth-card" aria-labelledby="page-title">
      <h1 id="page-title">{title}</h1>
      {children}
    </section>
  );
}

function GithubButton() {
  const { t } = useTranslation();
  return (
    <a className="button secondary block" href={GITHUB_SIGN_IN_URL}>
      {t('auth.github')}
    </a>
  );
}

function Divider() {
  const { t } = useTranslation();
  return (
    <p className="divider" aria-hidden="true">
      <span>{t('common.or')}</span>
    </p>
  );
}

function codePoints(text: string): number {
  return Array.from(text).length;
}

function from(location: { state: unknown }): string {
  const state = location.state as { from?: string } | null;
  const target = state?.from;
  // Only same-site paths: never follow a full URL out of the app.
  return typeof target === 'string' && target.startsWith('/') && !target.startsWith('//')
    ? target
    : '/';
}

/** Error code a failed GitHub sign-in can leave in the address (`?error=github_denied`). */
function errorFromQuery(params: URLSearchParams): unknown {
  const code = params.get('error');
  if (!code || !/^github_[a-z_]+$/.test(code)) return null;
  return new ApiError({ status: 400, code, message: '' });
}

// ----- Sign in ---------------------------------------------------------------------------

export function SignInPage() {
  const { t } = useTranslation();
  const { state, setAccount } = useAuth();
  const location = useLocation();
  const [params] = useSearchParams();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [errors, setErrors] = useState<{ email?: string; password?: string }>({});
  const [failure, setFailure] = useState<unknown>(() => errorFromQuery(params));
  const [busy, setBusy] = useState(false);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  if (state.status === 'authenticated') return <Navigate to={from(location)} replace />;

  async function submit(event: FormEvent) {
    event.preventDefault();
    const next: typeof errors = {};
    if (!email.trim()) next.email = t('validation.emailRequired');
    if (!password) next.password = t('validation.passwordRequired');
    setErrors(next);
    setFailure(null);
    if (next.email || next.password) {
      setVersion((v) => v + 1);
      return;
    }
    setBusy(true);
    try {
      setAccount(await signIn({ email: email.trim(), password }));
    } catch (error) {
      setFailure(error);
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  return (
    <AuthCard title={t('auth.signInTitle')}>
      <GithubButton />
      <Divider />
      <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate>
        {failure ? <ErrorMessage error={failure} focusable /> : null}
        <Field
          label={t('auth.email')}
          type="email"
          autoComplete="email"
          dir="ltr"
          value={email}
          onChange={setEmail}
          error={errors.email}
        />
        <Field
          label={t('auth.password')}
          type="password"
          autoComplete="current-password"
          dir="ltr"
          value={password}
          onChange={setPassword}
          error={errors.password}
        />
        <button type="submit" className="button primary block" disabled={busy}>
          {busy ? t('auth.signingIn') : t('common.signIn')}
        </button>
      </form>
      <p className="small">
        <Link to="/forgot-password">{t('auth.forgotLink')}</Link>
      </p>
      <p className="small">
        {t('auth.noAccount')} <Link to="/signup">{t('common.signUp')}</Link>
      </p>
    </AuthCard>
  );
}

// ----- Sign up ---------------------------------------------------------------------------

const SIGN_UP_FIELDS = {
  email: /e-?mail/i,
  password: /password/i,
  displayName: /display name|name/i,
};

export function SignUpPage() {
  const { t } = useTranslation();
  const { state, setAccount } = useAuth();
  const location = useLocation();
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [password, setPassword] = useState('');
  const [errors, setErrors] = useState<Partial<Record<keyof typeof SIGN_UP_FIELDS, string>>>({});
  const [failure, setFailure] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  if (state.status === 'authenticated') return <Navigate to={from(location)} replace />;

  async function submit(event: FormEvent) {
    event.preventDefault();
    const next: typeof errors = {};
    if (!email.trim() || !email.includes('@')) next.email = t('validation.emailInvalid');
    if (codePoints(password) < MIN_PASSWORD) {
      next.password = t('validation.passwordShort', { count: MIN_PASSWORD });
    } else if (codePoints(password) > MAX_PASSWORD) {
      next.password = t('validation.passwordLong', { count: MAX_PASSWORD });
    }
    setErrors(next);
    setFailure(null);
    if (Object.keys(next).length > 0) {
      setVersion((v) => v + 1);
      return;
    }
    setBusy(true);
    try {
      const name = displayName.trim();
      const result = await signUp({
        email: email.trim(),
        password,
        ...(name ? { display_name: name } : {}),
      });
      setAccount(result.account);
    } catch (error) {
      // Everything typed stays in the form. The server's answer decides where it is shown.
      const field = fieldForError(error, SIGN_UP_FIELDS);
      if (field) {
        setErrors({ [field]: describeError(error).message });
      } else {
        setFailure(error);
      }
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  return (
    <AuthCard title={t('auth.signUpTitle')}>
      <GithubButton />
      <Divider />
      <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate>
        {failure ? <ErrorMessage error={failure} focusable /> : null}
        <Field
          label={t('auth.email')}
          type="email"
          autoComplete="email"
          dir="ltr"
          value={email}
          onChange={setEmail}
          error={errors.email}
        />
        <Field
          label={t('auth.displayName')}
          autoComplete="name"
          optional
          maxLength={200}
          value={displayName}
          onChange={setDisplayName}
          error={errors.displayName}
        />
        <Field
          label={t('auth.password')}
          type="password"
          autoComplete="new-password"
          dir="ltr"
          hint={t('auth.passwordHint', { count: MIN_PASSWORD })}
          value={password}
          onChange={setPassword}
          error={errors.password}
        />
        <button type="submit" className="button primary block" disabled={busy}>
          {busy ? t('auth.signingUp') : t('auth.signUpButton')}
        </button>
      </form>
      <p className="small">
        {t('auth.haveAccount')} <Link to="/signin">{t('common.signIn')}</Link>
      </p>
    </AuthCard>
  );
}

// ----- Email verification ----------------------------------------------------------------

// A verification token works once. React runs effects twice in development, so the request
// for a token is shared instead of repeated.
const verifications = new Map<string, ReturnType<typeof verifyEmail>>();

export function VerifyEmailPage() {
  const { t } = useTranslation();
  const { state, setAccount } = useAuth();
  const [params] = useSearchParams();
  const token = params.get('token') ?? '';
  const [result, setResult] = useState<'working' | 'done' | { error: unknown }>(
    token ? 'working' : { error: new ApiError({ status: 400, code: 'invalid_token', message: '' }) },
  );
  const [resend, setResend] = useState<'idle' | 'sent' | { error: unknown }>('idle');

  useEffect(() => {
    if (!token) return;
    let promise = verifications.get(token);
    if (!promise) {
      promise = verifyEmail(token);
      verifications.set(token, promise);
    }
    let active = true;
    promise.then(
      (account) => {
        if (!active) return;
        setResult('done');
        // The verified account is the signed-in account only when it is the same one.
        if (state.status === 'authenticated' && state.account.id === account.id) setAccount(account);
      },
      (error: unknown) => active && setResult({ error }),
    );
    return () => {
      active = false;
    };
    // The token decides the request. Auth state changing must not repeat it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);

  async function sendNew() {
    try {
      await resendVerification();
      setResend('sent');
    } catch (error) {
      setResend({ error });
    }
  }

  return (
    <AuthCard title={t('verify.title')}>
      {result === 'working' ? <Loading label={t('verify.working')} /> : null}
      {result === 'done' ? (
        <>
          <Alert kind="success">{t('verify.done')}</Alert>
          <p>
            <Link className="button primary" to="/">
              {t('verify.toProjects')}
            </Link>
          </p>
        </>
      ) : null}
      {typeof result === 'object' ? (
        <>
          <ErrorMessage error={result.error} />
          {state.status === 'authenticated' ? (
            <>
              <p>
                <button type="button" className="button secondary" onClick={() => void sendNew()}>
                  {t('verify.resend')}
                </button>
              </p>
              {resend === 'sent' ? <Alert kind="success">{t('verify.sent')}</Alert> : null}
              {typeof resend === 'object' ? <ErrorMessage error={resend.error} /> : null}
            </>
          ) : (
            <p>
              <Link to="/signin" state={{ from: '/' }}>
                {t('verify.signInForNew')}
              </Link>
            </p>
          )}
        </>
      ) : null}
    </AuthCard>
  );
}

// ----- Password reset --------------------------------------------------------------------

export function ForgotPasswordPage() {
  const { t } = useTranslation();
  const [email, setEmail] = useState('');
  const [error, setError] = useState<string | undefined>();
  const [failure, setFailure] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [sent, setSent] = useState(false);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setFailure(null);
    if (!email.trim() || !email.includes('@')) {
      setError(t('validation.emailInvalid'));
      setVersion((v) => v + 1);
      return;
    }
    setError(undefined);
    setBusy(true);
    try {
      await requestPasswordReset(email.trim());
      setSent(true);
    } catch (e) {
      setFailure(e);
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  return (
    <AuthCard title={t('reset.requestTitle')}>
      {sent ? (
        <>
          {/* The server answers the same way whether or not the address has an account. */}
          <Alert kind="success">{t('reset.requestSent')}</Alert>
          <p>
            <Link to="/signin">{t('reset.backToSignIn')}</Link>
          </p>
        </>
      ) : (
        <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate>
          <p>{t('reset.requestIntro')}</p>
          {failure ? <ErrorMessage error={failure} focusable /> : null}
          <Field
            label={t('auth.email')}
            type="email"
            autoComplete="email"
            dir="ltr"
            value={email}
            onChange={setEmail}
            error={error}
          />
          <button type="submit" className="button primary block" disabled={busy}>
            {busy ? t('reset.sending') : t('reset.sendLink')}
          </button>
          <p className="small">
            <Link to="/signin">{t('reset.backToSignIn')}</Link>
          </p>
        </form>
      )}
    </AuthCard>
  );
}

export function ResetPasswordPage() {
  const { t } = useTranslation();
  const { refresh } = useAuth();
  const [params] = useSearchParams();
  const token = params.get('token') ?? '';
  const [password, setPassword] = useState('');
  const [again, setAgain] = useState('');
  const [errors, setErrors] = useState<{ password?: string; again?: string }>({});
  const [failure, setFailure] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const next: typeof errors = {};
    if (codePoints(password) < MIN_PASSWORD) {
      next.password = t('validation.passwordShort', { count: MIN_PASSWORD });
    } else if (codePoints(password) > MAX_PASSWORD) {
      next.password = t('validation.passwordLong', { count: MAX_PASSWORD });
    }
    if (!next.password && again !== password) next.again = t('validation.passwordMismatch');
    setErrors(next);
    setFailure(null);
    if (next.password || next.again) {
      setVersion((v) => v + 1);
      return;
    }
    setBusy(true);
    try {
      await confirmPasswordReset(token, password);
      setDone(true);
      // Every session ended on the server. Learn that here too.
      await refresh();
    } catch (error) {
      const field = fieldForError(error, { password: /password/i });
      if (field) setErrors({ password: describeError(error).message });
      else setFailure(error);
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  if (!token) {
    return (
      <AuthCard title={t('reset.confirmTitle')}>
        <Alert kind="error">{t('errors.code.invalid_token')}</Alert>
        <p>
          <Link to="/forgot-password">{t('reset.askAgain')}</Link>
        </p>
      </AuthCard>
    );
  }

  return (
    <AuthCard title={t('reset.confirmTitle')}>
      {done ? (
        <>
          <Alert kind="success">{t('reset.done')}</Alert>
          <p>
            <Link className="button primary" to="/signin">
              {t('common.signIn')}
            </Link>
          </p>
        </>
      ) : (
        <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate>
          {failure ? (
            <>
              <ErrorMessage error={failure} focusable />
              {failure instanceof ApiError && failure.code === 'invalid_token' ? (
                <p>
                  <Link to="/forgot-password">{t('reset.askAgain')}</Link>
                </p>
              ) : null}
            </>
          ) : null}
          <Field
            label={t('reset.newPassword')}
            type="password"
            autoComplete="new-password"
            dir="ltr"
            hint={t('auth.passwordHint', { count: MIN_PASSWORD })}
            value={password}
            onChange={setPassword}
            error={errors.password}
          />
          <Field
            label={t('reset.repeatPassword')}
            type="password"
            autoComplete="new-password"
            dir="ltr"
            value={again}
            onChange={setAgain}
            error={errors.again}
          />
          <button type="submit" className="button primary block" disabled={busy}>
            {busy ? t('reset.saving') : t('reset.save')}
          </button>
        </form>
      )}
    </AuthCard>
  );
}

import { Link, Navigate, Outlet, Route, Routes, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useAuth } from './auth';
import { Layout } from './components/Layout';
import { ErrorMessage, Loading } from './components/ui';
import { usePageTitle } from './hooks';
import { AccountPage } from './pages/Account';
import {
  ForgotPasswordPage,
  ResetPasswordPage,
  SignInPage,
  SignUpPage,
  VerifyEmailPage,
} from './pages/auth';
import { ProjectPage } from './pages/Project';
import { ProjectsPage } from './pages/Projects';

/** Waits for the "who am I" answer before any page decides what to show. */
function AuthBoundary() {
  const { t } = useTranslation();
  const { state, refresh } = useAuth();
  if (state.status === 'loading') return <Loading label={t('common.loadingApp')} />;
  if (state.status === 'error') {
    return (
      <>
        <h1>{t('common.connectionTitle')}</h1>
        <ErrorMessage error={state.error} onRetry={() => void refresh()} />
      </>
    );
  }
  return <Outlet />;
}

/** Pages that need an account send guests to sign in and bring them back afterwards. */
function RequireAuth() {
  const { state } = useAuth();
  const location = useLocation();
  if (state.status === 'anonymous') {
    return <Navigate to="/signin" replace state={{ from: location.pathname + location.search }} />;
  }
  return <Outlet />;
}

function NotFoundPage() {
  const { t } = useTranslation();
  usePageTitle(t('notFound.title'));
  return (
    <>
      <h1>{t('notFound.title')}</h1>
      <p>{t('notFound.body')}</p>
      <p>
        <Link to="/">{t('notFound.home')}</Link>
      </p>
    </>
  );
}

export function AppRoutes() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route element={<AuthBoundary />}>
          <Route path="signin" element={<SignInPage />} />
          <Route path="signup" element={<SignUpPage />} />
          <Route path="forgot-password" element={<ForgotPasswordPage />} />
          <Route path="reset-password" element={<ResetPasswordPage />} />
          <Route path="verify-email" element={<VerifyEmailPage />} />
          <Route element={<RequireAuth />}>
            <Route index element={<ProjectsPage />} />
            <Route path="projects/:id" element={<ProjectPage />} />
            <Route path="account" element={<AccountPage />} />
          </Route>
          <Route path="*" element={<NotFoundPage />} />
        </Route>
      </Route>
    </Routes>
  );
}

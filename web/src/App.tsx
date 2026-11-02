import { lazy, Suspense } from 'react';
import { Link, Navigate, Outlet, Route, Routes, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useAuth } from './auth';
import { AuthLayout } from './components/AuthLayout';
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
import { TaskPage } from './pages/Task';
import { TasksPage } from './pages/Tasks';
import { ProjectsPage } from './pages/Projects';

// The design lab (ADR-025) exists only in development builds. In a production build the
// condition is the constant `false`, so the lab and its chunk are left out entirely.
const DesignLab = import.meta.env.DEV ? lazy(() => import('./design-lab/DesignLab')) : null;

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
      {DesignLab ? (
        <Route
          path="design-lab/*"
          element={
            <Suspense fallback={<Loading />}>
              <DesignLab />
            </Suspense>
          }
        />
      ) : null}
      <Route element={<AuthLayout />}>
        <Route element={<AuthBoundary />}>
          <Route path="signin" element={<SignInPage />} />
          <Route path="signup" element={<SignUpPage />} />
          <Route path="forgot-password" element={<ForgotPasswordPage />} />
          <Route path="reset-password" element={<ResetPasswordPage />} />
          <Route path="verify-email" element={<VerifyEmailPage />} />
        </Route>
      </Route>
      <Route element={<Layout />}>
        <Route element={<AuthBoundary />}>
          <Route element={<RequireAuth />}>
            <Route index element={<ProjectsPage />} />
            <Route path="projects/:id" element={<ProjectPage />} />
            <Route path="projects/:id/tasks" element={<TasksPage />} />
            <Route path="projects/:id/tasks/:taskId" element={<TaskPage />} />
            <Route path="account" element={<AccountPage />} />
          </Route>
          <Route path="*" element={<NotFoundPage />} />
        </Route>
      </Route>
    </Routes>
  );
}

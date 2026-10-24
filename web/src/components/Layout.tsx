import { useEffect, useRef, useState } from 'react';
import { Link, NavLink, Outlet, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useAuth } from '../auth';
import { applyTheme, readTheme, type ThemeChoice } from '../hooks';
import { GameReferenceIndicator } from './GameReference';
import { ErrorMessage } from './ui';
import { VerifyBanner } from './VerifyBanner';

function ThemeSelect() {
  const { t } = useTranslation();
  const [theme, setTheme] = useState<ThemeChoice>(readTheme);
  useEffect(() => applyTheme(theme), [theme]);
  return (
    <div className="theme-select">
      <label htmlFor="theme-select" className="small">
        {t('layout.theme')}
      </label>
      <select
        id="theme-select"
        value={theme}
        onChange={(event) => setTheme(event.target.value as ThemeChoice)}
      >
        <option value="system">{t('layout.themeSystem')}</option>
        <option value="light">{t('layout.themeLight')}</option>
        <option value="dark">{t('layout.themeDark')}</option>
      </select>
    </div>
  );
}

export function Layout() {
  const { t } = useTranslation();
  const { state, signOut } = useAuth();
  const location = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const firstRender = useRef(true);

  // A route change is a new page: move focus to its content so keyboard and screen reader
  // users start at the top of it, the way a full page load would.
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    mainRef.current?.focus({ preventScroll: false });
  }, [location.pathname]);

  const [signOutError, setSignOutError] = useState<unknown>(null);
  const signedIn = state.status === 'authenticated';

  return (
    <>
      <a className="skip-link" href="#main">
        {t('layout.skip')}
      </a>
      <header className="site-header">
        <div className="container header-row">
          <Link to="/" className="brand">
            {t('app.name')}
          </Link>
          <nav aria-label={t('layout.primaryNav')} className="nav">
            {signedIn ? (
              <>
                <NavLink to="/" end>
                  {t('nav.projects')}
                </NavLink>
                <NavLink to="/account">{t('nav.account')}</NavLink>
              </>
            ) : state.status === 'loading' ? null : (
              <>
                <NavLink to="/signin">{t('common.signIn')}</NavLink>
                <NavLink to="/signup">{t('common.signUp')}</NavLink>
              </>
            )}
          </nav>
          <div className="header-tools">
            {signedIn ? <GameReferenceIndicator /> : null}
            <ThemeSelect />
            {signedIn ? (
              <button type="button" className="button secondary small-button" onClick={() => void signOut().catch(setSignOutError)}>
                {t('common.signOut')}
              </button>
            ) : null}
          </div>
        </div>
        <VerifyBanner />
        {signOutError ? (
          <div className="container">
            <ErrorMessage error={signOutError} />
          </div>
        ) : null}
      </header>
      <main id="main" ref={mainRef} tabIndex={-1} className="container main">
        <Outlet />
      </main>
      <footer className="container footer small muted">{t('layout.footer')}</footer>
    </>
  );
}

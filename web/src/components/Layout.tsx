import { useEffect, useRef, useState } from 'react';
import { Link, NavLink, Outlet, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useAuth } from '../auth';
import { applyTheme, readTheme, type ThemeChoice } from '../hooks';
import { GameReferenceIndicator } from './GameReference';
import { BrandMark, ErrorMessage, Icon } from './ui';
import { VerifyBanner } from './VerifyBanner';

function ThemeSelect() {
  const { t } = useTranslation();
  const [theme, setTheme] = useState<ThemeChoice>(readTheme);
  useEffect(() => applyTheme(theme), [theme]);
  return (
    <div className="theme-select">
      <label htmlFor="theme-select">{t('layout.theme')}</label>
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

/**
 * The app shell: a sidebar with the navigation and account tools on wide screens, and a top bar
 * whose Menu button opens the same panel on phones.
 */
export function Layout() {
  const { t } = useTranslation();
  const { state, signOut } = useAuth();
  const location = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const firstRender = useRef(true);
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuFor, setMenuFor] = useState(location.pathname);

  // A route change closes the phone menu.
  if (menuFor !== location.pathname) {
    setMenuFor(location.pathname);
    setMenuOpen(false);
  }

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
      <div className="app-shell">
        <header className="app-side">
          <div className="app-side-top">
            <Link to="/" className="brand">
              <BrandMark />
              <span>{t('app.name')}</span>
            </Link>
            <button
              type="button"
              className="button ghost app-menu-button"
              aria-expanded={menuOpen}
              aria-controls="app-side-panel"
              onClick={() => setMenuOpen((open) => !open)}
            >
              {t('layout.menu')}
            </button>
          </div>
          <div id="app-side-panel" className={`app-side-panel${menuOpen ? ' is-open' : ''}`}>
            <nav aria-label={t('layout.primaryNav')}>
              <ul className="app-nav">
                {signedIn ? (
                  <>
                    <li>
                      <NavLink to="/" end>
                        <Icon name="folder" />
                        {t('nav.projects')}
                      </NavLink>
                    </li>
                    <li>
                      <NavLink to="/account">
                        <Icon name="account" />
                        {t('nav.account')}
                      </NavLink>
                    </li>
                  </>
                ) : state.status === 'loading' ? null : (
                  <>
                    <li>
                      <NavLink to="/signin">
                        <Icon name="sign-out" />
                        {t('common.signIn')}
                      </NavLink>
                    </li>
                    <li>
                      <NavLink to="/signup">
                        <Icon name="plus" />
                        {t('common.signUp')}
                      </NavLink>
                    </li>
                  </>
                )}
              </ul>
            </nav>
            <div className="app-side-bottom">
              {signedIn ? <GameReferenceIndicator /> : null}
              <ThemeSelect />
              {signedIn ? (
                <div className="app-user">
                  <p>
                    {t('layout.signedInAs')}{' '}
                    <bdi dir="auto" className="strong">
                      {state.account.display_name}
                    </bdi>
                  </p>
                  <button
                    type="button"
                    className="button secondary small-button"
                    onClick={() => void signOut().catch(setSignOutError)}
                  >
                    <Icon name="sign-out" size="sm" />
                    {t('common.signOut')}
                  </button>
                </div>
              ) : null}
            </div>
          </div>
        </header>
        <div className="app-body">
          <VerifyBanner />
          {signOutError ? (
            <div className="app-main">
              <ErrorMessage error={signOutError} />
            </div>
          ) : null}
          <main id="main" ref={mainRef} tabIndex={-1} className="app-main">
            <Outlet />
          </main>
          <footer className="app-footer">{t('layout.footer')}</footer>
        </div>
      </div>
    </>
  );
}

import { useEffect, useRef } from 'react';
import { Link, Outlet, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import { ThemeSelect } from './Layout';
import { Mascot } from './Mascot';
import { BrandMark } from './ui';

/**
 * The shell of the sign-in, sign-up, verify and reset pages (sign-in round, variant C): a calm
 * panel with the mark, one line about the app and the mascot peeking in, and the form beside it.
 * On phones the panel shrinks to the mark.
 */
export function AuthLayout() {
  const { t } = useTranslation();
  const location = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const firstRender = useRef(true);

  // A route change is a new page: move focus to its content, as Layout does.
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    mainRef.current?.focus({ preventScroll: false });
  }, [location.pathname]);

  return (
    <>
      <a className="skip-link" href="#main">
        {t('layout.skip')}
      </a>
      <div className="auth-shell">
        <header className="auth-panel">
          <Link to="/" className="brand auth-brand">
            <BrandMark />
            <span>{t('app.name')}</span>
          </Link>
          <p className="auth-pitch">{t('auth.pitch')}</p>
          <div className="auth-mascot">
            <Mascot pose="peek" size="lg" motion="none" />
          </div>
        </header>
        <main id="main" ref={mainRef} tabIndex={-1} className="auth-main">
          <div className="auth-tools">
            <ThemeSelect />
          </div>
          <div className="auth-form">
            <Outlet />
          </div>
        </main>
      </div>
    </>
  );
}

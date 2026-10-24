import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { GITHUB_LINK_URL } from '../api/accounts';
import { useAuth } from '../auth';
import { Alert, ErrorMessage } from '../components/ui';
import { formatDate } from '../format';
import { usePageTitle } from '../hooks';

export function AccountPage() {
  const { t } = useTranslation();
  usePageTitle(t('account.title'));
  const { state, signOut, signOutEverywhere } = useAuth();
  const [confirming, setConfirming] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);

  if (state.status !== 'authenticated') return null;
  const { account } = state;

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setFailure(null);
    try {
      await action();
    } catch (error) {
      setFailure(error);
      setBusy(false);
    }
  }

  return (
    <>
      <h1>{t('account.title')}</h1>
      <section className="card" aria-labelledby="account-details">
        <h2 id="account-details">{t('account.details')}</h2>
        <dl className="facts">
          <dt>{t('auth.displayName')}</dt>
          <dd dir="auto">{account.display_name}</dd>
          <dt>{t('auth.email')}</dt>
          <dd dir="ltr">{account.email}</dd>
          <dt>{t('account.emailStatus')}</dt>
          <dd>{account.email_verified ? t('account.verified') : t('account.notVerified')}</dd>
          <dt>{t('account.github')}</dt>
          <dd>
            {account.github_linked ? (
              t('account.githubLinked')
            ) : (
              <>
                {t('account.githubNotLinked')}{' '}
                <a href={GITHUB_LINK_URL}>{t('account.githubConnect')}</a>
              </>
            )}
          </dd>
          <dt>{t('account.created')}</dt>
          <dd>{formatDate(account.created_at)}</dd>
        </dl>
      </section>

      <section className="card" aria-labelledby="account-sessions">
        <h2 id="account-sessions">{t('account.sessions')}</h2>
        {failure ? <ErrorMessage error={failure} /> : null}
        {confirming ? (
          <div className="confirm" role="group" aria-label={t('account.signOutAllLabel')}>
            <Alert kind="warning">{t('account.signOutAllConfirm')}</Alert>
            <div className="button-row">
              <button type="button" className="button danger" disabled={busy} onClick={() => void run(signOutEverywhere)}>
                {busy ? t('account.signingOut') : t('account.signOutAllButton')}
              </button>
              <button type="button" className="button secondary" onClick={() => setConfirming(false)}>
                {t('common.cancel')}
              </button>
            </div>
          </div>
        ) : (
          <div className="button-row">
            <button type="button" className="button secondary" disabled={busy} onClick={() => void run(signOut)}>
              {t('common.signOut')}
            </button>
            <button type="button" className="button danger-outline" onClick={() => setConfirming(true)}>
              {t('account.signOutAll')}
            </button>
          </div>
        )}
      </section>
    </>
  );
}

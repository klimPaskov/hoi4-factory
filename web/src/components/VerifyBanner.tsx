import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { resendVerification } from '../api/accounts';
import { useAuth } from '../auth';
import { describeError } from '../errors';

type Status = 'idle' | 'sending' | 'sent' | 'already' | 'failed';

/** Non-blocking notice for an account whose email is not verified yet. */
export function VerifyBanner() {
  const { t } = useTranslation();
  const { state, refresh } = useAuth();
  const [status, setStatus] = useState<Status>('idle');
  const [failure, setFailure] = useState('');

  if (state.status !== 'authenticated' || state.account.email_verified) return null;

  async function resend() {
    setStatus('sending');
    try {
      const result = await resendVerification();
      if (result.already_verified) {
        setStatus('already');
        await refresh();
      } else {
        setStatus('sent');
      }
    } catch (error) {
      setFailure(describeError(error).message);
      setStatus('failed');
    }
  }

  return (
    <div className="banner" role="region" aria-label={t('verify.bannerLabel')}>
      <p>
        {t('verify.bannerText')}{' '}
        <bdi dir="auto" className="strong">
          {state.account.email}
        </bdi>
      </p>
      <p className="banner-actions">
        <button
          type="button"
          className="button secondary small-button"
          onClick={() => void resend()}
          disabled={status === 'sending'}
        >
          {status === 'sending' ? t('verify.sending') : t('verify.resend')}
        </button>
        <span role="status" className="small">
          {status === 'sent' ? t('verify.sent') : null}
          {status === 'already' ? t('verify.already') : null}
          {status === 'failed' ? failure : null}
        </span>
      </p>
    </div>
  );
}

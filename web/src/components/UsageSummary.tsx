import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import { getUsage } from '../api/tasks';
import { formatMicros } from '../format';
import { useAsync } from '../hooks';
import { Alert, ErrorMessage, Loading, NotAvailableYet } from './ui';

/** The free monthly allowance and what has been spent this month. */
export function UsageSummary() {
  const { t } = useTranslation();
  const state = useAsync((signal) => getUsage(signal));

  return (
    <section className="card" aria-labelledby="usage-title">
      <h2 id="usage-title">{t('usage.title')}</h2>
      {state.status === 'loading' ? <Loading label={t('usage.loading')} /> : null}
      {state.status === 'error' ? (
        state.error instanceof ApiError && state.error.isUnavailable ? (
          <NotAvailableYet what={t('usage.what')} />
        ) : (
          <ErrorMessage error={state.error} onRetry={state.reload} />
        )
      ) : null}
      {state.status === 'ready' ? (
        <>
          {state.data.verified ? (
            <p>
              {t('usage.allowance', {
                left: formatMicros(state.data.remaining_allowance_micros),
                total: formatMicros(state.data.allowance_micros),
              })}
            </p>
          ) : (
            <Alert kind="info">{t('usage.notVerified')}</Alert>
          )}
          {state.data.unknown_count > 0 ? (
            <Alert kind="warning">{t('usage.unknown', { count: state.data.unknown_count })}</Alert>
          ) : null}
          <p className="muted small">
            {t('usage.userKey', { amount: formatMicros(state.data.user_key_spent_micros) })}
          </p>
        </>
      ) : null}
    </section>
  );
}

import { useTranslation } from 'react-i18next';
import { ApiError } from '../../api/client';
import { ErrorMessage, NotAvailableYet } from '../ui';

/** Failures after which only a fresh scan helps. */
const RESTART_CODES = new Set(['evidence_not_approved', 'analysis_not_confirmed', 'root_changed']);

/**
 * A failed Setup call. A missing route is the calm "not available yet" state; a failure that
 * only a fresh scan can fix offers the scan button next to the explanation.
 */
export function StepError({
  error,
  what,
  onRetry,
  onRestart,
}: {
  error: unknown;
  what: string;
  onRetry?: () => void;
  onRestart?: () => void;
}) {
  const { t } = useTranslation();
  if (error instanceof ApiError && error.isUnavailable && error.code !== 'capability_unavailable') {
    return <NotAvailableYet what={what} />;
  }
  return (
    <>
      <ErrorMessage error={error} onRetry={onRetry} />
      {onRestart && error instanceof ApiError && RESTART_CODES.has(error.code) ? (
        <p>
          <button type="button" className="button secondary" onClick={onRestart}>
            {t('setup.sessionEnded.button')}
          </button>
        </p>
      ) : null}
    </>
  );
}

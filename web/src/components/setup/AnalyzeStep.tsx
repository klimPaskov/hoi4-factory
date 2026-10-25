import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../../api/client';
import { analyzeSetup, type SetupAnalysis } from '../../api/intake';
import { formatNumber } from '../../format';
import { Alert } from '../ui';
import { StepError } from './StepError';

/** Seconds since this component appeared. Mounted only while the long call runs. */
function Elapsed() {
  const { t } = useTranslation();
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    const started = Date.now();
    const timer = window.setInterval(() => {
      setSeconds(Math.floor((Date.now() - started) / 1000));
    }, 1000);
    return () => window.clearInterval(timer);
  }, []);
  // Not a live region: a counter that changes every second would be read out every second.
  return (
    <p className="muted small">
      {t('setup.analyze.elapsed', { count: seconds, n: formatNumber(seconds) })}
    </p>
  );
}

/**
 * Step 3: ask the model for the values. The call can take up to about three minutes, so the
 * wait is explained and the failure states are told apart.
 */
export function AnalyzeStep({
  projectId,
  analysis,
  state,
  onAnalyzed,
  onApiError,
  onRestart,
}: {
  projectId: string;
  analysis: SetupAnalysis | null;
  state: 'current' | 'done';
  onAnalyzed: (analysis: SetupAnalysis) => void;
  onApiError: (error: unknown) => boolean;
  onRestart: () => void;
}) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);

  async function run() {
    setBusy(true);
    setFailure(null);
    try {
      onAnalyzed(await analyzeSetup(projectId));
    } catch (error) {
      if (!onApiError(error)) setFailure(error);
    } finally {
      setBusy(false);
    }
  }

  if (state === 'done' && analysis) {
    return (
      <p>
        {t('setup.analyze.done', {
          model: analysis.model,
          count: analysis.attempts,
          n: formatNumber(analysis.attempts),
        })}
      </p>
    );
  }

  const notConfigured =
    failure instanceof ApiError && failure.code === 'capability_unavailable';

  return (
    <div>
      <p>{t('setup.analyze.intro')}</p>
      {busy ? (
        <>
          <p className="loading" role="status">
            <span className="spinner" aria-hidden="true" />
            <span>
              {t('setup.analyze.working')} {t('setup.analyze.patience')}
            </span>
          </p>
          <Elapsed />
          <p className="muted small">{t('setup.analyze.leaveNote')}</p>
        </>
      ) : null}
      {!busy && notConfigured ? <Alert kind="error">{t('setup.analyze.notConfigured')}</Alert> : null}
      {!busy && failure && !notConfigured ? (
        <StepError
          error={failure}
          what={t('setup.analyze.what')}
          onRestart={onRestart}
        />
      ) : null}
      <p className="button-row">
        <button type="button" className="button primary" onClick={() => void run()} disabled={busy}>
          {busy
            ? t('setup.analyze.buttonWorking')
            : failure
              ? t('setup.analyze.retry')
              : t('setup.analyze.button')}
        </button>
      </p>
    </div>
  );
}

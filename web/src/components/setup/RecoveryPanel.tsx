import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { recoverSetup, type SetupChanges, type SetupRecovery, type SetupRecoveryOutcome } from '../../api/intake';
import { formatNumber } from '../../format';
import { Alert } from '../ui';
import { StepError } from './StepError';

const ACTION_KEYS = ['rollback', 'resume', 'discard_staging'] as const;
type ActionKey = (typeof ACTION_KEYS)[number];

function isAction(value: string | null): value is ActionKey {
  return value !== null && (ACTION_KEYS as readonly string[]).includes(value);
}

function PathList({ label, paths }: { label: string; paths: string[] }) {
  if (paths.length === 0) return null;
  return (
    <>
      <h4>
        {label} ({formatNumber(paths.length)})
      </h4>
      <ul>
        {paths.slice(0, 50).map((path) => (
          <li key={path} dir="ltr" className="path">
            {path}
          </li>
        ))}
      </ul>
    </>
  );
}

function Differences({ differences }: { differences: SetupChanges }) {
  const { t } = useTranslation();
  return (
    <>
      <PathList label={t('setup.recovery.differences.added')} paths={differences.added} />
      <PathList label={t('setup.recovery.differences.removed')} paths={differences.removed} />
      <PathList label={t('setup.recovery.differences.changed')} paths={differences.changed} />
      <PathList label={t('setup.recovery.differences.modeChanged')} paths={differences.mode_changed} />
      {differences.git_index_changed ? <p>{t('setup.recovery.differences.gitIndex')}</p> : null}
    </>
  );
}

/** The result of a recovery: exact restoration, or exactly what still differs. */
function Outcome({
  outcome,
  onRescan,
  onDismiss,
}: {
  outcome: SetupRecoveryOutcome;
  onRescan: () => void;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  const restoration = outcome.restoration;
  return (
    <section className="card recovery" aria-labelledby="recovery-outcome-title">
      <h2 id="recovery-outcome-title">{t('setup.recovery.outcomeTitle')}</h2>
      <p>
        {isAction(outcome.action)
          ? t(`setup.recovery.outcome.${outcome.action}`)
          : t('setup.recovery.outcomeOther', { action: outcome.action })}{' '}
        {t('setup.recovery.state', { state: outcome.journal_state })}
      </p>
      {restoration ? (
        restoration.exact ? (
          <Alert kind="success">{t('setup.recovery.exact')}</Alert>
        ) : (
          <Alert kind="warning">
            <p>{t('setup.recovery.notExact')}</p>
            <Differences differences={restoration.differences} />
          </Alert>
        )
      ) : null}
      <p className="button-row">
        <button type="button" className="button secondary" onClick={onRescan}>
          {t('setup.recovery.rescan')}
        </button>
        <button type="button" className="button secondary" onClick={onDismiss}>
          {t('setup.recovery.dismiss')}
        </button>
      </p>
    </section>
  );
}

/**
 * An unfinished apply, shown above the wizard. Only the one action the journal allows is
 * offered, and only after a confirm step. When no action is allowed, a person has to review it.
 */
export function RecoveryPanel({
  projectId,
  recovery,
  outcome,
  onRecovered,
  onDismissOutcome,
  onRescan,
  onApiError,
}: {
  projectId: string;
  recovery: SetupRecovery | null;
  outcome: SetupRecoveryOutcome | null;
  onRecovered: (outcome: SetupRecoveryOutcome) => void;
  onDismissOutcome: () => void;
  onRescan: () => void;
  onApiError: (error: unknown) => boolean;
}) {
  const { t } = useTranslation();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);

  if (!recovery) {
    return outcome ? (
      <Outcome outcome={outcome} onRescan={onRescan} onDismiss={onDismissOutcome} />
    ) : null;
  }

  const action = isAction(recovery.allowed_action) ? recovery.allowed_action : null;

  async function recover(current: SetupRecovery) {
    setBusy(true);
    setFailure(null);
    try {
      onRecovered(await recoverSetup(projectId, { journal_id: current.journal_id, variant: current.variant }));
      setConfirming(false);
    } catch (error) {
      if (!onApiError(error)) setFailure(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="card recovery" aria-labelledby="recovery-title">
      <h2 id="recovery-title">{t('setup.recovery.title')}</h2>
      <Alert kind="warning">{t('setup.recovery.intro')}</Alert>
      <h3>{t('setup.recovery.explanationLabel')}</h3>
      <p dir="auto">{recovery.explanation}</p>
      <dl className="facts">
        <div className="fact">
          <dt>{t('setup.recovery.journalState')}</dt>
          <dd dir="ltr">{recovery.journal_state}</dd>
        </div>
        <div className="fact">
          <dt>{t('setup.recovery.variant')}</dt>
          <dd dir="ltr">{recovery.variant}</dd>
        </div>
      </dl>
      {recovery.owner_active ? <Alert kind="info">{t('setup.recovery.ownerActive')}</Alert> : null}
      {failure ? <StepError error={failure} what={t('setup.recovery.what')} /> : null}
      {action === null ? (
        <Alert kind="error">{t('setup.recovery.manual')}</Alert>
      ) : (
        <>
          <p>
            {t('setup.recovery.actionIntro')} {t(`setup.recovery.action.${action}`)}
          </p>
          {confirming ? (
            <div className="confirm" role="group" aria-label={t('setup.recovery.confirmLabel')}>
              <p>{t(`setup.recovery.confirm.${action}`)}</p>
              <div className="button-row">
                <button
                  type="button"
                  className="button primary"
                  onClick={() => void recover(recovery)}
                  disabled={busy}
                >
                  {busy ? t('setup.recovery.working') : t(`setup.recovery.confirmButton.${action}`)}
                </button>
                <button
                  type="button"
                  className="button secondary"
                  onClick={() => setConfirming(false)}
                  disabled={busy}
                >
                  {t('common.cancel')}
                </button>
              </div>
            </div>
          ) : (
            <p className="button-row">
              <button
                type="button"
                className="button primary"
                onClick={() => setConfirming(true)}
                disabled={recovery.owner_active}
              >
                {t(`setup.recovery.button.${action}`)}
              </button>
            </p>
          )}
        </>
      )}
    </section>
  );
}

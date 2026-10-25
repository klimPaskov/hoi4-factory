import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import {
  getSetupRecovery,
  getSetupStatus,
  scanSetup,
  type SetupRecovery,
  type SetupRecoveryOutcome,
  type SetupScan,
  type SetupStatus,
} from '../api/intake';
import { useAsync } from '../hooks';
import { Alert, ErrorMessage, Loading } from './ui';
import { AnalyzeStep } from './setup/AnalyzeStep';
import { ApplyStep } from './setup/ApplyStep';
import { ConfirmStep } from './setup/ConfirmStep';
import { EvidenceStep } from './setup/EvidenceStep';
import { PlanStep } from './setup/PlanStep';
import { RecoveryPanel } from './setup/RecoveryPanel';
import { ScanStep } from './setup/ScanStep';
import { StepError } from './setup/StepError';
import {
  currentStep,
  dataFromStatus,
  EMPTY_DATA,
  stepsFor,
  type StepId,
  type WizardData,
} from './setup/wizard';

interface Loaded {
  status: SetupStatus | null;
  recovery: SetupRecovery | null;
}

/** A route this server does not have is "nothing to show", not a failure of the page. */
async function orNull<T>(call: Promise<T>): Promise<T | null> {
  try {
    return await call;
  } catch (error) {
    if (error instanceof ApiError && error.isUnavailable) return null;
    throw error;
  }
}

async function loadInitial(projectId: string, skipStatus: boolean, signal: AbortSignal): Promise<Loaded> {
  const [status, recovery] = await Promise.all([
    skipStatus ? Promise.resolve(null) : orNull(getSetupStatus(projectId, signal)),
    orNull(getSetupRecovery(projectId, signal)),
  ]);
  return { status, recovery };
}

/**
 * The Setup wizard: scan, approve excerpts, analyze, confirm values, plan, approve and apply,
 * with recovery for an interrupted apply. The server holds the session; this page restores the
 * right step from GET status after a reload. Nothing is written to the mod before the person
 * ticks "I reviewed this plan" and approves the exact plan hash shown.
 */
export function SetupReview({
  projectId,
  initialScan,
  onChanged,
}: {
  projectId: string;
  /** A scan that was just made elsewhere on the page (creating a mod). Skips the status lookup. */
  initialScan: SetupScan | null;
  /** The workspace changed (a plan was applied or an interrupted apply was recovered). */
  onChanged: () => void;
}) {
  const { t } = useTranslation();
  const loaded = useAsync((signal) => loadInitial(projectId, initialScan !== null, signal));

  if (loaded.status === 'ready') {
    return (
      <Wizard projectId={projectId} initialScan={initialScan} loaded={loaded.data} onChanged={onChanged} />
    );
  }
  return (
    <section className="card" aria-labelledby="setup-title">
      <h2 id="setup-title">{t('setup.title')}</h2>
      {loaded.status === 'loading' ? <Loading label={t('setup.loading')} /> : null}
      {loaded.status === 'error' ? <StepError error={loaded.error} what={t('setup.loadWhat')} onRetry={loaded.reload} /> : null}
    </section>
  );
}

function Wizard({
  projectId,
  initialScan,
  loaded,
  onChanged,
}: {
  projectId: string;
  initialScan: SetupScan | null;
  loaded: Loaded;
  onChanged: () => void;
}) {
  const { t } = useTranslation();
  const [restored] = useState(() => (loaded.status ? dataFromStatus(loaded.status) : null));
  const [data, setData] = useState<WizardData>(() =>
    initialScan ? { ...EMPTY_DATA, scan: initialScan } : (restored?.data ?? EMPTY_DATA),
  );
  const [ended, setEnded] = useState(restored?.ended ?? false);
  const [mode, setMode] = useState<'create' | 'existing'>(
    () => (initialScan ?? loaded.status?.scan)?.mode ?? loaded.status?.session?.mode ?? 'existing',
  );
  const [editingValues, setEditingValues] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanFailure, setScanFailure] = useState<unknown>(null);
  const [recovery, setRecovery] = useState<SetupRecovery | null>(loaded.recovery);
  const [recoveryCheckFailure, setRecoveryCheckFailure] = useState<unknown>(null);
  const [outcome, setOutcome] = useState<SetupRecoveryOutcome | null>(null);

  function patch(change: Partial<WizardData>) {
    setData((previous) => ({ ...previous, ...change }));
  }

  /** Asks the server whether an unfinished apply needs recovery. The panel appears when it does. */
  async function refreshRecovery(): Promise<void> {
    try {
      setRecovery(await getSetupRecovery(projectId));
      setRecoveryCheckFailure(null);
    } catch (error) {
      if (!(error instanceof ApiError && error.isUnavailable)) setRecoveryCheckFailure(error);
    }
  }

  /**
   * Side effects every failed Setup call shares. Returns true when the failure was a lost
   * session, which the wizard shows itself, so the step must not show it again.
   */
  function handleError(error: unknown): boolean {
    if (!(error instanceof ApiError)) return false;
    if (error.code === 'setup_session_missing' || error.code === 'setup_session_expired') {
      setEnded(true);
      return true;
    }
    if (error.code === 'recovery_required') void refreshRecovery();
    return false;
  }

  async function runScan() {
    setScanning(true);
    setScanFailure(null);
    try {
      const scan = await scanSetup(projectId, { mode });
      setData({ ...EMPTY_DATA, scan });
      setMode(scan.mode ?? mode);
      setEnded(false);
      setEditingValues(false);
    } catch (error) {
      setScanFailure(error);
    } finally {
      setScanning(false);
    }
  }

  const blocked = recovery !== null;
  const current = currentStep(data, editingValues);
  const ids = stepsFor(data);
  const currentIndex = current === 'done' ? ids.length : ids.indexOf(current);

  function body(id: StepId, state: 'current' | 'done') {
    const { scan, analysis, plan } = data;
    switch (id) {
      case 'scan':
        return <ScanStep scan={scan} busy={scanning} failure={scanFailure} onScan={() => void runScan()} />;
      case 'evidence':
        return scan ? (
          <EvidenceStep
            projectId={projectId}
            scan={scan}
            state={state}
            approvedCount={data.approvedCount}
            onApproved={(count) => patch({ evidenceApproved: true, approvedCount: count })}
            onApiError={handleError}
          />
        ) : null;
      case 'analyze':
        return (
          <AnalyzeStep
            projectId={projectId}
            analysis={analysis}
            state={state}
            onAnalyzed={(next) => patch({ analysis: next })}
            onApiError={handleError}
            onRestart={() => void runScan()}
          />
        );
      case 'confirm':
        return analysis ? (
          <ConfirmStep
            projectId={projectId}
            analysis={analysis}
            confirmed={data.confirmed}
            state={state}
            canEdit={plan === null}
            onConfirmed={(values) => {
              patch({ confirmed: values });
              setEditingValues(false);
            }}
            onEdit={() => setEditingValues(true)}
            onCancelEdit={() => setEditingValues(false)}
            onApiError={handleError}
            onRestart={() => void runScan()}
          />
        ) : null;
      case 'plan':
        return (
          <PlanStep
            projectId={projectId}
            plan={plan}
            state={state}
            blocked={blocked}
            onPlan={(next) => patch({ plan: next, result: null })}
            onApiError={handleError}
            onRestart={() => void runScan()}
          />
        );
      case 'apply':
        return plan ? (
          <ApplyStep
            projectId={projectId}
            plan={plan}
            result={data.result}
            appliedEarlier={data.appliedEarlier}
            blocked={blocked}
            onPlan={(next) => patch({ plan: next })}
            onResult={(result) => {
              patch({ result });
              if (result.status === 'applied') onChanged();
            }}
            onApiError={handleError}
            onRecoveryCheck={refreshRecovery}
            onRestart={() => void runScan()}
          />
        ) : null;
    }
  }

  return (
    <>
      {recoveryCheckFailure ? (
        <ErrorMessage error={recoveryCheckFailure} onRetry={() => void refreshRecovery()} />
      ) : null}
      <RecoveryPanel
        projectId={projectId}
        recovery={recovery}
        outcome={outcome}
        onRecovered={(next) => {
          setOutcome(next);
          // The session this wizard was working on is over; the next scan starts a fresh one.
          setData(EMPTY_DATA);
          setEditingValues(false);
          onChanged();
          void refreshRecovery();
        }}
        onDismissOutcome={() => setOutcome(null)}
        onRescan={() => {
          setOutcome(null);
          void runScan();
        }}
        onApiError={handleError}
      />
      <section className="card" aria-labelledby="setup-title">
        <h2 id="setup-title">{t('setup.title')}</h2>
        <p className="muted">{t('setup.intro')}</p>
        {ended ? (
          <div>
            <Alert kind="info">{t('setup.sessionEnded.body')}</Alert>
            {scanFailure ? <StepError error={scanFailure} what={t('setup.scan.what')} onRetry={() => void runScan()} /> : null}
            <p className="button-row">
              <button type="button" className="button primary" onClick={() => void runScan()} disabled={scanning}>
                {scanning ? t('setup.scan.working') : t('setup.sessionEnded.button')}
              </button>
            </p>
          </div>
        ) : (
          <ol className="wizard" aria-label={t('setup.listLabel')}>
            {ids.map((id, index) => {
              const state = index < currentIndex ? 'done' : index === currentIndex ? 'current' : 'upcoming';
              return (
                <li
                  key={id}
                  className={`wizard-step wizard-${state}`}
                  aria-current={state === 'current' ? 'step' : undefined}
                >
                  <h3>
                    {t('setup.stepHeading', { n: index + 1, title: t(`setup.steps.${id}`) })}
                    {state === 'upcoming' ? null : (
                      <span className="sr-only"> {t(`setup.stepState.${state}`)}</span>
                    )}
                  </h3>
                  {state === 'upcoming' ? null : body(id, state)}
                </li>
              );
            })}
          </ol>
        )}
      </section>
    </>
  );
}

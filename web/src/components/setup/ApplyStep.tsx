import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../../api/client';
import {
  applySetup,
  approveSetupPlan,
  getSetupPlan,
  type SetupApplyResult,
  type SetupPlan,
} from '../../api/intake';
import { formatNumber } from '../../format';
import { Alert } from '../ui';
import { filesToWrite, Hash } from './PlanStep';
import { StepError } from './StepError';

/** What happened when the plan was applied. */
export function ApplyResult({ result }: { result: SetupApplyResult }) {
  const { t } = useTranslation();
  if (result.status === 'applied') {
    const changes = result.changes;
    const unplanned = result.unplanned_changes ?? [];
    return (
      <>
        <Alert kind="success">
          <p>{t('setup.result.applied')}</p>
          {changes ? (
            <p>
              {t('setup.result.counts', {
                added: formatNumber(changes.added.length),
                changed: formatNumber(changes.changed.length),
                removed: formatNumber(changes.removed.length),
              })}
              {changes.mode_changed.length > 0
                ? ` ${t('setup.result.modeChanged', { count: changes.mode_changed.length, n: formatNumber(changes.mode_changed.length) })}`
                : ''}
              {changes.git_index_changed ? ` ${t('setup.result.gitIndex')}` : ''}
            </p>
          ) : null}
          {result.receipt_id ? (
            <p className="small">
              <bdi dir="ltr">{t('setup.result.receipt', { id: result.receipt_id })}</bdi>
            </p>
          ) : null}
        </Alert>
        {unplanned.length > 0 ? (
          <Alert kind="warning">
            <p>
              <strong>{t('setup.result.unplannedTitle')}</strong>
            </p>
            <p>{t('setup.result.unplanned', { count: unplanned.length, n: formatNumber(unplanned.length) })}</p>
            <ul>
              {unplanned.map((path) => (
                <li key={path} dir="ltr" className="path">
                  {path}
                </li>
              ))}
            </ul>
          </Alert>
        ) : null}
      </>
    );
  }
  if (result.status === 'needs_rollback' || result.status === 'interrupted') {
    return (
      <Alert kind="warning">
        <p>{t(`setup.result.${result.status}`)}</p>
        {result.explanation ? (
          <p className="small" dir="auto">
            {t('setup.result.explanation', { text: result.explanation })}
          </p>
        ) : null}
      </Alert>
    );
  }
  if (result.status === 'failed') return <Alert kind="error">{t('setup.result.failed')}</Alert>;
  return <Alert kind="info">{t('setup.result.other', { status: result.status })}</Alert>;
}

/**
 * Step 6: the explicit confirmation. The button stays disabled until the person ticks that they
 * reviewed the plan, then approves exactly the hash shown and applies that plan.
 */
export function ApplyStep({
  projectId,
  plan,
  result,
  appliedEarlier,
  blocked,
  onPlan,
  onResult,
  onApiError,
  onRecoveryCheck,
  onRestart,
}: {
  projectId: string;
  plan: SetupPlan;
  result: SetupApplyResult | null;
  appliedEarlier: boolean;
  /** An unfinished change must be recovered before anything is applied. */
  blocked: boolean;
  onPlan: (plan: SetupPlan) => void;
  onResult: (result: SetupApplyResult) => void;
  onApiError: (error: unknown) => boolean;
  /** Asks the server whether an unfinished change needs recovery. */
  onRecoveryCheck: () => Promise<void>;
  onRestart: () => void;
}) {
  const { t } = useTranslation();
  // The tick belongs to one plan hash. A new plan (new hash) starts unticked.
  const [reviewedHash, setReviewedHash] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);
  const [planChanged, setPlanChanged] = useState(false);

  const applied = appliedEarlier || result?.status === 'applied';
  const reviewed = reviewedHash === plan.plan_sha256;
  const files = filesToWrite(plan);

  async function approveAndApply() {
    setBusy(true);
    setFailure(null);
    setPlanChanged(false);
    try {
      if (!plan.approved) {
        try {
          await approveSetupPlan(projectId, { plan_id: plan.plan_id, plan_sha256: plan.plan_sha256 });
          onPlan({ ...plan, approved: true });
        } catch (error) {
          if (error instanceof ApiError && error.code === 'plan_changed') {
            // The plan moved on since it was shown: show the new one and ask for a fresh review.
            onPlan(await getSetupPlan(projectId));
            setReviewedHash(null);
            setPlanChanged(true);
            return;
          }
          throw error;
        }
      }
      const done = await applySetup(projectId, plan.plan_id);
      onResult(done);
      if (done.status === 'needs_rollback' || done.status === 'interrupted') await onRecoveryCheck();
    } catch (error) {
      if (!onApiError(error)) setFailure(error);
      // The apply may have started before the call failed, so ask what state it left behind.
      await onRecoveryCheck();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      {result ? <ApplyResult result={result} /> : null}
      {applied && !result ? <Alert kind="success">{t('setup.result.appliedEarlier')}</Alert> : null}
      {!applied ? (
        <div className="confirm approve-panel" role="group" aria-label={t('setup.apply.panelLabel')}>
          <p>{t('setup.apply.intro')}</p>
          <dl className="facts">
            <div className="fact">
              <dt>{t('setup.apply.root')}</dt>
              <dd>
                <Hash value={plan.root_identity} />
              </dd>
            </div>
            <div className="fact">
              <dt>{t('setup.apply.hash')}</dt>
              <dd>
                <Hash value={plan.plan_sha256} />
              </dd>
            </div>
          </dl>
          <p>
            <strong>{t('setup.apply.files', { count: files, n: formatNumber(files) })}</strong>
          </p>
          {plan.approved ? <p>{t('setup.apply.alreadyApproved')}</p> : null}
          {planChanged ? <Alert kind="warning">{t('setup.apply.planChanged')}</Alert> : null}
          {blocked ? <Alert kind="warning">{t('setup.apply.blocked')}</Alert> : null}
          {files === 0 ? <p className="muted">{t('setup.apply.nothing')}</p> : null}
          {failure ? (
            <StepError error={failure} what={t('setup.apply.what')} onRestart={onRestart} />
          ) : null}
          <div className="field">
            <label htmlFor="setup-reviewed">
              <input
                id="setup-reviewed"
                type="checkbox"
                checked={reviewed}
                onChange={(event) => setReviewedHash(event.target.checked ? plan.plan_sha256 : null)}
                disabled={busy}
              />{' '}
              {t('setup.apply.reviewed')}
            </label>
            {!reviewed ? <p className="hint">{t('setup.apply.reviewedHint')}</p> : null}
          </div>
          <p className="button-row">
            <button
              type="button"
              className="button primary"
              onClick={() => void approveAndApply()}
              disabled={!reviewed || busy || blocked || files === 0}
            >
              {busy ? t('setup.apply.working') : t('setup.apply.button')}
            </button>
          </p>
        </div>
      ) : null}
    </div>
  );
}

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  buildSetupPlan,
  decideConflicts,
  getSetupPlan,
  type PlanAction,
  type SetupPlan,
} from '../../api/intake';
import { formatNumber } from '../../format';
import { Alert } from '../ui';
import { StepError } from './StepError';
import { shortHash, unresolvedConflicts } from './wizard';

const ACTIONS: PlanAction[] = ['conflict', 'change', 'create', 'preserve'];
const INITIAL_ROWS = 100;

/** A hash is long. Show enough to compare by eye, with the whole value on hover. */
export function Hash({ value }: { value: string }) {
  return (
    <code dir="ltr" title={value}>
      {shortHash(value)}
    </code>
  );
}

function countActions(plan: SetupPlan): Record<PlanAction, number> {
  return (
    plan.summary ??
    (Object.fromEntries(
      ACTIONS.map((action) => [action, plan.operations.filter((o) => o.action === action).length]),
    ) as Record<PlanAction, number>)
  );
}

/** The files that will be written: everything the plan creates or changes. */
export function filesToWrite(plan: SetupPlan): number {
  const counts = countActions(plan);
  return counts.create + counts.change;
}

/**
 * Step 5: build the plan, read it, and decide every conflict. Deciding conflicts produces a
 * new plan with a new hash and withdraws any approval, and the page says so.
 */
export function PlanStep({
  projectId,
  plan,
  state,
  blocked,
  onPlan,
  onApiError,
  onRestart,
}: {
  projectId: string;
  plan: SetupPlan | null;
  state: 'current' | 'done';
  /** An unfinished change must be recovered before anything is planned. */
  blocked: boolean;
  onPlan: (plan: SetupPlan) => void;
  onApiError: (error: unknown) => boolean;
  onRestart: () => void;
}) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState<'build' | 'reload' | 'decide' | null>(null);
  const [failure, setFailure] = useState<unknown>(null);
  const [decidedHash, setDecidedHash] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);

  async function run(kind: 'build' | 'reload', load: () => Promise<SetupPlan>) {
    setBusy(kind);
    setFailure(null);
    try {
      onPlan(await load());
    } catch (error) {
      if (!onApiError(error)) setFailure(error);
    } finally {
      setBusy(null);
    }
  }

  async function decide(decisions: { path: string; choice: string }[]) {
    if (!plan) return;
    setBusy('decide');
    setFailure(null);
    try {
      const next = await decideConflicts(projectId, { plan_id: plan.plan_id, decisions });
      setDecidedHash(next.plan_sha256);
      onPlan(next);
    } catch (error) {
      if (!onApiError(error)) setFailure(error);
    } finally {
      setBusy(null);
    }
  }

  const error = failure ? (
    <StepError
      error={failure}
      what={t('setup.plan.what')}
      onRetry={plan ? () => void run('reload', () => getSetupPlan(projectId)) : () => void run('build', () => buildSetupPlan(projectId))}
      onRestart={onRestart}
    />
  ) : null;

  if (!plan) {
    return (
      <div>
        <p>{t('setup.plan.intro')}</p>
        {blocked ? <Alert kind="warning">{t('setup.plan.blocked')}</Alert> : null}
        {error}
        <p className="button-row">
          <button
            type="button"
            className="button primary"
            onClick={() => void run('build', () => buildSetupPlan(projectId))}
            disabled={busy !== null || blocked || state !== 'current'}
          >
            {busy === 'build' ? t('setup.plan.building') : t('setup.plan.build')}
          </button>
        </p>
      </div>
    );
  }

  const counts = countActions(plan);
  const rows = showAll ? plan.operations : plan.operations.slice(0, INITIAL_ROWS);
  const unresolved = unresolvedConflicts(plan);

  return (
    <div>
      <p>
        {t('setup.plan.hashLabel')}: <Hash value={plan.plan_sha256} />{' '}
        <span className="muted">{plan.approved ? t('setup.plan.approved') : t('setup.plan.notApproved')}</span>
      </p>
      {decidedHash === plan.plan_sha256 ? (
        <Alert kind="info">{t('setup.plan.decided', { hash: shortHash(plan.plan_sha256) })}</Alert>
      ) : null}
      <ul className="plan-summary">
        {ACTIONS.map((action) => (
          <li key={action} className={`plan-chip plan-${action}`}>
            {t(`setup.plan.summary.${action}`, { n: formatNumber(counts[action]) })}
          </li>
        ))}
      </ul>
      {plan.operations.length === 0 ? (
        <p>{t('setup.plan.nothingToChange')}</p>
      ) : (
        <div className="table-wrap">
          <table>
            <caption className="sr-only">{t('setup.plan.tableCaption')}</caption>
            <thead>
              <tr>
                <th scope="col">{t('setup.plan.colAction')}</th>
                <th scope="col">{t('setup.plan.colPath')}</th>
                <th scope="col">{t('setup.plan.colNote')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((op) => (
                <tr key={`${op.action}:${op.path}`}>
                  <td>
                    <span className={`plan-chip plan-${op.action}`}>{t(`setup.plan.action.${op.action}`)}</span>
                  </td>
                  <td dir="ltr" className="path">
                    {op.path}
                  </td>
                  <td dir="auto">{op.note ?? ''}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {!showAll && plan.operations.length > INITIAL_ROWS ? (
        <p>
          <button type="button" className="link-button" onClick={() => setShowAll(true)}>
            {t('setup.plan.showAll', { n: formatNumber(plan.operations.length) })}
          </button>
        </p>
      ) : null}
      {plan.components.length > 0 ? (
        <>
          <h4>{t('setup.plan.components')}</h4>
          <ul>
            {plan.components.map((c) => (
              <li key={c.name}>
                <bdi dir="auto">{c.name}</bdi> <span className="muted">{c.version}</span>
              </li>
            ))}
          </ul>
        </>
      ) : null}

      {plan.conflicts.length > 0 ? (
        <ConflictForm
          key={plan.plan_sha256}
          plan={plan}
          unresolved={unresolved}
          busy={busy === 'decide'}
          disabled={blocked || busy !== null}
          onDecide={(decisions) => void decide(decisions)}
        />
      ) : null}

      {blocked ? <Alert kind="warning">{t('setup.plan.blocked')}</Alert> : null}
      {error}
      <p>
        <button
          type="button"
          className="button secondary"
          onClick={() => void run('reload', () => getSetupPlan(projectId))}
          disabled={busy !== null}
        >
          {busy === 'reload' ? t('setup.plan.reloading') : t('setup.plan.reload')}
        </button>
      </p>
    </div>
  );
}

function ConflictForm({
  plan,
  unresolved,
  busy,
  disabled,
  onDecide,
}: {
  plan: SetupPlan;
  unresolved: number;
  busy: boolean;
  disabled: boolean;
  onDecide: (decisions: { path: string; choice: string }[]) => void;
}) {
  const { t } = useTranslation();
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const choiceFor = (path: string, selected: string | null) => drafts[path] ?? selected ?? '';

  const decisions = plan.conflicts
    .map((c) => ({ path: c.path, choice: choiceFor(c.path, c.selected) }))
    .filter((d) => d.choice !== '');
  const changed = plan.conflicts.some((c) => choiceFor(c.path, c.selected) !== (c.selected ?? ''));
  const complete = plan.conflicts.every((c) => c.options.length === 0 || choiceFor(c.path, c.selected) !== '');

  return (
    <section aria-labelledby="conflicts-title" className="conflicts">
      <h4 id="conflicts-title">{t('setup.plan.conflictsTitle')}</h4>
      <p>{t('setup.plan.conflictsIntro')}</p>
      {unresolved > 0 ? (
        <Alert kind="warning">{t('setup.plan.unresolved', { count: unresolved, n: formatNumber(unresolved) })}</Alert>
      ) : null}
      {plan.conflicts.map((conflict, index) => (
        <fieldset key={conflict.path} className="conflict">
          <legend>
            <bdi dir="ltr">{t('setup.plan.choiceLegend', { path: conflict.path })}</bdi>
          </legend>
          {conflict.options.length === 0 ? (
            <p className="muted">{t('setup.plan.noOptions')}</p>
          ) : (
            conflict.options.map((option) => (
              <label key={option} className="choice">
                <input
                  type="radio"
                  name={`conflict-${index}`}
                  value={option}
                  checked={choiceFor(conflict.path, conflict.selected) === option}
                  onChange={() => setDrafts((previous) => ({ ...previous, [conflict.path]: option }))}
                  disabled={disabled}
                />{' '}
                {t(`setup.plan.choice.${option}`, { defaultValue: option })}
              </label>
            ))
          )}
        </fieldset>
      ))}
      <p className="button-row">
        <button
          type="button"
          className="button primary"
          onClick={() => onDecide(decisions)}
          disabled={disabled || !changed || !complete}
        >
          {busy ? t('setup.plan.applyingDecisions') : t('setup.plan.applyDecisions')}
        </button>
      </p>
    </section>
  );
}

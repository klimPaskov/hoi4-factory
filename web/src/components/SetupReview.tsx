import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import {
  applySetup,
  getSetupPlan,
  scanSetup,
  type PlanAction,
  type SetupApplyResult,
  type SetupPlan,
  type SetupScan,
} from '../api/intake';
import { formatNumber } from '../format';
import { Alert, ErrorMessage, Loading, NotAvailableYet } from './ui';

const ACTIONS: PlanAction[] = ['conflict', 'change', 'create', 'preserve'];
const INITIAL_ROWS = 100;

function Failure({ error, what, onRetry }: { error: unknown; what: string; onRetry?: () => void }) {
  if (error instanceof ApiError && error.isUnavailable) return <NotAvailableYet what={what} />;
  return <ErrorMessage error={error} onRetry={onRetry} />;
}

/**
 * Scan, dry run and apply. Nothing is written until the user applies the exact plan shown.
 * The whole panel depends on the pending Setup routes, so each step handles "not available yet".
 */
export function SetupReview({
  projectId,
  scan,
  onScan,
}: {
  projectId: string;
  scan: SetupScan | null;
  onScan: (scan: SetupScan) => void;
}) {
  const { t } = useTranslation();
  const [scanning, setScanning] = useState(false);
  const [scanFailure, setScanFailure] = useState<unknown>(null);
  const [plan, setPlan] = useState<
    { status: 'idle' } | { status: 'loading' } | { status: 'ready'; plan: SetupPlan } | { status: 'error'; error: unknown }
  >({ status: 'idle' });
  const [confirming, setConfirming] = useState(false);
  const [applying, setApplying] = useState(false);
  const [applyFailure, setApplyFailure] = useState<unknown>(null);
  const [result, setResult] = useState<SetupApplyResult | null>(null);
  const [showAll, setShowAll] = useState(false);

  async function runScan() {
    setScanning(true);
    setScanFailure(null);
    setPlan({ status: 'idle' });
    setResult(null);
    try {
      onScan(await scanSetup(projectId, { mode: 'existing' }));
    } catch (error) {
      setScanFailure(error);
    } finally {
      setScanning(false);
    }
  }

  async function loadPlan() {
    setPlan({ status: 'loading' });
    setConfirming(false);
    setResult(null);
    try {
      setPlan({ status: 'ready', plan: await getSetupPlan(projectId) });
    } catch (error) {
      setPlan({ status: 'error', error });
    }
  }

  async function apply(planId: string) {
    setApplying(true);
    setApplyFailure(null);
    try {
      setResult(await applySetup(projectId, planId));
      setConfirming(false);
    } catch (error) {
      setApplyFailure(error);
    } finally {
      setApplying(false);
    }
  }

  return (
    <section className="card" aria-labelledby="setup-title">
      <h2 id="setup-title">{t('setup.title')}</h2>
      <p className="muted">{t('setup.intro')}</p>

      <h3>{t('setup.scanTitle')}</h3>
      {scanFailure ? <Failure error={scanFailure} what={t('setup.scanWhat')} onRetry={() => void runScan()} /> : null}
      {scan ? (
        <>
          {scan.incomplete ? <Alert kind="warning">{t('setup.incomplete')}</Alert> : null}
          {scan.facts && scan.facts.length > 0 ? (
            <dl className="facts">
              {scan.facts.map((fact) => (
                <div key={fact.label} className="fact">
                  <dt dir="auto">{fact.label}</dt>
                  <dd dir="auto">{fact.value}</dd>
                </div>
              ))}
            </dl>
          ) : (
            <p>{t('setup.scanDone')}</p>
          )}
        </>
      ) : null}
      <p>
        <button type="button" className="button secondary" onClick={() => void runScan()} disabled={scanning}>
          {scanning ? t('setup.scanning') : scan ? t('setup.scanAgain') : t('setup.scan')}
        </button>
      </p>

      {scan ? (
        <>
          <h3>{t('setup.planTitle')}</h3>
          <p className="muted small">{t('setup.planIntro')}</p>
          {plan.status === 'idle' ? (
            <button type="button" className="button secondary" onClick={() => void loadPlan()}>
              {t('setup.showPlan')}
            </button>
          ) : null}
          {plan.status === 'loading' ? <Loading label={t('setup.planLoading')} /> : null}
          {plan.status === 'error' ? (
            <Failure error={plan.error} what={t('setup.planWhat')} onRetry={() => void loadPlan()} />
          ) : null}
          {plan.status === 'ready' ? (
            <PlanView
              plan={plan.plan}
              showAll={showAll}
              onShowAll={() => setShowAll(true)}
              confirming={confirming}
              applying={applying}
              result={result}
              failure={applyFailure}
              onAsk={() => setConfirming(true)}
              onCancel={() => setConfirming(false)}
              onApply={() => void apply(plan.plan.plan_id)}
              onRefresh={() => void loadPlan()}
            />
          ) : null}
        </>
      ) : null}
    </section>
  );
}

function PlanView(props: {
  plan: SetupPlan;
  showAll: boolean;
  onShowAll: () => void;
  confirming: boolean;
  applying: boolean;
  result: SetupApplyResult | null;
  failure: unknown;
  onAsk: () => void;
  onCancel: () => void;
  onApply: () => void;
  onRefresh: () => void;
}) {
  const { t } = useTranslation();
  const { plan } = props;
  const counts = Object.fromEntries(ACTIONS.map((a) => [a, plan.operations.filter((o) => o.action === a).length])) as Record<PlanAction, number>;
  const rows = props.showAll ? plan.operations : plan.operations.slice(0, INITIAL_ROWS);
  const blocked = counts.conflict > 0;

  return (
    <div>
      <ul className="plan-summary">
        {ACTIONS.map((action) => (
          <li key={action} className={`plan-chip plan-${action}`}>
            {t(`setup.actions.${action}`, { n: formatNumber(counts[action]) })}
          </li>
        ))}
      </ul>
      {blocked ? <Alert kind="warning">{t('setup.conflicts')}</Alert> : null}
      {plan.operations.length === 0 ? (
        <p>{t('setup.nothingToChange')}</p>
      ) : (
        <div className="table-wrap">
          <table>
            <caption className="sr-only">{t('setup.tableCaption')}</caption>
            <thead>
              <tr>
                <th scope="col">{t('setup.colAction')}</th>
                <th scope="col">{t('setup.colPath')}</th>
                <th scope="col">{t('setup.colNote')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((op) => (
                <tr key={`${op.action}:${op.path}`}>
                  <td>
                    <span className={`plan-chip plan-${op.action}`}>{t(`setup.action.${op.action}`)}</span>
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
      {!props.showAll && plan.operations.length > INITIAL_ROWS ? (
        <p>
          <button type="button" className="link-button" onClick={props.onShowAll}>
            {t('setup.showAll', { n: formatNumber(plan.operations.length) })}
          </button>
        </p>
      ) : null}
      {plan.components && plan.components.length > 0 ? (
        <>
          <h4>{t('setup.components')}</h4>
          <ul>
            {plan.components.map((c) => (
              <li key={c.name}>
                <bdi dir="auto">{c.name}</bdi> <span className="muted">{c.version}</span>
              </li>
            ))}
          </ul>
        </>
      ) : null}

      {props.failure ? <Failure error={props.failure} what={t('setup.applyWhat')} /> : null}
      {props.result ? <ApplyResult result={props.result} /> : null}

      {!props.result && !props.confirming ? (
        <p className="button-row">
          <button type="button" className="button primary" onClick={props.onAsk} disabled={blocked || plan.operations.length === 0}>
            {t('setup.apply')}
          </button>
          <button type="button" className="button secondary" onClick={props.onRefresh}>
            {t('setup.refreshPlan')}
          </button>
        </p>
      ) : null}
      {!props.result && props.confirming ? (
        <div className="confirm" role="group" aria-label={t('setup.confirmLabel')}>
          <p>{t('setup.confirm', { count: plan.operations.length - counts.preserve, n: formatNumber(plan.operations.length - counts.preserve) })}</p>
          <div className="button-row">
            <button type="button" className="button primary" onClick={props.onApply} disabled={props.applying}>
              {props.applying ? t('setup.applying') : t('setup.confirmButton')}
            </button>
            <button type="button" className="button secondary" onClick={props.onCancel}>
              {t('common.cancel')}
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function ApplyResult({ result }: { result: SetupApplyResult }) {
  const { t } = useTranslation();
  if (result.status === 'applied') return <Alert kind="success">{t('setup.result.applied')}</Alert>;
  if (result.status === 'needs_rollback') return <Alert kind="warning">{t('setup.result.needs_rollback')}</Alert>;
  if (result.status === 'failed') return <Alert kind="error">{t('setup.result.failed')}</Alert>;
  return <Alert kind="info">{t('setup.result.other', { status: result.status })}</Alert>;
}

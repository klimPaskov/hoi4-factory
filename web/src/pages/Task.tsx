import { useState } from 'react';
import { Link, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import {
  ACTIVE_ATTEMPT_STATES,
  getTask,
  interruptAttempt,
  pauseTask,
  resumeTask,
  revokeGrant,
  startAttempt,
  type Attempt,
  type TaskDetail,
} from '../api/tasks';
import { TaskEvents } from '../components/TaskEvents';
import { Alert, ErrorMessage, Loading, NotAvailableYet } from '../components/ui';
import { formatDateTime, formatMicros } from '../format';
import { useAsync, usePageTitle } from '../hooks';

export function TaskPage() {
  const { id = '', taskId = '' } = useParams();
  return <TaskView key={`${id}/${taskId}`} projectId={id} taskId={taskId} />;
}

function TaskView({ projectId, taskId }: { projectId: string; taskId: string }) {
  const { t } = useTranslation();
  const state = useAsync((signal) => getTask(projectId, taskId, signal));
  usePageTitle(t('tasks.title'));

  return (
    <>
      <p className="breadcrumb">
        <Link to={`/projects/${encodeURIComponent(projectId)}/tasks`}>{t('tasks.backToTasks')}</Link>
      </p>
      {state.status === 'loading' ? <Loading label={t('tasks.loadingOne')} /> : null}
      {state.status === 'error' ? (
        state.error instanceof ApiError && state.error.isUnavailable ? (
          <NotAvailableYet what={t('tasks.what')} />
        ) : (
          <>
            <h1>{t('tasks.errorTitle')}</h1>
            <ErrorMessage error={state.error} onRetry={state.reload} />
          </>
        )
      ) : null}
      {state.status === 'ready' ? (
        <TaskBody projectId={projectId} taskId={taskId} detail={state.data} onDetail={state.replace} />
      ) : null}
    </>
  );
}

function TaskBody({
  projectId,
  taskId,
  detail,
  onDetail,
}: {
  projectId: string;
  taskId: string;
  detail: TaskDetail;
  onDetail: (detail: TaskDetail) => void;
}) {
  const { t } = useTranslation();
  const { task, attempts, pending_effects: pending } = detail;
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);
  const [conflict, setConflict] = useState(false);
  const active = attempts.find((a) => ACTIVE_ATTEMPT_STATES.includes(a.state));

  // Reads the task again without the loading state, so the page does not jump.
  async function refresh() {
    try {
      onDetail(await getTask(projectId, taskId));
    } catch (error) {
      setFailure((current: unknown) => current ?? error);
    }
  }

  async function act(action: () => Promise<unknown>) {
    setBusy(true);
    setFailure(null);
    setConflict(false);
    try {
      await action();
    } catch (error) {
      if (error instanceof ApiError && error.code === 'version_conflict') setConflict(true);
      else setFailure(error);
    }
    await refresh();
    setBusy(false);
  }

  return (
    <>
      <h1>
        <bdi dir="auto">{task.objective}</bdi>
      </h1>
      {conflict ? <Alert kind="warning">{t('tasks.conflict')}</Alert> : null}
      {failure ? <ErrorMessage error={failure} /> : null}
      {task.paused ? (
        <Alert kind="warning">
          {t('tasks.pausedNote', { reason: task.pause_reason ? `: ${task.pause_reason}` : '' })}
        </Alert>
      ) : null}

      <section className="card" aria-labelledby="task-details-title">
        <h2 id="task-details-title">{t('tasks.title')}</h2>
        <dl className="facts">
          <dt>{t('tasks.fields.state')}</dt>
          <dd>
            {t(`tasks.states.${task.state}`)}
            {task.paused ? ` · ${t('tasks.paused')}` : ''}
          </dd>
          <dt>{t('tasks.fields.mode')}</dt>
          <dd>{t(`tasks.modes.${task.mode}`)}</dd>
          {task.allowed_paths.length > 0 ? (
            <>
              <dt>{t('tasks.fields.paths')}</dt>
              <dd dir="ltr">{task.allowed_paths.join(', ')}</dd>
            </>
          ) : null}
          <dt>{t('tasks.fields.budget')}</dt>
          <dd>{formatMicros(task.budget_micros)}</dd>
          <dt>{t('tasks.fields.spent')}</dt>
          <dd>
            {formatMicros(task.spent_micros)}
            {task.spend_unknown ? ` · ${t('tasks.spentUnknown')}` : ''}
          </dd>
          <dt>{t('tasks.fields.model')}</dt>
          <dd dir="ltr">{task.model.profile_id}</dd>
          <dt>{t('tasks.fields.funding')}</dt>
          <dd>{t(`tasks.funded.${task.model.funding}`)}</dd>
          <dt>{t('tasks.fields.turns')}</dt>
          <dd>{task.max_turns}</dd>
        </dl>
        <div className="button-row">
          {!active && !task.paused ? (
            <button
              type="button"
              className="button primary"
              disabled={busy}
              onClick={() => void act(() => startAttempt(projectId, taskId, task.version))}
            >
              {busy ? t('tasks.actions.working') : t('tasks.actions.start')}
            </button>
          ) : null}
          {task.paused ? (
            <button
              type="button"
              className="button secondary"
              disabled={busy}
              onClick={() => void act(() => resumeTask(projectId, taskId, task.version))}
            >
              {t('tasks.actions.resume')}
            </button>
          ) : (
            <button
              type="button"
              className="button secondary"
              disabled={busy}
              onClick={() => void act(() => pauseTask(projectId, taskId, task.version))}
            >
              {t('tasks.actions.pause')}
            </button>
          )}
        </div>
      </section>

      {pending.length > 0 ? (
        <section className="card" aria-labelledby="task-pending-title">
          <h2 id="task-pending-title">{t('tasks.pendingTitle')}</h2>
          <Alert kind="warning">{t('tasks.pendingBody')}</Alert>
          <ul>
            {pending.map((effect) => (
              <li key={effect.operation_id} dir="auto">
                {t('tasks.pendingItem', {
                  tool: effect.tool,
                  status: effect.status,
                  id: effect.operation_id,
                })}
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <section className="card" aria-labelledby="task-attempts-title">
        <h2 id="task-attempts-title">{t('tasks.attemptsTitle')}</h2>
        {attempts.length === 0 ? <p className="muted">{t('tasks.noAttempts')}</p> : null}
        {attempts.map((attempt) => (
          <AttemptCard
            key={attempt.id}
            attempt={attempt}
            busy={busy}
            onInterrupt={() => void act(() => interruptAttempt(projectId, taskId, attempt.id))}
            onRevoke={() => void act(() => revokeGrant(projectId, taskId, attempt.grant.id))}
          />
        ))}
      </section>

      <TaskEvents projectId={projectId} taskId={taskId} onNew={() => void refresh()} />
    </>
  );
}

function AttemptCard({
  attempt,
  busy,
  onInterrupt,
  onRevoke,
}: {
  attempt: Attempt;
  busy: boolean;
  onInterrupt: () => void;
  onRevoke: () => void;
}) {
  const { t } = useTranslation();
  const live = ACTIVE_ATTEMPT_STATES.includes(attempt.state);
  const { grant, outcome } = attempt;
  return (
    <article className="card" aria-label={t('tasks.attempt', { n: attempt.attempt_no })}>
      <h3>{t('tasks.attempt', { n: attempt.attempt_no })}</h3>
      <p>
        <strong>{t(`tasks.attemptStates.${attempt.state}`)}</strong>
      </p>
      <p className="muted small">
        {attempt.started_at ? t('tasks.started', { date: formatDateTime(attempt.started_at) }) : ''}
        {attempt.finished_at ? ` · ${t('tasks.finished', { date: formatDateTime(attempt.finished_at) })}` : ''}
      </p>
      {attempt.error_code ? <p>{t('tasks.error', { code: attempt.error_code })}</p> : null}
      <dl className="facts">
        <dt>{t('tasks.grantTitle')}</dt>
        <dd>{grant.revoked ? t('tasks.grantRevoked') : t('tasks.grantActive')}</dd>
        <dt>{t('tasks.grantClasses')}</dt>
        <dd dir="ltr">{grant.operation_classes.join(', ') || t('tasks.none')}</dd>
        <dt>{t('tasks.grantRoots')}</dt>
        <dd dir="ltr">{grant.allowed_roots.join(', ') || t('tasks.none')}</dd>
        <dt>{t('tasks.grantEpoch')}</dt>
        <dd>{grant.revocation_epoch}</dd>
        <dt>{t('tasks.grantExpires')}</dt>
        <dd>{formatDateTime(grant.expires_at)}</dd>
      </dl>
      {outcome ? (
        <div>
          <h4>{t('tasks.outcomeTitle')}</h4>
          <p dir="auto">{outcome.summary}</p>
          <dl className="facts">
            <dt>{t('tasks.outcomeChanged')}</dt>
            <dd dir="ltr">{outcome.changed_files.join(', ') || t('tasks.none')}</dd>
            {outcome.branch ? (
              <>
                <dt>{t('tasks.outcomeBranch')}</dt>
                <dd dir="ltr">{outcome.branch}</dd>
              </>
            ) : null}
            {outcome.commit ? (
              <>
                <dt>{t('tasks.outcomeCommit')}</dt>
                <dd dir="ltr">{outcome.commit}</dd>
              </>
            ) : null}
            {outcome.open_questions.length > 0 ? (
              <>
                <dt>{t('tasks.outcomeQuestions')}</dt>
                <dd dir="auto">{outcome.open_questions.join(' ')}</dd>
              </>
            ) : null}
          </dl>
        </div>
      ) : null}
      {live ? (
        <div className="button-row">
          {attempt.state !== 'interrupt_requested' && attempt.state !== 'reconciling' ? (
            <button type="button" className="button danger-outline" disabled={busy} onClick={onInterrupt}>
              {t('tasks.actions.interrupt')}
            </button>
          ) : null}
          {!grant.revoked ? (
            <button type="button" className="button danger-outline" disabled={busy} onClick={onRevoke}>
              {t('tasks.actions.revoke')}
            </button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

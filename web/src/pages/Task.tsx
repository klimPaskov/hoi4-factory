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
import { Alert, Badge, ErrorMessage, Icon, Loading, Meter, NotAvailableYet, type Tone } from '../components/ui';
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

// The task page follows docs/design/mockups/61-task-b-story (provisional overnight choice): the
// state, spend and controls on top, the live activity below, and the details folded away.

function taskTone(task: TaskDetail['task']): Tone {
  if (task.paused) return 'warning';
  switch (task.state) {
    case 'running':
      return 'accent';
    case 'waiting_for_input':
      return 'warning';
    case 'in_review':
      return 'info';
    case 'completed':
      return 'success';
    case 'blocked':
      return 'danger';
    default:
      return 'neutral';
  }
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
  const needsAttention = task.state === 'in_review' || task.state === 'blocked' || task.state === 'waiting_for_input';

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

      <section className="card task-status" aria-labelledby="task-status-title">
        <h2 id="task-status-title" className="sr-only">
          {t('tasks.statusTitle')}
        </h2>
        <div className="task-status-head">
          <span className="task-status-state">
            <Badge tone={taskTone(task)} live={task.state === 'running' && !task.paused}>
              {t(`tasks.states.${task.state}`)}
            </Badge>
            {task.paused ? <Badge tone="warning">{t('tasks.paused')}</Badge> : null}
          </span>
          <span className="muted small">
            {t(`tasks.modes.${task.mode}`)} {'· '}
            <bdi dir="ltr">{task.model.profile_id}</bdi>
          </span>
        </div>
        <Meter
          label={t('tasks.fields.spent')}
          value={task.spent_micros}
          max={task.budget_micros}
          text={
            <>
              {t('tasks.spentOf', { amount: formatMicros(task.spent_micros), budget: formatMicros(task.budget_micros) })}
              {task.spend_unknown ? ` · ${t('tasks.spentUnknown')}` : ''}
            </>
          }
        />
        <div className="button-row">
          {!active && !task.paused ? (
            <button
              type="button"
              className="button primary"
              disabled={busy}
              onClick={() => void act(() => startAttempt(projectId, taskId, task.version))}
            >
              <Icon name="play" size="sm" />
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
              <Icon name="play" size="sm" />
              {t('tasks.actions.resume')}
            </button>
          ) : (
            <button
              type="button"
              className="button secondary"
              disabled={busy}
              onClick={() => void act(() => pauseTask(projectId, taskId, task.version))}
            >
              <Icon name="pause" size="sm" />
              {t('tasks.actions.pause')}
            </button>
          )}
          {active && active.state !== 'interrupt_requested' && active.state !== 'reconciling' ? (
            <button
              type="button"
              className="button danger-outline"
              disabled={busy}
              onClick={() => void act(() => interruptAttempt(projectId, taskId, active.id))}
            >
              <Icon name="stop" size="sm" />
              {t('tasks.actions.interrupt')}
            </button>
          ) : null}
          {active && !active.grant.revoked ? (
            <button
              type="button"
              className="button ghost danger-text"
              disabled={busy}
              onClick={() => void act(() => revokeGrant(projectId, taskId, active.grant.id))}
            >
              <Icon name="revoke" size="sm" />
              {t('tasks.actions.revoke')}
            </button>
          ) : null}
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

      <TaskEvents projectId={projectId} taskId={taskId} onNew={() => void refresh()} />

      <details className="later" open={needsAttention}>
        <summary>
          <h2>
            {t('tasks.attemptsTitle')} <span className="count">{attempts.length}</span>
          </h2>
        </summary>
        {attempts.length === 0 ? <p className="muted">{t('tasks.noAttempts')}</p> : null}
        {attempts.map((attempt) => (
          <AttemptCard key={attempt.id} attempt={attempt} />
        ))}
      </details>

      <details className="later">
        <summary>
          <h2>{t('tasks.settingsTitle')}</h2>
        </summary>
        <dl className="facts">
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
          <dt>{t('tasks.fields.model')}</dt>
          <dd dir="ltr">{task.model.profile_id}</dd>
          <dt>{t('tasks.fields.funding')}</dt>
          <dd>{t(`tasks.funded.${task.model.funding}`)}</dd>
          <dt>{t('tasks.fields.turns')}</dt>
          <dd>{task.max_turns}</dd>
        </dl>
      </details>
    </>
  );
}

function AttemptCard({ attempt }: { attempt: Attempt }) {
  const { t } = useTranslation();
  const { grant, outcome } = attempt;
  return (
    <article className="card attempt" aria-label={t('tasks.attempt', { n: attempt.attempt_no })}>
      <h3>{t('tasks.attempt', { n: attempt.attempt_no })}</h3>
      <p>
        <strong>{t(`tasks.attemptStates.${attempt.state}`)}</strong>
      </p>
      <p className="muted small">
        {attempt.started_at ? t('tasks.started', { date: formatDateTime(attempt.started_at) }) : ''}
        {attempt.finished_at ? ` · ${t('tasks.finished', { date: formatDateTime(attempt.finished_at) })}` : ''}
      </p>
      {attempt.error_code ? <p>{t('tasks.error', { code: attempt.error_code })}</p> : null}
      {outcome ? (
        <div className="attempt-outcome">
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
      <h4>{t('tasks.grantTitle')}</h4>
      <dl className="facts">
        <dt>{t('tasks.grantState')}</dt>
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
    </article>
  );
}

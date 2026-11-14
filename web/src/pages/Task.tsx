import { useEffect, useState } from 'react';
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
import { listProposals, type Proposal } from '../api/focus';
import { TaskEvents } from '../components/TaskEvents';
import { Alert, Badge, ErrorMessage, Icon, Loading, Meter, NotAvailableYet, Tabs, type Tone } from '../components/ui';
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

/**
 * The way from a task to its focus tree and to changes that wait for review. It shows nothing
 * for a task that has not touched a focus tree, or on a server without the focus routes.
 */
function FocusLinks({ projectId, taskId, version }: { projectId: string; taskId: string; version: number }) {
  const { t } = useTranslation();
  const [proposals, setProposals] = useState<Proposal[]>([]);
  useEffect(() => {
    const controller = new AbortController();
    listProposals(projectId, taskId, controller.signal).then(
      (list) => {
        if (!controller.signal.aborted) setProposals(list);
      },
      () => undefined,
    );
    return () => controller.abort();
  }, [projectId, taskId, version]);
  const trees = [...new Set(proposals.map((p) => p.treeId))];
  if (trees.length === 0) return null;
  const base = `/projects/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(taskId)}`;
  const waiting = proposals.filter((p) => p.state === 'applied');
  return (
    <nav className="button-row" aria-label={t('tasks.changesTitle')}>
      {trees.map((tree) => (
        <Link key={tree} className="button secondary" to={`${base}/focus/${encodeURIComponent(tree)}`}>
          {trees.length === 1 ? t('tasks.openFocusTree') : t('tasks.openFocusTreeNamed', { name: tree })}
        </Link>
      ))}
      {waiting.length > 0 ? (
        <Link className="button secondary" to={`${base}/changes/${encodeURIComponent((waiting[waiting.length - 1] as Proposal).id)}`}>
          {t('tasks.reviewChange')}
        </Link>
      ) : null}
    </nav>
  );
}

// The task page follows docs/design/mockups/62-task-c-tabs (the maintainer's choice C): the
// title and state, the controls beside Activity, Attempts, Permission and Settings tabs, and the
// spend meter under them.

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
  const latest = attempts.reduce<Attempt | undefined>(
    (found, attempt) => (!found || attempt.attempt_no > found.attempt_no ? attempt : found),
    undefined,
  );
  const needsAttention = task.state === 'in_review' || task.state === 'blocked' || task.state === 'waiting_for_input';
  // A task that needs review opens on its attempts, where the result and open questions are.
  const [tab, setTab] = useState(needsAttention && attempts.length > 0 ? 'attempts' : 'activity');

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

  const controls = (
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
  );

  return (
    <>
      <div className="task-title">
        <h1>
          <bdi dir="auto">{task.objective}</bdi>
        </h1>
        <Badge tone={taskTone(task)} live={task.state === 'running' && !task.paused}>
          {t(`tasks.states.${task.state}`)}
        </Badge>
        {task.paused ? <Badge tone="warning">{t('tasks.paused')}</Badge> : null}
      </div>
      {conflict ? <Alert kind="warning">{t('tasks.conflict')}</Alert> : null}
      {failure ? <ErrorMessage error={failure} /> : null}
      {task.paused ? (
        <Alert kind="warning">
          {t('tasks.pausedNote', { reason: task.pause_reason ? `: ${task.pause_reason}` : '' })}
        </Alert>
      ) : null}

      <FocusLinks projectId={projectId} taskId={taskId} version={task.last_seq} />

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

      <Tabs
        label={t('tasks.tabsLabel')}
        value={tab}
        onChange={setTab}
        aside={controls}
        tabs={[
          {
            id: 'activity',
            label: t('tasks.eventsTitle'),
            panel: <TaskEvents projectId={projectId} taskId={taskId} onNew={() => void refresh()} />,
          },
          {
            id: 'attempts',
            label: (
              <>
                {t('tasks.attemptsTitle')} <span className="count">{attempts.length}</span>
              </>
            ),
            panel: (
              <>
                {attempts.length === 0 ? <p className="muted">{t('tasks.noAttempts')}</p> : null}
                {attempts.map((attempt) => (
                  <AttemptCard key={attempt.id} attempt={attempt} />
                ))}
              </>
            ),
          },
          {
            id: 'permission',
            label: t('tasks.permissionTab'),
            panel: latest ? (
              <section className="card" aria-label={t('tasks.grantTitle')}>
                <p className="muted small">{t('tasks.attempt', { n: latest.attempt_no })}</p>
                <GrantFacts grant={latest.grant} />
              </section>
            ) : (
              <p className="muted">{t('tasks.noAttempts')}</p>
            ),
          },
          {
            id: 'settings',
            label: t('tasks.settingsTab'),
            panel: (
              <section className="card" aria-label={t('tasks.settingsTitle')}>
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
              </section>
            ),
          },
        ]}
      />
    </>
  );
}

function GrantFacts({ grant }: { grant: Attempt['grant'] }) {
  const { t } = useTranslation();
  return (
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
  );
}

function AttemptCard({ attempt }: { attempt: Attempt }) {
  const { t } = useTranslation();
  const { outcome } = attempt;
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
    </article>
  );
}

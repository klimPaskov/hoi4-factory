import { useRef, useState, type FormEvent } from 'react';
import { Link, useNavigate, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import {
  createTask,
  getModelProfiles,
  listApiKeys,
  listTasks,
  type ApiKeyRecord,
  type ModelProfile,
  type Task,
  type TaskMode,
  type TaskState,
} from '../api/tasks';
import { Badge, Button, ErrorMessage, Field, Loading, NotAvailableYet, PageHeader, type Tone } from '../components/ui';
import { formatMicros, formatNumber } from '../format';
import { useAsync, useFocusOnError, usePageTitle } from '../hooks';

export function TasksPage() {
  const { id = '' } = useParams();
  return <TasksView key={id} projectId={id} />;
}

// The task list follows docs/design/mockups/52-tasks-c-grouped (provisional overnight choice):
// tasks grouped by what they need, "Needs you" first, finished ones folded away.

const NEEDS_YOU: TaskState[] = ['waiting_for_input', 'in_review', 'blocked'];
const FINISHED: TaskState[] = ['completed', 'cancelled'];

function stateTone(task: Task): Tone {
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

function TasksView({ projectId }: { projectId: string }) {
  const { t } = useTranslation();
  usePageTitle(t('tasks.title'));
  const state = useAsync((signal) => listTasks(projectId, signal));
  const [formOpen, setFormOpen] = useState(false);
  const hasTasks = state.status === 'ready' && state.data.length > 0;

  return (
    <>
      <PageHeader
        title={t('tasks.title')}
        back={{ to: `/projects/${encodeURIComponent(projectId)}`, label: t('tasks.back') }}
        actions={
          hasTasks && !formOpen ? (
            <Button variant="primary" icon="plus" onClick={() => setFormOpen(true)}>
              {t('tasks.newTitle')}
            </Button>
          ) : undefined
        }
      />
      {state.status === 'loading' ? <Loading label={t('tasks.loading')} /> : null}
      {state.status === 'error' ? (
        state.error instanceof ApiError && state.error.isUnavailable ? (
          <NotAvailableYet what={t('tasks.what')} />
        ) : (
          <ErrorMessage error={state.error} onRetry={state.reload} />
        )
      ) : null}
      {state.status === 'ready' && (!hasTasks || formOpen) ? (
        <NewTaskForm projectId={projectId} onCancel={hasTasks ? () => setFormOpen(false) : undefined} />
      ) : null}
      {state.status === 'ready' && hasTasks ? <TaskList projectId={projectId} tasks={state.data} /> : null}
    </>
  );
}

function TaskRow({ projectId, task }: { projectId: string; task: Task }) {
  const { t } = useTranslation();
  return (
    <li className="task-row">
      <Link className="task-objective" to={`/projects/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(task.id)}`}>
        <bdi dir="auto">{task.objective}</bdi>
      </Link>
      <span className="task-meta">
        <Badge tone={stateTone(task)} live={task.state === 'running' && !task.paused}>
          {task.paused ? t('tasks.paused') : t(`tasks.states.${task.state}`)}
        </Badge>
        <span className="muted small">{t(`tasks.modes.${task.mode}`)}</span>
        <span className="muted small task-spend">
          {t('tasks.spent', { amount: formatMicros(task.spent_micros), budget: formatMicros(task.budget_micros) })}
        </span>
      </span>
    </li>
  );
}

function TaskGroup({ id, title, projectId, tasks }: { id: string; title: string; projectId: string; tasks: Task[] }) {
  if (tasks.length === 0) return null;
  return (
    <section className="task-group" aria-labelledby={`tasks-${id}`}>
      <h2 id={`tasks-${id}`}>
        {title} <span className="count">{formatNumber(tasks.length)}</span>
      </h2>
      <ul className="task-rows">
        {tasks.map((task) => (
          <TaskRow key={task.id} projectId={projectId} task={task} />
        ))}
      </ul>
    </section>
  );
}

function TaskList({ projectId, tasks }: { projectId: string; tasks: Task[] }) {
  const { t } = useTranslation();
  const needsYou = tasks.filter((task) => NEEDS_YOU.includes(task.state));
  const finished = tasks.filter((task) => FINISHED.includes(task.state));
  const working = tasks.filter((task) => !NEEDS_YOU.includes(task.state) && !FINISHED.includes(task.state));
  return (
    <>
      <TaskGroup id="needs-you" title={t('tasks.groups.needsYou')} projectId={projectId} tasks={needsYou} />
      <TaskGroup id="working" title={t('tasks.groups.working')} projectId={projectId} tasks={working} />
      {finished.length > 0 ? (
        <details className="later task-finished">
          <summary>
            <h2>
              {t('tasks.groups.finished')} <span className="count">{formatNumber(finished.length)}</span>
            </h2>
          </summary>
          <ul className="task-rows">
            {finished.map((task) => (
              <TaskRow key={task.id} projectId={projectId} task={task} />
            ))}
          </ul>
        </details>
      ) : null}
    </>
  );
}

const ALLOWANCE = 'allowance';

function NewTaskForm({ projectId, onCancel }: { projectId: string; onCancel?: () => void }) {
  const { t } = useTranslation();
  const profiles = useAsync((signal) => getModelProfiles(signal));
  // The saved keys are optional: a server without them still offers the free allowance.
  const keys = useAsync(async (signal): Promise<ApiKeyRecord[]> => {
    try {
      const list = await listApiKeys(signal);
      return list.available ? list.keys : [];
    } catch (error) {
      if (error instanceof ApiError) return [];
      throw error;
    }
  });

  return (
    <section className="card new-task" aria-labelledby="new-task-title">
      <h2 id="new-task-title">{t('tasks.newTitle')}</h2>
      {onCancel ? null : <p className="muted">{t('tasks.emptyBody')}</p>}
      {profiles.status === 'loading' || keys.status === 'loading' ? <Loading /> : null}
      {profiles.status === 'error' ? (
        profiles.error instanceof ApiError && profiles.error.isUnavailable ? (
          <NotAvailableYet what={t('tasks.what')} />
        ) : (
          <ErrorMessage error={profiles.error} onRetry={profiles.reload} />
        )
      ) : null}
      {profiles.status === 'ready' && keys.status === 'ready' ? (
        profiles.data.profiles.length === 0 ? (
          <p className="muted">{t('tasks.modelsUnavailable')}</p>
        ) : (
          <Form projectId={projectId} profiles={profiles.data.profiles} keys={keys.data} onCancel={onCancel} />
        )
      ) : null}
    </section>
  );
}

function Form({
  projectId,
  profiles,
  keys,
  onCancel,
}: {
  projectId: string;
  profiles: ModelProfile[];
  keys: ApiKeyRecord[];
  onCancel?: () => void;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const initial = profiles.find((p) => p.default) ?? profiles[0];
  const [objective, setObjective] = useState('');
  const [mode, setMode] = useState<TaskMode>('read_only');
  const [paths, setPaths] = useState('');
  const [budget, setBudget] = useState('0.20');
  const [profileId, setProfileId] = useState(initial?.id ?? '');
  const [funding, setFunding] = useState(ALLOWANCE);
  const [problems, setProblems] = useState<{ objective?: string; paths?: string; budget?: string }>({});
  const [failure, setFailure] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  const profile = profiles.find((p) => p.id === profileId) ?? initial;
  const usableKeys = keys.filter((k) => k.provider === profile?.provider);
  const allowanceOk = profile?.operator_allowance ?? true;
  // Fall back to what is on offer when the model changes and the chosen funding no longer fits.
  const fundingChoice =
    funding === ALLOWANCE
      ? allowanceOk || usableKeys.length === 0
        ? ALLOWANCE
        : (usableKeys[0]?.id ?? ALLOWANCE)
      : usableKeys.some((k) => k.id === funding)
        ? funding
        : allowanceOk
          ? ALLOWANCE
          : (usableKeys[0]?.id ?? ALLOWANCE);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setFailure(null);
    const found: typeof problems = {};
    const dollars = Number(budget.replace(',', '.'));
    const allowed = paths
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean);
    if (!objective.trim()) found.objective = t('tasks.needObjective');
    if (!Number.isFinite(dollars) || dollars <= 0) found.budget = t('tasks.needBudget');
    if (mode === 'edit' && allowed.length === 0) found.paths = t('tasks.needPaths');
    setProblems(found);
    if (Object.keys(found).length > 0 || !profile) {
      setVersion((v) => v + 1);
      return;
    }
    setBusy(true);
    try {
      const task = await createTask(projectId, {
        objective: objective.trim(),
        mode,
        allowed_paths: mode === 'edit' ? allowed : [],
        budget_usd: dollars,
        model: { profile_id: profile.id, key_id: fundingChoice === ALLOWANCE ? null : fundingChoice },
      });
      void navigate(`/projects/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(task.id)}`);
    } catch (error) {
      setFailure(error);
      setVersion((v) => v + 1);
      setBusy(false);
    }
  }

  return (
    <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate>
      {failure ? <ErrorMessage error={failure} focusable /> : null}
      <Field
        label={t('tasks.objective')}
        hint={t('tasks.objectiveHint')}
        value={objective}
        onChange={setObjective}
        error={problems.objective}
        multiline
        rows={4}
      />
      <fieldset className="plain-fieldset">
        <legend>{t('tasks.mode')}</legend>
        <label className="choice">
          <input type="radio" name="task-mode" checked={mode === 'read_only'} onChange={() => setMode('read_only')} />
          <span>{t('tasks.modeReadOnly')}</span>
        </label>
        <label className="choice">
          <input type="radio" name="task-mode" checked={mode === 'edit'} onChange={() => setMode('edit')} />
          <span>{t('tasks.modeEdit')}</span>
        </label>
      </fieldset>
      {mode === 'edit' ? (
        <>
          <p className="muted">{t('tasks.editNote')}</p>
          <Field
            label={t('tasks.allowedPaths')}
            hint={t('tasks.allowedPathsHint')}
            value={paths}
            onChange={setPaths}
            error={problems.paths}
            multiline
            dir="ltr"
            spellCheck={false}
          />
        </>
      ) : null}
      <Field
        label={t('tasks.budget')}
        hint={t('tasks.budgetHint')}
        value={budget}
        onChange={setBudget}
        error={problems.budget}
        dir="ltr"
      />
      <div className="field">
        <label htmlFor="task-model">{t('tasks.model')}</label>
        <select id="task-model" value={profile?.id ?? ''} onChange={(event) => setProfileId(event.target.value)}>
          {profiles.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <label htmlFor="task-funding">{t('tasks.funding')}</label>
        <select id="task-funding" value={fundingChoice} onChange={(event) => setFunding(event.target.value)}>
          {allowanceOk ? <option value={ALLOWANCE}>{t('tasks.fundingAllowance')}</option> : null}
          {usableKeys.map((k) => (
            <option key={k.id} value={k.id}>
              {t('tasks.fundingKey', { label: k.label })}
            </option>
          ))}
        </select>
      </div>
      <div className="button-row">
        <button type="submit" className="button primary" disabled={busy}>
          {busy ? t('tasks.creating') : t('tasks.create')}
        </button>
        {onCancel ? (
          <button type="button" className="button secondary" onClick={onCancel}>
            {t('common.cancel')}
          </button>
        ) : null}
      </div>
    </form>
  );
}

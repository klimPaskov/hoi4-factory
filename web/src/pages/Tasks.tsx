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
} from '../api/tasks';
import { Badge, Button, ErrorMessage, Field, Loading, NotAvailableYet, PageHeader, type Tone } from '../components/ui';
import { formatMicros, formatNumber } from '../format';
import { useAsync, useFocusOnError, usePageTitle } from '../hooks';

export function TasksPage() {
  const { id = '' } = useParams();
  return <TasksView key={id} projectId={id} />;
}

// The task list follows docs/design/mockups/50-tasks-a-panel (the maintainer's choice B): one
// table of every task, and the new task form in a panel beside it.

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
      {state.status === 'ready' && !hasTasks ? <NewTaskForm projectId={projectId} /> : null}
      {state.status === 'ready' && hasTasks ? (
        <div className={formOpen ? 'tasks-layout with-panel' : 'tasks-layout'}>
          <TaskTable projectId={projectId} tasks={state.data} />
          {formOpen ? <NewTaskForm projectId={projectId} onCancel={() => setFormOpen(false)} /> : null}
        </div>
      ) : null}
    </>
  );
}

function TaskTable({ projectId, tasks }: { projectId: string; tasks: Task[] }) {
  const { t } = useTranslation();
  return (
    <div className="table-wrap">
      <table className="data-table task-table">
        <caption className="sr-only">{t('tasks.listLabel', { count: tasks.length, n: formatNumber(tasks.length) })}</caption>
        <thead>
          <tr>
            <th scope="col">{t('tasks.columns.objective')}</th>
            <th scope="col">{t('tasks.columns.mode')}</th>
            <th scope="col">{t('tasks.columns.state')}</th>
            <th scope="col">{t('tasks.columns.spend')}</th>
            <th scope="col">{t('tasks.columns.model')}</th>
          </tr>
        </thead>
        <tbody>
          {tasks.map((task) => (
            <tr key={task.id}>
              <th scope="row" className="task-objective">
                <Link to={`/projects/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(task.id)}`}>
                  <bdi dir="auto">{task.objective}</bdi>
                </Link>
              </th>
              <td className="cell-nowrap">{t(`tasks.modes.${task.mode}`)}</td>
              <td className="cell-nowrap">
                <Badge tone={stateTone(task)} live={task.state === 'running' && !task.paused}>
                  {task.paused ? t('tasks.paused') : t(`tasks.states.${task.state}`)}
                </Badge>
              </td>
              <td className="task-spend">
                <span className="sr-only">
                  {t('tasks.spent', { amount: formatMicros(task.spent_micros), budget: formatMicros(task.budget_micros) })}
                </span>
                <span aria-hidden="true">
                  {t('tasks.spentOf', { amount: formatMicros(task.spent_micros), budget: formatMicros(task.budget_micros) })}
                </span>
                <meter
                  className="meter-bar"
                  min={0}
                  max={task.budget_micros}
                  value={Math.min(task.spent_micros, task.budget_micros)}
                  aria-hidden="true"
                />
              </td>
              <td className="cell-nowrap">
                <bdi dir="ltr">{task.model.profile_id}</bdi>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
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
    <section className={onCancel ? 'card new-task task-panel' : 'card new-task'} aria-labelledby="new-task-title">
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

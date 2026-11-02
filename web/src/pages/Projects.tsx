import { useId, useRef, useState, type FormEvent, type RefObject } from 'react';
import { Link, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { createProject, listProjects } from '../api/accounts';
import type { Project } from '../api/types';
import { PendingDeletion, useProjectActions } from '../components/ProjectActions';
import { Badge, Button, ErrorMessage, Field, FlagAvatar, Icon, PageHeader, Skeleton } from '../components/ui';
import { describeError, fieldForError } from '../errors';
import { formatDate, formatNumber } from '../format';
import { useAsync, useFocusOnError, usePageTitle } from '../hooks';

// The project list follows the approved mockup docs/design/mockups/20-projects-a-table: a quiet
// table with a filter, and a single name field for a new user's empty list.

function useNewProject() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [name, setName] = useState('');
  const [error, setError] = useState<string | undefined>();
  const [failure, setFailure] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setFailure(null);
    if (!name.trim()) {
      setError(t('validation.nameRequired'));
      setVersion((v) => v + 1);
      return;
    }
    setError(undefined);
    setBusy(true);
    try {
      const project = await createProject(name.trim());
      // The next step is to bring a mod into the new project.
      navigate(`/projects/${project.id}`);
    } catch (e) {
      if (fieldForError(e, { name: /name/i })) setError(describeError(e).message);
      else setFailure(e);
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  return { name, setName, error, failure, busy, formRef, submit };
}

/** The new project form that opens under the page title. */
function NewProjectForm({ inputRef, onCancel }: { inputRef: RefObject<HTMLInputElement | null>; onCancel: () => void }) {
  const { t } = useTranslation();
  const { name, setName, error, failure, busy, formRef, submit } = useNewProject();
  return (
    <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate className="card new-project" aria-labelledby="new-project-title">
      <h2 id="new-project-title">{t('projects.newTitle')}</h2>
      {failure ? <ErrorMessage error={failure} focusable /> : null}
      <Field
        label={t('projects.nameLabel')}
        hint={t('projects.nameHint')}
        value={name}
        onChange={setName}
        error={error}
        inputRef={inputRef}
        maxLength={400}
      />
      <div className="button-row">
        <Button type="submit" variant="primary" disabled={busy}>
          {busy ? t('projects.creating') : t('projects.create')}
        </Button>
        <Button onClick={onCancel}>{t('common.cancel')}</Button>
      </div>
    </form>
  );
}

/** A new user's empty list: one field and one line of help, nothing else. */
function FirstProject() {
  const { t } = useTranslation();
  const { name, setName, error, failure, busy, formRef, submit } = useNewProject();
  const id = useId();
  return (
    <section className="first-project" aria-labelledby="first-project-title">
      <h2 id="first-project-title">{t('projects.emptyTitle')}</h2>
      <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate>
        {failure ? <ErrorMessage error={failure} focusable /> : null}
        <div className="first-project-row">
          <label htmlFor={id} className="sr-only">
            {t('projects.nameLabel')}
          </label>
          <input
            id={id}
            type="text"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={t('projects.emptyPlaceholder')}
            maxLength={400}
            dir="auto"
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? `${id}-error` : `${id}-help`}
          />
          <Button type="submit" variant="primary" disabled={busy}>
            {busy ? t('projects.creating') : t('projects.create')}
          </Button>
        </div>
        {error ? (
          <p id={`${id}-error`} className="field-error">
            {error}
          </p>
        ) : null}
        <p id={`${id}-help`} className="muted small">
          {t('projects.emptyBody')}
        </p>
      </form>
    </section>
  );
}

function ProjectRow({ project, onChanged }: { project: Project; onChanged: (project: Project) => void }) {
  const { t } = useTranslation();
  const actions = useProjectActions(project, onChanged);
  const columns = 'projects.columns';
  return (
    <>
      <tr>
        <th scope="row">
          <span className="project-name">
            <FlagAvatar name={project.name} />
            <Link to={`/projects/${project.id}`}>
              <bdi dir="auto">{project.name}</bdi>
            </Link>
          </span>
        </th>
        <td className="cell-nowrap" data-label={t(`${columns}.visibility`)}>
          <Badge tone={project.visibility === 'public' ? 'success' : 'neutral'}>{t(`project.visibility.${project.visibility}`)}</Badge>
        </td>
        <td className="cell-nowrap" data-label={t(`${columns}.role`)}>
          {project.role ? t(`project.role.${project.role}`) : null}
        </td>
        <td className="cell-nowrap" data-label={t(`${columns}.created`)}>
          <time dateTime={project.created_at}>{formatDate(project.created_at)}</time>
        </td>
        <td className="cell-actions">{actions.triggers}</td>
      </tr>
      {actions.panel ? (
        <tr className="row-panel">
          <td colSpan={5}>{actions.panel}</td>
        </tr>
      ) : null}
    </>
  );
}

export function ProjectsPage() {
  const { t } = useTranslation();
  usePageTitle(t('projects.title'));
  const state = useAsync((signal) => listProjects(true, signal));
  const nameRef = useRef<HTMLInputElement>(null);
  const [formOpen, setFormOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [pendingOpen, setPendingOpen] = useState(false);
  const filterId = useId();

  function update(changed: Project) {
    if (state.status !== 'ready') return;
    if (changed.deleted_at) setPendingOpen(true);
    state.replace(state.data.map((p) => (p.id === changed.id ? changed : p)));
  }

  function openForm() {
    setFormOpen(true);
    requestAnimationFrame(() => nameRef.current?.focus());
  }

  const active = state.status === 'ready' ? state.data.filter((p) => !p.deleted_at) : [];
  const pending = state.status === 'ready' ? state.data.filter((p) => p.deleted_at) : [];
  const needle = query.trim().toLocaleLowerCase();
  const shown = needle ? active.filter((p) => p.name.toLocaleLowerCase().includes(needle)) : active;

  return (
    <>
      <PageHeader
        title={t('projects.title')}
        subtitle={active.length > 0 ? t('projects.count', { count: active.length }) : undefined}
        actions={
          active.length > 0 && !formOpen ? (
            <Button variant="primary" icon="plus" onClick={openForm}>
              {t('projects.newTitle')}
            </Button>
          ) : undefined
        }
      />

      {formOpen ? <NewProjectForm inputRef={nameRef} onCancel={() => setFormOpen(false)} /> : null}

      {state.status === 'loading' ? (
        <div className="table-wrap table-loading">
          <Skeleton label={t('projects.loading')} rows={5} />
        </div>
      ) : null}
      {state.status === 'error' ? <ErrorMessage error={state.error} onRetry={state.reload} /> : null}

      {state.status === 'ready' && active.length === 0 ? <FirstProject /> : null}

      {active.length > 0 ? (
        <section aria-labelledby="your-projects">
          <h2 id="your-projects" className="sr-only">
            {t('projects.yourProjects')}
          </h2>
          <div className="filter">
            <label htmlFor={filterId} className="sr-only">
              {t('projects.filter')}
            </label>
            <Icon name="search" size="sm" />
            <input
              id={filterId}
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t('projects.filter')}
            />
          </div>
          {shown.length === 0 ? (
            <p className="muted">{t('projects.noMatch', { query: query.trim() })}</p>
          ) : (
            <div className="table-wrap">
              <table className="data-table project-table">
                <thead>
                  <tr>
                    <th scope="col">{t('projects.columns.name')}</th>
                    <th scope="col">{t('projects.columns.visibility')}</th>
                    <th scope="col">{t('projects.columns.role')}</th>
                    <th scope="col">{t('projects.columns.created')}</th>
                    <th scope="col">
                      <span className="sr-only">{t('projects.columns.actions')}</span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {shown.map((project) => (
                    <ProjectRow key={project.id} project={project} onChanged={update} />
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>
      ) : null}

      {pending.length > 0 ? (
        <details
          className="pending"
          open={pendingOpen}
          onToggle={(event) => setPendingOpen((event.currentTarget as HTMLDetailsElement).open)}
        >
          <summary>
            <h2 id="pending-projects">{t('projects.pendingTitle')}</h2>
            <span className="count">{formatNumber(pending.length)}</span>
          </summary>
          <p className="muted small">{t('projects.pendingBody')}</p>
          <ul className="pending-list">
            {pending.map((project) => (
              <li key={project.id}>
                <span className="project-name">
                  <FlagAvatar name={project.name} />
                  <bdi dir="auto" className="strong">
                    {project.name}
                  </bdi>
                </span>
                <PendingDeletion project={project} onChanged={update} />
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </>
  );
}

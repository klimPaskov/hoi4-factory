import { useRef, useState, type FormEvent, type RefObject } from 'react';
import { Link, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { createProject, listProjects } from '../api/accounts';
import type { Project } from '../api/types';
import { PendingDeletion, ProjectActions } from '../components/ProjectActions';
import { EmptyState, ErrorMessage, Field, Loading } from '../components/ui';
import { describeError, fieldForError } from '../errors';
import { formatDate, formatNumber } from '../format';
import { useAsync, useFocusOnError, usePageTitle } from '../hooks';

function NewProjectForm({ inputRef }: { inputRef: RefObject<HTMLInputElement | null> }) {
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

  return (
    <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate className="card" aria-labelledby="new-project-title">
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
      <button type="submit" className="button primary" disabled={busy}>
        {busy ? t('projects.creating') : t('projects.create')}
      </button>
    </form>
  );
}

export function ProjectsPage() {
  const { t } = useTranslation();
  usePageTitle(t('projects.title'));
  const state = useAsync((signal) => listProjects(true, signal));
  const nameRef = useRef<HTMLInputElement>(null);

  function update(changed: Project) {
    if (state.status !== 'ready') return;
    state.replace(state.data.map((p) => (p.id === changed.id ? changed : p)));
  }

  const active = state.status === 'ready' ? state.data.filter((p) => !p.deleted_at) : [];
  const pending = state.status === 'ready' ? state.data.filter((p) => p.deleted_at) : [];

  return (
    <>
      <h1>{t('projects.title')}</h1>
      <NewProjectForm inputRef={nameRef} />

      {state.status === 'loading' ? <Loading label={t('projects.loading')} /> : null}
      {state.status === 'error' ? <ErrorMessage error={state.error} onRetry={state.reload} /> : null}

      {state.status === 'ready' && active.length === 0 ? (
        <EmptyState
          title={t('projects.emptyTitle')}
          action={
            <button type="button" className="button primary" onClick={() => nameRef.current?.focus()}>
              {t('projects.emptyAction')}
            </button>
          }
        >
          {t('projects.emptyBody')}
        </EmptyState>
      ) : null}

      {active.length > 0 ? (
        <section aria-labelledby="your-projects">
          <h2 id="your-projects">
            {t('projects.yourProjects')}{' '}
            <span className="muted small">({formatNumber(active.length)})</span>
          </h2>
          <ul className="project-list">
            {active.map((project) => (
              <li key={project.id} className="card project-item">
                <div>
                  <h3>
                    <Link to={`/projects/${project.id}`}>
                      <bdi dir="auto">{project.name}</bdi>
                    </Link>
                  </h3>
                  <p className="small muted">
                    {t(`project.visibility.${project.visibility}`)} {'· '}
                    {t('projects.created', { date: formatDate(project.created_at) })}
                    {project.role && project.role !== 'owner' ? ` · ${t(`project.role.${project.role}`)}` : ''}
                  </p>
                </div>
                <ProjectActions project={project} onChanged={update} />
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {pending.length > 0 ? (
        <section aria-labelledby="pending-projects">
          <h2 id="pending-projects">{t('projects.pendingTitle')}</h2>
          <p className="muted small">{t('projects.pendingBody')}</p>
          <ul className="project-list">
            {pending.map((project) => (
              <li key={project.id} className="card project-item">
                <div>
                  <h3>
                    <bdi dir="auto">{project.name}</bdi>
                  </h3>
                </div>
                <PendingDeletion project={project} onChanged={update} />
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </>
  );
}

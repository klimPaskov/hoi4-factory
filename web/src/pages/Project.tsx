import { useState } from 'react';
import { Link, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { getProject } from '../api/accounts';
import type { SetupScan } from '../api/intake';
import type { Project } from '../api/types';
import { BringInMod } from '../components/BringInMod';
import { PendingDeletion, ProjectActions } from '../components/ProjectActions';
import { SetupReview } from '../components/SetupReview';
import { Alert, ErrorMessage, Loading } from '../components/ui';
import { WorkspacePanel } from '../components/WorkspacePanel';
import { formatDate } from '../format';
import { useAsync, usePageTitle } from '../hooks';

export function ProjectPage() {
  const { id = '' } = useParams();
  // A different project is a fresh page: start from the loading state.
  return <ProjectView key={id} id={id} />;
}

function ProjectView({ id }: { id: string }) {
  const { t } = useTranslation();
  const state = useAsync((signal) => getProject(id, signal));
  usePageTitle(state.status === 'ready' ? state.data.name : t('project.titleFallback'));

  return (
    <>
      <p className="breadcrumb">
        <Link to="/">{t('project.back')}</Link>
      </p>
      {state.status === 'loading' ? <Loading label={t('project.loading')} /> : null}
      {state.status === 'error' ? (
        <>
          <h1>{t('project.errorTitle')}</h1>
          <ErrorMessage error={state.error} onRetry={state.reload} />
        </>
      ) : null}
      {state.status === 'ready' ? <ProjectBody project={state.data} onChanged={state.replace} /> : null}
    </>
  );
}

function ProjectBody({ project, onChanged }: { project: Project; onChanged: (project: Project) => void }) {
  const { t } = useTranslation();
  const [workspaceVersion, setWorkspaceVersion] = useState(0);
  // The wizard restores its own step from the server. A scan made by "Create new" is handed to
  // it, and a new `wizardKey` makes it start from that scan (or from the server's status).
  const [scan, setScan] = useState<SetupScan | null>(null);
  const [wizardKey, setWizardKey] = useState(0);
  const pending = project.deleted_at !== null;
  const canWork = project.role === 'owner' || project.role === 'maintainer';

  return (
    <>
      <header className="project-header">
        <div>
          <h1>
            <bdi dir="auto">{project.name}</bdi>
          </h1>
          <p className="muted small">
            {t(`project.visibility.${project.visibility}`)} {'· '}
            {t('projects.created', { date: formatDate(project.created_at) })}
          </p>
        </div>
        <ProjectActions project={project} onChanged={onChanged} />
      </header>

      {pending ? (
        <Alert kind="warning">
          <p>{t('project.pendingNotice')}</p>
          <PendingDeletion project={project} onChanged={onChanged} />
        </Alert>
      ) : (
        <>
          <WorkspacePanel key={workspaceVersion} projectId={project.id} />
          <section className="card" aria-labelledby="project-tasks-title">
            <h2 id="project-tasks-title">{t('project.tasksTitle')}</h2>
            <p>{t('project.tasksBody')}</p>
            <Link to={`/projects/${encodeURIComponent(project.id)}/tasks`}>{t('project.tasksLink')}</Link>
          </section>
          {canWork ? (
            <>
              <BringInMod
                projectId={project.id}
                onBroughtIn={() => {
                  setScan(null);
                  setWizardKey((k) => k + 1);
                  setWorkspaceVersion((v) => v + 1);
                }}
                onScanned={(next) => {
                  setScan(next);
                  setWizardKey((k) => k + 1);
                  setWorkspaceVersion((v) => v + 1);
                }}
              />
              <SetupReview
                key={wizardKey}
                projectId={project.id}
                initialScan={scan}
                onChanged={() => setWorkspaceVersion((v) => v + 1)}
              />
            </>
          ) : (
            <Alert kind="info">{t('project.readOnly')}</Alert>
          )}
        </>
      )}
    </>
  );
}

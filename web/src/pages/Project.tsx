import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { getProject } from '../api/accounts';
import { ApiError, isAbort } from '../api/client';
import { getWorkspace, type SetupScan, type WorkspaceSummary } from '../api/intake';
import type { Project } from '../api/types';
import { BringInMod } from '../components/BringInMod';
import { PendingDeletion, useProjectActions } from '../components/ProjectActions';
import { SetupReview } from '../components/SetupReview';
import { Alert, Badge, ErrorMessage, Icon, Loading, NotAvailableYet, StepIndicator, type StepState } from '../components/ui';
import { WorkspaceFacts } from '../components/WorkspacePanel';
import { formatDate } from '../format';
import { useAsync, usePageTitle } from '../hooks';

// The project page follows the approved mockup docs/design/mockups/32-project-c-guided: a
// three-step line and only the current step as a card. Finished steps stay reachable in
// collapsed sections under it.

type Stage = 1 | 2 | 3;

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

function ProjectHeader({ project, onChanged }: { project: Project; onChanged: (project: Project) => void }) {
  const { t } = useTranslation();
  const actions = useProjectActions(project, onChanged);
  return (
    <>
      <header className="page-header project-head">
        <div>
          <h1 className="project-title">
            <bdi dir="auto">{project.name}</bdi>
            <Badge tone={project.visibility === 'public' ? 'success' : 'neutral'}>{t(`project.visibility.${project.visibility}`)}</Badge>
          </h1>
          <p className="page-subtitle">{t('projects.created', { date: formatDate(project.created_at) })}</p>
        </div>
        <div className="page-actions">
          {project.deleted_at ? null : (
            <Link className="button ghost small-button" to={`/projects/${encodeURIComponent(project.id)}/tasks`}>
              {t('project.tasksLink')}
            </Link>
          )}
          {actions.triggers}
        </div>
      </header>
      {actions.panel ? <div className="project-panel">{actions.panel}</div> : null}
    </>
  );
}

function ProjectBody({ project, onChanged }: { project: Project; onChanged: (project: Project) => void }) {
  const { t } = useTranslation();
  const pending = project.deleted_at !== null;
  const canWork = project.role === 'owner' || project.role === 'maintainer';

  return (
    <>
      <ProjectHeader project={project} onChanged={onChanged} />
      {pending ? (
        <Alert kind="warning">
          <p>{t('project.pendingNotice')}</p>
          <PendingDeletion project={project} onChanged={onChanged} />
        </Alert>
      ) : canWork ? (
        <GuidedSteps projectId={project.id} />
      ) : (
        <ReadOnly projectId={project.id} />
      )}
    </>
  );
}

/** Someone who may look but not change sees what is there and why they cannot act. */
function ReadOnly({ projectId }: { projectId: string }) {
  const { t } = useTranslation();
  const workspace = useAsync((signal) => getWorkspace(projectId, signal));
  return (
    <>
      <Alert kind="info">{t('project.readOnly')}</Alert>
      {workspace.status === 'ready' && workspace.data.has_source ? (
        <section className="card" aria-labelledby="source-title">
          <h2 id="source-title">{t('project.sourceTitle')}</h2>
          <WorkspaceFacts summary={workspace.data} />
        </section>
      ) : null}
    </>
  );
}

type WorkspaceLoad =
  | { status: 'loading' }
  | { status: 'ready'; data: WorkspaceSummary }
  | { status: 'error'; error: unknown };

function GuidedSteps({ projectId }: { projectId: string }) {
  const { t } = useTranslation();
  // The wizard restores its own step from the server. A scan made by "Create new" is handed to
  // it, and a new `wizardKey` makes it start from that scan (or from the server's status).
  const [scan, setScan] = useState<SetupScan | null>(null);
  const [wizardKey, setWizardKey] = useState(0);
  // A refresh after a Setup step keeps the last answer on screen, so the wizard is not remounted
  // and keeps what it is showing (for example "Done. The plan was applied.").
  const [workspace, setWorkspace] = useState<WorkspaceLoad>({ status: 'loading' });
  const [version, setVersion] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    getWorkspace(projectId, controller.signal).then(
      (data) => setWorkspace({ status: 'ready', data }),
      (error: unknown) => {
        if (controller.signal.aborted || isAbort(error)) return;
        setWorkspace((previous) => (previous.status === 'ready' ? previous : { status: 'error', error }));
      },
    );
    return () => controller.abort();
  }, [projectId, version]);
  const refresh = () => setVersion((v) => v + 1);

  if (workspace.status === 'loading') return <Loading label={t('workspace.loading')} />;
  return (
    <WorkspaceStages
      projectId={projectId}
      workspace={workspace}
      onRetry={refresh}
      scan={scan}
      wizardKey={wizardKey}
      onBroughtIn={() => {
        setScan(null);
        setWizardKey((k) => k + 1);
        refresh();
      }}
      onScanned={(next) => {
        setScan(next);
        setWizardKey((k) => k + 1);
        refresh();
      }}
      onSetupChanged={refresh}
    />
  );
}

function WorkspaceStages({
  projectId,
  workspace,
  onRetry,
  scan,
  wizardKey,
  onBroughtIn,
  onScanned,
  onSetupChanged,
}: {
  projectId: string;
  workspace: Exclude<WorkspaceLoad, { status: 'loading' }>;
  onRetry: () => void;
  scan: SetupScan | null;
  wizardKey: number;
  onBroughtIn: () => void;
  onScanned: (scan: SetupScan) => void;
  onSetupChanged: () => void;
}) {
  const { t } = useTranslation();
  const unavailable = workspace.status === 'error' && workspace.error instanceof ApiError && workspace.error.isUnavailable;
  const summary = workspace.status === 'ready' ? workspace.data : null;
  const hasSource = Boolean(summary?.has_source) || scan !== null;
  const stage: Stage = !hasSource ? 1 : summary?.setup_state === 'applied' ? 3 : 2;
  const stepState = (step: Stage): StepState => (step < stage ? 'done' : step === stage ? 'current' : 'upcoming');

  const bringIn = <BringInMod projectId={projectId} onBroughtIn={onBroughtIn} onScanned={onScanned} />;
  const wizard = <SetupReview key={wizardKey} projectId={projectId} initialScan={scan} onChanged={onSetupChanged} />;

  return (
    <>
      <StepIndicator
        label={t('project.steps.label')}
        steps={[
          { label: t('project.steps.bring'), state: stepState(1) },
          { label: t('project.steps.setup'), state: stepState(2) },
          { label: t('project.steps.tasks'), state: stepState(3) },
        ]}
      />
      {unavailable ? <NotAvailableYet what={t('workspace.what')} /> : null}
      {workspace.status === 'error' && !unavailable ? <ErrorMessage error={workspace.error} onRetry={onRetry} /> : null}

      {stage === 1 ? bringIn : null}
      {stage === 2 ? wizard : null}
      {stage === 3 ? (
        <section className="card next-step" aria-labelledby="next-step-title">
          <h2 id="next-step-title">{t('project.nextTitle')}</h2>
          <p className="muted">{t('project.tasksBody')}</p>
          <Link className="button primary" to={`/projects/${encodeURIComponent(projectId)}/tasks`}>
            <Icon name="agent" size="sm" />
            {t('project.nextButton')}
          </Link>
        </section>
      ) : null}

      {stage > 1 ? (
        <details className="later">
          <summary>
            <h2>{t('project.sourceTitle')}</h2>
          </summary>
          {summary?.has_source ? <WorkspaceFacts summary={summary} /> : null}
          {bringIn}
        </details>
      ) : null}
      {stage === 3 ? (
        <details className="later">
          <summary>
            <h2>{t('setup.title')}</h2>
          </summary>
          {wizard}
        </details>
      ) : null}
    </>
  );
}

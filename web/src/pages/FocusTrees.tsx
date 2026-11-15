// The way into the focus editor from a task (D-66). It asks the server which focus trees the mod
// has. A mod with one tree goes straight to it. A mod with several gets one link per tree.
import { Link, Navigate, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import { listTrees } from '../api/focus';
import { EmptyState, ErrorMessage, Loading, NotAvailableYet } from '../components/ui';
import { useAsync, usePageTitle } from '../hooks';
import '../focus/focus.css';

export function FocusTreesPage() {
  const { id = '', taskId = '' } = useParams();
  return <Trees key={`${id}/${taskId}`} projectId={id} taskId={taskId} />;
}

function Trees({ projectId, taskId }: { projectId: string; taskId: string }) {
  const { t } = useTranslation();
  usePageTitle(t('focus.title'));
  const state = useAsync((signal) => listTrees(projectId, taskId, signal));
  const task = `/projects/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(taskId)}`;
  const only = state.status === 'ready' && state.data.length === 1 ? state.data[0] : undefined;
  if (only) return <Navigate to={`${task}/focus/${encodeURIComponent(only.treeId)}`} replace />;
  return (
    <>
      <p className="breadcrumb">
        <Link to={task}>{t('focus.backToTask')}</Link>
      </p>
      <h1>{t('focus.title')}</h1>
      {state.status === 'loading' ? <Loading label={t('focus.trees.loading')} /> : null}
      {state.status === 'error' ? (
        state.error instanceof ApiError && state.error.isUnavailable ? (
          <NotAvailableYet what={t('focus.title')} />
        ) : (
          <ErrorMessage error={state.error} onRetry={state.reload} />
        )
      ) : null}
      {state.status === 'ready' && state.data.length === 0 ? <EmptyState title={t('focus.trees.noneTitle')}>{t('focus.trees.noneBody')}</EmptyState> : null}
      {state.status === 'ready' && state.data.length > 1 ? (
        <>
          <p>{t('focus.trees.choose')}</p>
          <ul className="fx-trees">
            {state.data.map((tree) => (
              <li key={tree.treeId}>
                <Link className="button secondary" to={`${task}/focus/${encodeURIComponent(tree.treeId)}`}>
                  <bdi dir="ltr">{tree.treeId}</bdi>
                </Link>
                <span className="muted small">{t('focus.trees.size', { count: tree.focusCount })}</span>
              </li>
            ))}
          </ul>
        </>
      ) : null}
    </>
  );
}

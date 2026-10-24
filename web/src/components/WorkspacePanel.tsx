import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import { getWorkspace } from '../api/intake';
import { formatNumber } from '../format';
import { useAsync } from '../hooks';
import { EmptyState, ErrorMessage, Loading, NotAvailableYet } from './ui';

/** What is in the project's server workspace right now. */
export function WorkspacePanel({ projectId }: { projectId: string }) {
  const { t } = useTranslation();
  const state = useAsync((signal) => getWorkspace(projectId, signal));

  return (
    <section className="card" aria-labelledby="workspace-title">
      <h2 id="workspace-title">{t('workspace.title')}</h2>
      {state.status === 'loading' ? <Loading label={t('workspace.loading')} /> : null}
      {state.status === 'error' ? (
        state.error instanceof ApiError && state.error.isUnavailable ? (
          <NotAvailableYet what={t('workspace.what')} />
        ) : (
          <ErrorMessage error={state.error} onRetry={state.reload} />
        )
      ) : null}
      {state.status === 'ready' && !state.data.has_source ? (
        <EmptyState title={t('workspace.emptyTitle')}>{t('workspace.emptyBody')}</EmptyState>
      ) : null}
      {state.status === 'ready' && state.data.has_source ? (
        <dl className="facts">
          {state.data.origin ? (
            <>
              <dt>{t('workspace.origin')}</dt>
              <dd>{t(`workspace.origins.${state.data.origin}`)}</dd>
            </>
          ) : null}
          {typeof state.data.file_count === 'number' ? (
            <>
              <dt>{t('workspace.files')}</dt>
              <dd>{formatNumber(state.data.file_count)}</dd>
            </>
          ) : null}
          {state.data.ref ? (
            <>
              <dt>{t('workspace.ref')}</dt>
              <dd dir="auto">{state.data.ref}</dd>
            </>
          ) : null}
          {state.data.revision ? (
            <>
              <dt>{t('workspace.revision')}</dt>
              <dd dir="ltr">{state.data.revision}</dd>
            </>
          ) : null}
          {state.data.setup_state ? (
            <>
              <dt>{t('workspace.setupState')}</dt>
              <dd>
                {t(`workspace.states.${state.data.setup_state}`, {
                  defaultValue: state.data.setup_state,
                })}
              </dd>
            </>
          ) : null}
        </dl>
      ) : null}
    </section>
  );
}

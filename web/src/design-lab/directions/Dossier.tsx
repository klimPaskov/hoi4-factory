import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import type { Project } from '../../api/types';
import { Field } from '../../components/ui';
import { formatDate } from '../../format';
import { SAMPLE_ACCOUNT } from '../fixtures';
import { DossierMark } from '../marks';
import {
  ConfirmDeleteText,
  ThemeControl,
  useGameReferenceText,
  useHere,
  useNewProject,
  useRetry,
  useRowActions,
  useSampleProjects,
  type DirectionProps,
  type SampleProjects,
} from '../shared';
import './dossier.css';

function Folder({ project, model }: { project: Project; model: SampleProjects }) {
  const { t } = useTranslation();
  const here = useHere();
  const row = useRowActions(project, model);
  return (
    <li className="ds-folder">
      <div className="ds-folder-top">
        <span className={`ds-stamp ds-stamp-${project.visibility}`}>
          {t(`project.visibility.${project.visibility}`)}
        </span>
        {project.role && project.role !== 'owner' ? (
          <span className="ds-role">{t(`project.role.${project.role}`)}</span>
        ) : null}
      </div>
      <h3 className="ds-folder-title">
        <Link to={here}>
          <bdi dir="auto">{project.name}</bdi>
        </Link>
      </h3>
      <p className="ds-type ds-folder-meta">
        <time dateTime={project.created_at}>{t('projects.created', { date: formatDate(project.created_at) })}</time>
      </p>
      {row.mode === 'idle' && (row.canRename || row.canDelete) ? (
        <p className="ds-folder-actions">
          {row.canRename ? (
            <button type="button" className="ds-text-button" onClick={() => row.open('rename')}>
              {t('project.rename')}
            </button>
          ) : null}
          {row.canDelete ? (
            <button type="button" className="ds-text-button ds-text-danger" onClick={() => row.open('delete')}>
              {t('project.delete')}
            </button>
          ) : null}
        </p>
      ) : null}
      {row.mode === 'rename' ? (
        <form className="ds-inline" onSubmit={row.submitRename} noValidate>
          <Field label={t('project.newName')} value={row.name} onChange={row.setName} error={row.error} maxLength={400} />
          <div className="ds-actions">
            <button type="submit" className="ds-btn ds-btn-primary">
              {t('common.save')}
            </button>
            <button type="button" className="ds-btn" onClick={row.close}>
              {t('common.cancel')}
            </button>
          </div>
        </form>
      ) : null}
      {row.mode === 'delete' ? (
        <div className="ds-inline ds-confirm" role="group" aria-label={t('project.confirmDeleteLabel')}>
          <p>
            <ConfirmDeleteText name={project.name} nameClass="ds-strong" />
          </p>
          <div className="ds-actions">
            <button type="button" className="ds-btn ds-btn-danger" onClick={row.confirmDelete}>
              {t('project.confirmDeleteButton')}
            </button>
            <button type="button" className="ds-btn" onClick={row.close}>
              {t('common.cancel')}
            </button>
          </div>
        </div>
      ) : null}
    </li>
  );
}

export function DossierDirection({ state, unverified }: DirectionProps) {
  const { t } = useTranslation();
  const here = useHere();
  const retry = useRetry();
  const model = useSampleProjects(state);
  const form = useNewProject(model);
  const gameReference = useGameReferenceText();
  const ready = state !== 'loading' && state !== 'error';

  return (
    <div className="dir-dossier">
      <header className="ds-band">
        <div className="ds-wrap ds-band-row">
          <Link to={here} className="ds-brand">
            <DossierMark />
            <span>{t('app.name')}</span>
          </Link>
          <div className="ds-band-tools">
            <p className="ds-type ds-gameref">
              <span className="ds-gameref-label">{t('gameReference.label')}</span> {gameReference}
            </p>
            <ThemeControl className="ds-theme" />
            <button type="button" className="ds-btn ds-btn-band">
              {t('common.signOut')}
            </button>
          </div>
        </div>
        <nav className="ds-wrap" aria-label={t('layout.primaryNav')}>
          <ul className="ds-tabs">
            <li>
              <Link to={here} aria-current="page">
                {t('nav.projects')}
              </Link>
            </li>
            <li>
              <Link to={here}>{t('nav.account')}</Link>
            </li>
          </ul>
        </nav>
      </header>

      {unverified ? (
        <div className="ds-wrap">
          <div className="ds-telegram" role="region" aria-label={t('verify.bannerLabel')}>
            <p className="ds-type">
              {t('verify.bannerText')} <bdi className="ds-strong">{SAMPLE_ACCOUNT.email}</bdi>
            </p>
            <button type="button" className="ds-btn">
              {t('verify.resend')}
            </button>
          </div>
        </div>
      ) : null}

      <main className="ds-wrap ds-main">
        <div className="ds-title">
          <p className="ds-type ds-kicker">
            {t('designLab.sample.dossierKicker')}
            {ready && model.active.length > 0
              ? ` · ${t('designLab.sample.projectCount', { count: model.active.length })}`
              : null}
          </p>
          <h1>{t('projects.title')}</h1>
        </div>

        <div className="ds-columns">
          <form className="ds-sheet" onSubmit={form.submit} noValidate aria-labelledby="ds-new-title">
            <h2 id="ds-new-title">{t('projects.newTitle')}</h2>
            <p className="ds-sheet-help">{t('designLab.sample.requisition')}</p>
            <Field
              label={t('projects.nameLabel')}
              hint={t('projects.nameHint')}
              value={form.name}
              onChange={form.setName}
              error={form.error}
              inputRef={form.inputRef}
              maxLength={400}
            />
            <button type="submit" className="ds-btn ds-btn-primary ds-btn-wide">
              {t('projects.create')}
            </button>
          </form>

          <div className="ds-list">
            {state === 'loading' ? (
              <p className="ds-type ds-loading" role="status">
                <span className="ds-loading-mark" aria-hidden="true" />
                {t('projects.loading')}
              </p>
            ) : null}

            {state === 'error' ? (
              <div className="ds-notice ds-notice-error" role="alert">
                <p>{t('errors.class.safe_same_inputs')}</p>
                <button type="button" className="ds-btn" onClick={retry}>
                  {t('common.tryAgain')}
                </button>
              </div>
            ) : null}

            {ready && model.active.length === 0 ? (
              <div className="ds-empty">
                <h2>{t('projects.emptyTitle')}</h2>
                <p>{t('projects.emptyBody')}</p>
                <button type="button" className="ds-btn ds-btn-primary" onClick={() => form.inputRef.current?.focus()}>
                  {t('projects.emptyAction')}
                </button>
              </div>
            ) : null}

            {ready && model.active.length > 0 ? (
              <section aria-labelledby="ds-list-title">
                <h2 id="ds-list-title" className="sr-only">
                  {t('projects.yourProjects')}
                </h2>
                <ul className="ds-folders">
                  {model.active.map((project) => (
                    <Folder key={project.id} project={project} model={model} />
                  ))}
                </ul>
              </section>
            ) : null}

            {ready && model.pending.length > 0 ? (
              <section className="ds-pending" aria-labelledby="ds-pending-title">
                <h2 id="ds-pending-title">{t('projects.pendingTitle')}</h2>
                <p className="ds-muted">{t('projects.pendingBody')}</p>
                <ul className="ds-pending-list">
                  {model.pending.map((project) => (
                    <li key={project.id}>
                      <div>
                        <bdi dir="auto" className="ds-strong ds-struck">
                          {project.name}
                        </bdi>
                        <p className="ds-type ds-muted">
                          {project.delete_after ? t('project.deletesOn', { date: formatDate(project.delete_after) }) : null}
                        </p>
                      </div>
                      <button type="button" className="ds-btn" onClick={() => model.restore(project.id)}>
                        {t('project.restore')}
                      </button>
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}
          </div>
        </div>
      </main>
      <footer className="ds-wrap ds-footer">{t('layout.footer')}</footer>
    </div>
  );
}

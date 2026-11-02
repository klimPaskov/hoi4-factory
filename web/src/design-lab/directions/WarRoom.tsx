import { useId } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import type { Project } from '../../api/types';
import { Field } from '../../components/ui';
import { formatDate, formatNumber } from '../../format';
import { SAMPLE_ACCOUNT } from '../fixtures';
import { WarRoomMark } from '../marks';
import {
  ConfirmDeleteText,
  ThemeControl,
  useFilter,
  useGameReferenceText,
  useHere,
  useNewProject,
  useRetry,
  useRowActions,
  useSampleProjects,
  type DirectionProps,
  type SampleProjects,
} from '../shared';
import './warroom.css';

function Tile({ project, model }: { project: Project; model: SampleProjects }) {
  const { t } = useTranslation();
  const here = useHere();
  const row = useRowActions(project, model);
  return (
    <li className={`wr-tile wr-tile-${project.visibility}`}>
      <div className="wr-tags">
        <span className={`wr-tag wr-tag-${project.visibility}`}>{t(`project.visibility.${project.visibility}`)}</span>
        {project.role && project.role !== 'owner' ? (
          <span className="wr-tag">{t(`project.role.${project.role}`)}</span>
        ) : null}
      </div>
      <h3 className="wr-tile-title">
        <Link to={here}>
          <bdi dir="auto">{project.name}</bdi>
        </Link>
      </h3>
      <p className="wr-mono wr-tile-meta">
        <time dateTime={project.created_at}>{t('projects.created', { date: formatDate(project.created_at) })}</time>
      </p>
      {row.mode === 'idle' && (row.canRename || row.canDelete) ? (
        <div className="wr-tile-actions">
          {row.canRename ? (
            <button type="button" className="wr-btn wr-btn-small" onClick={() => row.open('rename')}>
              {t('project.rename')}
            </button>
          ) : null}
          {row.canDelete ? (
            <button type="button" className="wr-btn wr-btn-small wr-btn-danger-outline" onClick={() => row.open('delete')}>
              {t('project.delete')}
            </button>
          ) : null}
        </div>
      ) : null}
      {row.mode === 'rename' ? (
        <form className="wr-inline" onSubmit={row.submitRename} noValidate>
          <Field label={t('project.newName')} value={row.name} onChange={row.setName} error={row.error} maxLength={400} />
          <div className="wr-actions">
            <button type="submit" className="wr-btn wr-btn-primary wr-btn-small">
              {t('common.save')}
            </button>
            <button type="button" className="wr-btn wr-btn-small" onClick={row.close}>
              {t('common.cancel')}
            </button>
          </div>
        </form>
      ) : null}
      {row.mode === 'delete' ? (
        <div className="wr-inline" role="group" aria-label={t('project.confirmDeleteLabel')}>
          <p>
            <ConfirmDeleteText name={project.name} nameClass="wr-strong" />
          </p>
          <div className="wr-actions">
            <button type="button" className="wr-btn wr-btn-danger wr-btn-small" onClick={row.confirmDelete}>
              {t('project.confirmDeleteButton')}
            </button>
            <button type="button" className="wr-btn wr-btn-small" onClick={row.close}>
              {t('common.cancel')}
            </button>
          </div>
        </div>
      ) : null}
    </li>
  );
}

export function WarRoomDirection({ state, unverified }: DirectionProps) {
  const { t } = useTranslation();
  const here = useHere();
  const retry = useRetry();
  const model = useSampleProjects(state);
  const form = useNewProject(model);
  const filter = useFilter(model.active);
  const gameReference = useGameReferenceText();
  const filterId = useId();
  const ready = state !== 'loading' && state !== 'error';
  const publicCount = model.active.filter((p) => p.visibility === 'public').length;

  return (
    <div className="dir-warroom">
      <header className="wr-bar">
        <div className="wr-wrap wr-bar-row">
          <Link to={here} className="wr-brand">
            <WarRoomMark />
            <span>{t('app.name')}</span>
          </Link>
          <nav aria-label={t('layout.primaryNav')}>
            <ul className="wr-nav">
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
          <div className="wr-bar-tools">
            <p className="wr-readout">
              <span className="wr-led" aria-hidden="true" />
              <span className="wr-readout-label">{t('gameReference.label')}</span>{' '}
              <span className="wr-mono">{gameReference}</span>
            </p>
            <ThemeControl className="wr-theme" />
            <button type="button" className="wr-btn wr-btn-bar">
              {t('common.signOut')}
            </button>
          </div>
        </div>
      </header>

      {unverified ? (
        <div className="wr-strip" role="region" aria-label={t('verify.bannerLabel')}>
          <div className="wr-wrap wr-strip-row">
            <p>
              {t('verify.bannerText')} <bdi className="wr-strong">{SAMPLE_ACCOUNT.email}</bdi>
            </p>
            <button type="button" className="wr-btn wr-btn-small">
              {t('verify.resend')}
            </button>
          </div>
        </div>
      ) : null}

      <main className="wr-wrap wr-main">
        <div className="wr-head">
          <h1>{t('projects.title')}</h1>
          {ready && (model.active.length > 0 || model.pending.length > 0) ? (
            <dl className="wr-stats" aria-label={t('designLab.sample.summary')}>
              <div>
                <dt>{t('projects.title')}</dt>
                <dd>{formatNumber(model.active.length)}</dd>
              </div>
              <div>
                <dt>{t('designLab.sample.public')}</dt>
                <dd>{formatNumber(publicCount)}</dd>
              </div>
              <div>
                <dt>{t('projects.pendingTitle')}</dt>
                <dd>{formatNumber(model.pending.length)}</dd>
              </div>
            </dl>
          ) : null}
        </div>

        <form className="wr-command" onSubmit={form.submit} noValidate aria-labelledby="wr-new-title">
          <h2 id="wr-new-title" className="wr-section-title">
            {t('projects.newTitle')}
          </h2>
          <div className="wr-command-row">
            <Field
              label={t('projects.nameLabel')}
              hint={t('projects.nameHint')}
              value={form.name}
              onChange={form.setName}
              error={form.error}
              inputRef={form.inputRef}
              maxLength={400}
            />
            <button type="submit" className="wr-btn wr-btn-primary">
              {t('projects.create')}
            </button>
          </div>
        </form>

        {state === 'loading' ? (
          <div className="wr-loading" role="status">
            <div className="wr-scan" aria-hidden="true" />
            <p className="wr-mono">{t('projects.loading')}</p>
            <ul className="wr-grid" aria-hidden="true">
              {[0, 1, 2].map((n) => (
                <li key={n} className="wr-tile wr-tile-ghost" />
              ))}
            </ul>
          </div>
        ) : null}

        {state === 'error' ? (
          <div className="wr-alert" role="alert">
            <p>{t('errors.class.safe_same_inputs')}</p>
            <button type="button" className="wr-btn wr-btn-small" onClick={retry}>
              {t('common.tryAgain')}
            </button>
          </div>
        ) : null}

        {ready && model.active.length === 0 ? (
          <div className="wr-empty">
            <WarRoomMark />
            <h2>{t('projects.emptyTitle')}</h2>
            <p>{t('projects.emptyBody')}</p>
            <button type="button" className="wr-btn wr-btn-primary" onClick={() => form.inputRef.current?.focus()}>
              {t('projects.emptyAction')}
            </button>
          </div>
        ) : null}

        {ready && model.active.length > 0 ? (
          <section aria-labelledby="wr-list-title">
            <div className="wr-list-head">
              <h2 id="wr-list-title" className="wr-section-title">
                {t('projects.yourProjects')}
              </h2>
              <div className="wr-filter">
                <label htmlFor={filterId}>{t('designLab.sample.filter')}</label>
                <input
                  id={filterId}
                  type="search"
                  value={filter.query}
                  onChange={(event) => filter.setQuery(event.target.value)}
                />
              </div>
            </div>
            {filter.shown.length === 0 ? (
              <p className="wr-mono wr-muted">{t('designLab.sample.noMatch', { query: filter.query })}</p>
            ) : (
              <ul className="wr-grid">
                {filter.shown.map((project) => (
                  <Tile key={project.id} project={project} model={model} />
                ))}
              </ul>
            )}
          </section>
        ) : null}

        {ready && model.pending.length > 0 ? (
          <section className="wr-pending" aria-labelledby="wr-pending-title">
            <h2 id="wr-pending-title" className="wr-section-title">
              {t('projects.pendingTitle')}
            </h2>
            <p className="wr-muted">{t('projects.pendingBody')}</p>
            <ul className="wr-grid">
              {model.pending.map((project) => (
                <li key={project.id} className="wr-tile wr-tile-pending">
                  <h3 className="wr-tile-title">
                    <bdi dir="auto">{project.name}</bdi>
                  </h3>
                  <p className="wr-mono wr-tile-meta">
                    {project.delete_after ? t('project.deletesOn', { date: formatDate(project.delete_after) }) : null}
                  </p>
                  <div className="wr-tile-actions">
                    <button type="button" className="wr-btn wr-btn-small" onClick={() => model.restore(project.id)}>
                      {t('project.restore')}
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          </section>
        ) : null}
      </main>
      <footer className="wr-wrap wr-footer wr-mono">{t('layout.footer')}</footer>
    </div>
  );
}

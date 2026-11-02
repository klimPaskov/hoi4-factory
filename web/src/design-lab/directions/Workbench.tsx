import { useId, useState } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import type { Project } from '../../api/types';
import { Field } from '../../components/ui';
import { formatDate, formatNumber } from '../../format';
import { SAMPLE_ACCOUNT } from '../fixtures';
import { WorkbenchMark } from '../marks';
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
import './workbench.css';

function PlusIcon() {
  return (
    <svg className="wb-icon" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path d="M7.25 2.5h1.5v4.75h4.75v1.5H8.75v4.75h-1.5V8.75H2.5v-1.5h4.75z" />
    </svg>
  );
}

function SearchIcon() {
  return (
    <svg className="wb-icon" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path d="M7 2.5a4.5 4.5 0 0 1 3.6 7.2l3 3-1.1 1.1-3-3A4.5 4.5 0 1 1 7 2.5Zm0 1.5a3 3 0 1 0 0 6 3 3 0 0 0 0-6Z" />
    </svg>
  );
}

function Row({ project, model }: { project: Project; model: SampleProjects }) {
  const { t } = useTranslation();
  const here = useHere();
  const row = useRowActions(project, model);
  const columns = 'designLab.sample.columns';
  return (
    <>
      <tr className={row.mode === 'idle' ? undefined : 'is-active'}>
        <th scope="row" className="wb-name" data-label={t(`${columns}.name`)}>
          <Link to={here}>
            <bdi dir="auto">{project.name}</bdi>
          </Link>
        </th>
        <td data-label={t(`${columns}.visibility`)}>
          <span className={`wb-badge wb-badge-${project.visibility}`}>
            {t(`project.visibility.${project.visibility}`)}
          </span>
        </td>
        <td data-label={t(`${columns}.role`)}>{project.role ? t(`project.role.${project.role}`) : null}</td>
        <td data-label={t(`${columns}.created`)}>
          <time dateTime={project.created_at}>{formatDate(project.created_at)}</time>
        </td>
        <td className="wb-row-actions">
          {row.canRename ? (
            <button
              type="button"
              className="wb-btn wb-btn-small wb-btn-ghost"
              aria-expanded={row.mode === 'rename'}
              onClick={() => (row.mode === 'rename' ? row.close() : row.open('rename'))}
            >
              {t('project.rename')}
            </button>
          ) : null}
          {row.canDelete ? (
            <button
              type="button"
              className="wb-btn wb-btn-small wb-btn-ghost wb-btn-danger-text"
              aria-expanded={row.mode === 'delete'}
              onClick={() => (row.mode === 'delete' ? row.close() : row.open('delete'))}
            >
              {t('project.delete')}
            </button>
          ) : null}
        </td>
      </tr>
      {row.mode === 'rename' ? (
        <tr className="wb-panel-row">
          <td colSpan={5}>
            <form className="wb-inline-form" onSubmit={row.submitRename} noValidate>
              <Field label={t('project.newName')} value={row.name} onChange={row.setName} error={row.error} maxLength={400} />
              <div className="wb-actions">
                <button type="submit" className="wb-btn wb-btn-primary">
                  {t('common.save')}
                </button>
                <button type="button" className="wb-btn" onClick={row.close}>
                  {t('common.cancel')}
                </button>
              </div>
            </form>
          </td>
        </tr>
      ) : null}
      {row.mode === 'delete' ? (
        <tr className="wb-panel-row">
          <td colSpan={5}>
            <div className="wb-confirm" role="group" aria-label={t('project.confirmDeleteLabel')}>
              <p>
                <ConfirmDeleteText name={project.name} nameClass="wb-strong" />
              </p>
              <div className="wb-actions">
                <button type="button" className="wb-btn wb-btn-danger" onClick={row.confirmDelete}>
                  {t('project.confirmDeleteButton')}
                </button>
                <button type="button" className="wb-btn" onClick={row.close}>
                  {t('common.cancel')}
                </button>
              </div>
            </div>
          </td>
        </tr>
      ) : null}
    </>
  );
}

export function WorkbenchDirection({ state, unverified }: DirectionProps) {
  const { t } = useTranslation();
  const here = useHere();
  const retry = useRetry();
  const model = useSampleProjects(state);
  const filter = useFilter(model.active);
  const gameReference = useGameReferenceText();
  const [menuOpen, setMenuOpen] = useState(false);
  const [formOpen, setFormOpen] = useState(false);
  const form = useNewProject(model, () => setFormOpen(false));
  const filterId = useId();
  const ready = state !== 'loading' && state !== 'error';

  function openForm() {
    setFormOpen(true);
    requestAnimationFrame(() => form.inputRef.current?.focus());
  }

  return (
    <div className="dir-workbench">
      <div className="wb-shell">
        <header className="wb-side">
          <div className="wb-side-top">
            <Link to={here} className="wb-brand">
              <WorkbenchMark />
              <span>{t('app.name')}</span>
            </Link>
            <button
              type="button"
              className="wb-btn wb-btn-ghost wb-menu-button"
              aria-expanded={menuOpen}
              aria-controls="wb-side-panel"
              onClick={() => setMenuOpen((open) => !open)}
            >
              {t('designLab.sample.menu')}
            </button>
          </div>
          <div id="wb-side-panel" className={`wb-side-panel${menuOpen ? ' is-open' : ''}`}>
            <nav aria-label={t('layout.primaryNav')}>
              <ul className="wb-nav">
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
            <div className="wb-side-bottom">
              <p className="wb-gameref">
                <span className="wb-dot" aria-hidden="true" />
                <span>
                  <span className="wb-gameref-label">{t('gameReference.label')}</span> {gameReference}
                </span>
              </p>
              <ThemeControl className="wb-theme" />
              <div className="wb-user">
                <p>{t('designLab.sample.signedInAs', { name: SAMPLE_ACCOUNT.displayName })}</p>
                <button type="button" className="wb-btn wb-btn-small">
                  {t('common.signOut')}
                </button>
              </div>
            </div>
          </div>
        </header>

        <div className="wb-body">
          {unverified ? (
            <div className="wb-banner" role="region" aria-label={t('verify.bannerLabel')}>
              <p>
                {t('verify.bannerText')} <bdi className="wb-strong">{SAMPLE_ACCOUNT.email}</bdi>
              </p>
              <button type="button" className="wb-btn wb-btn-small">
                {t('verify.resend')}
              </button>
            </div>
          ) : null}

          <main className="wb-main">
            <div className="wb-page-head">
              <div>
                <h1>{t('projects.title')}</h1>
                {ready && model.active.length > 0 ? (
                  <p className="wb-sub">{t('designLab.sample.projectCount', { count: model.active.length })}</p>
                ) : null}
              </div>
              {ready && !formOpen ? (
                <button type="button" className="wb-btn wb-btn-primary" onClick={openForm}>
                  <PlusIcon />
                  {t('projects.newTitle')}
                </button>
              ) : null}
            </div>

            {formOpen ? (
              <form className="wb-card wb-new" onSubmit={form.submit} noValidate aria-labelledby="wb-new-title">
                <h2 id="wb-new-title">{t('projects.newTitle')}</h2>
                <Field
                  label={t('projects.nameLabel')}
                  hint={t('projects.nameHint')}
                  value={form.name}
                  onChange={form.setName}
                  error={form.error}
                  inputRef={form.inputRef}
                  maxLength={400}
                />
                <div className="wb-actions">
                  <button type="submit" className="wb-btn wb-btn-primary">
                    {t('projects.create')}
                  </button>
                  <button type="button" className="wb-btn" onClick={() => setFormOpen(false)}>
                    {t('common.cancel')}
                  </button>
                </div>
              </form>
            ) : null}

            {state === 'loading' ? (
              <div className="wb-card wb-skeleton" role="status">
                <span className="sr-only">{t('projects.loading')}</span>
                {[0, 1, 2, 3, 4].map((n) => (
                  <div key={n} className="wb-skeleton-row" aria-hidden="true">
                    <span />
                    <span />
                    <span />
                  </div>
                ))}
              </div>
            ) : null}

            {state === 'error' ? (
              <div className="wb-alert" role="alert">
                <p>{t('errors.class.safe_same_inputs')}</p>
                <button type="button" className="wb-btn wb-btn-small" onClick={retry}>
                  {t('common.tryAgain')}
                </button>
              </div>
            ) : null}

            {ready && model.active.length === 0 ? (
              <div className="wb-empty">
                <svg className="wb-empty-icon" viewBox="0 0 48 48" aria-hidden="true" focusable="false">
                  <path d="M6 14a4 4 0 0 1 4-4h10l4 4h14a4 4 0 0 1 4 4v18a4 4 0 0 1-4 4H10a4 4 0 0 1-4-4z" />
                  <path className="wb-empty-plus" d="M23 21h2v5h5v2h-5v5h-2v-5h-5v-2h5z" />
                </svg>
                <h2>{t('projects.emptyTitle')}</h2>
                <p>{t('projects.emptyBody')}</p>
                {!formOpen ? (
                  <button type="button" className="wb-btn wb-btn-primary" onClick={openForm}>
                    {t('projects.emptyAction')}
                  </button>
                ) : null}
              </div>
            ) : null}

            {ready && model.active.length > 0 ? (
              <section aria-labelledby="wb-list-title">
                <h2 id="wb-list-title" className="sr-only">
                  {t('projects.yourProjects')}
                </h2>
                <div className="wb-filter">
                  <label htmlFor={filterId} className="sr-only">
                    {t('designLab.sample.filter')}
                  </label>
                  <SearchIcon />
                  <input
                    id={filterId}
                    type="search"
                    value={filter.query}
                    onChange={(event) => filter.setQuery(event.target.value)}
                    placeholder={t('designLab.sample.filter')}
                  />
                </div>
                {filter.shown.length === 0 ? (
                  <p className="wb-sub">{t('designLab.sample.noMatch', { query: filter.query })}</p>
                ) : (
                  <div className="wb-table-wrap">
                    <table className="wb-table">
                      <thead>
                        <tr>
                          <th scope="col">{t('designLab.sample.columns.name')}</th>
                          <th scope="col">{t('designLab.sample.columns.visibility')}</th>
                          <th scope="col">{t('designLab.sample.columns.role')}</th>
                          <th scope="col">{t('designLab.sample.columns.created')}</th>
                          <th scope="col">
                            <span className="sr-only">{t('designLab.sample.columns.actions')}</span>
                          </th>
                        </tr>
                      </thead>
                      <tbody>
                        {filter.shown.map((project) => (
                          <Row key={project.id} project={project} model={model} />
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </section>
            ) : null}

            {ready && model.pending.length > 0 ? (
              <details className="wb-pending">
                <summary>
                  {t('projects.pendingTitle')} <span className="wb-count">{formatNumber(model.pending.length)}</span>
                </summary>
                <p className="wb-sub">{t('projects.pendingBody')}</p>
                <ul className="wb-pending-list">
                  {model.pending.map((project) => (
                    <li key={project.id}>
                      <div>
                        <bdi dir="auto" className="wb-strong">
                          {project.name}
                        </bdi>
                        <p className="wb-sub">
                          {project.delete_after ? t('project.deletesOn', { date: formatDate(project.delete_after) }) : null}
                        </p>
                      </div>
                      <button type="button" className="wb-btn wb-btn-small" onClick={() => model.restore(project.id)}>
                        {t('project.restore')}
                      </button>
                    </li>
                  ))}
                </ul>
              </details>
            ) : null}
          </main>
          <footer className="wb-footer">{t('layout.footer')}</footer>
        </div>
      </div>
    </div>
  );
}

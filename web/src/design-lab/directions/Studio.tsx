import { useState } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import type { Project } from '../../api/types';
import { Field } from '../../components/ui';
import { formatDate } from '../../format';
import { SAMPLE_ACCOUNT } from '../fixtures';
import { StudioMark } from '../marks';
import {
  ConfirmDeleteText,
  ThemeControl,
  nameHash,
  useGameReferenceText,
  useHere,
  useNewProject,
  useRetry,
  useRowActions,
  useSampleProjects,
  type DirectionProps,
  type SampleProjects,
} from '../shared';
import './studio.css';

const FLAG_COLORS = 8;

/** A decorative tricolor picked from the project name, so each project is easy to spot. */
function Flag({ name }: { name: string }) {
  const hash = nameHash(name);
  const first = hash % FLAG_COLORS;
  const second = (first + 1 + ((hash >>> 3) % (FLAG_COLORS - 1))) % FLAG_COLORS;
  let third = (hash >>> 7) % FLAG_COLORS;
  if (third === second) third = first;
  const direction = (hash >>> 11) % 3 === 0 ? 'v' : 'h';
  return (
    <div className={`st-flag st-flag-${direction}`} aria-hidden="true">
      <span className={`st-c${first}`} />
      <span className={`st-c${second}`} />
      <span className={`st-c${third}`} />
    </div>
  );
}

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => Array.from(part)[0] ?? '')
    .join('')
    .toLocaleUpperCase();
}

function Card({ project, model }: { project: Project; model: SampleProjects }) {
  const { t } = useTranslation();
  const here = useHere();
  const row = useRowActions(project, model);
  return (
    <li className="st-card">
      <Flag name={project.name} />
      <div className="st-card-body">
        <h3 className="st-card-title">
          <Link to={here}>
            <bdi dir="auto">{project.name}</bdi>
          </Link>
        </h3>
        <p className="st-pills">
          <span className={`st-pill st-pill-${project.visibility}`}>{t(`project.visibility.${project.visibility}`)}</span>
          {project.role && project.role !== 'owner' ? (
            <span className="st-pill">{t(`project.role.${project.role}`)}</span>
          ) : null}
        </p>
        <p className="st-date">
          <time dateTime={project.created_at}>{t('projects.created', { date: formatDate(project.created_at) })}</time>
        </p>
        {row.mode === 'idle' && (row.canRename || row.canDelete) ? (
          <div className="st-card-actions">
            {row.canRename ? (
              <button type="button" className="st-btn st-btn-soft st-btn-small" onClick={() => row.open('rename')}>
                {t('project.rename')}
              </button>
            ) : null}
            {row.canDelete ? (
              <button type="button" className="st-btn st-btn-quiet st-btn-small" onClick={() => row.open('delete')}>
                {t('project.delete')}
              </button>
            ) : null}
          </div>
        ) : null}
        {row.mode === 'rename' ? (
          <form className="st-inline" onSubmit={row.submitRename} noValidate>
            <Field label={t('project.newName')} value={row.name} onChange={row.setName} error={row.error} maxLength={400} />
            <div className="st-actions">
              <button type="submit" className="st-btn st-btn-primary st-btn-small">
                {t('common.save')}
              </button>
              <button type="button" className="st-btn st-btn-soft st-btn-small" onClick={row.close}>
                {t('common.cancel')}
              </button>
            </div>
          </form>
        ) : null}
        {row.mode === 'delete' ? (
          <div className="st-inline st-confirm" role="group" aria-label={t('project.confirmDeleteLabel')}>
            <p>
              <ConfirmDeleteText name={project.name} nameClass="st-strong" />
            </p>
            <div className="st-actions">
              <button type="button" className="st-btn st-btn-danger st-btn-small" onClick={row.confirmDelete}>
                {t('project.confirmDeleteButton')}
              </button>
              <button type="button" className="st-btn st-btn-soft st-btn-small" onClick={row.close}>
                {t('common.cancel')}
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </li>
  );
}

export function StudioDirection({ state, unverified }: DirectionProps) {
  const { t } = useTranslation();
  const here = useHere();
  const retry = useRetry();
  const model = useSampleProjects(state);
  const [formOpen, setFormOpen] = useState(false);
  const form = useNewProject(model, () => setFormOpen(false));
  const gameReference = useGameReferenceText();
  const ready = state !== 'loading' && state !== 'error';

  function openForm() {
    setFormOpen(true);
    requestAnimationFrame(() => form.inputRef.current?.focus());
  }

  const newCard = formOpen ? (
    <li className="st-card st-new-open">
      <form onSubmit={form.submit} noValidate aria-labelledby="st-new-title" className="st-card-body">
        <h2 id="st-new-title" className="st-card-title">
          {t('projects.newTitle')}
        </h2>
        <Field
          label={t('projects.nameLabel')}
          hint={t('projects.nameHint')}
          value={form.name}
          onChange={form.setName}
          error={form.error}
          inputRef={form.inputRef}
          maxLength={400}
        />
        <div className="st-actions">
          <button type="submit" className="st-btn st-btn-primary">
            {t('projects.create')}
          </button>
          <button type="button" className="st-btn st-btn-soft" onClick={() => setFormOpen(false)}>
            {t('common.cancel')}
          </button>
        </div>
      </form>
    </li>
  ) : (
    <li className="st-new">
      <button type="button" className="st-new-button" onClick={openForm}>
        <span className="st-new-plus" aria-hidden="true" />
        <span className="st-new-label">{t('projects.newTitle')}</span>
      </button>
    </li>
  );

  return (
    <div className="dir-studio">
      <header className="st-wrap st-header">
        <Link to={here} className="st-brand">
          <StudioMark />
          <span>{t('app.name')}</span>
        </Link>
        <nav aria-label={t('layout.primaryNav')}>
          <ul className="st-nav">
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
        <div className="st-tools">
          <ThemeControl className="st-theme" />
          <span className="st-avatar" aria-hidden="true">
            {initials(SAMPLE_ACCOUNT.displayName)}
          </span>
          <button type="button" className="st-btn st-btn-soft st-btn-small">
            {t('common.signOut')}
          </button>
        </div>
      </header>

      {unverified ? (
        <div className="st-wrap">
          <div className="st-banner" role="region" aria-label={t('verify.bannerLabel')}>
            <svg className="st-banner-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path d="M4 6h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1Zm1 2.2V16h14V8.2l-7 4.6z" />
            </svg>
            <p>
              {t('verify.bannerText')} <bdi className="st-strong">{SAMPLE_ACCOUNT.email}</bdi>
            </p>
            <button type="button" className="st-btn st-btn-primary st-btn-small">
              {t('verify.resend')}
            </button>
          </div>
        </div>
      ) : null}

      <main className="st-wrap st-main">
        <section className="st-hero" aria-labelledby="st-title">
          <p className="st-hello">{t('designLab.sample.welcome', { name: SAMPLE_ACCOUNT.displayName })}</p>
          <h1 id="st-title">{t('projects.title')}</h1>
          <p className="st-lead">{t('designLab.sample.lead')}</p>
          <p className="st-gameref">
            <span className="st-gameref-dot" aria-hidden="true" />
            {t('gameReference.label')} {gameReference}
          </p>
        </section>

        {state === 'loading' ? (
          <div role="status">
            <span className="sr-only">{t('projects.loading')}</span>
            <ul className="st-grid" aria-hidden="true">
              {[0, 1, 2].map((n) => (
                <li key={n} className="st-card st-ghost">
                  <div className="st-flag st-flag-h">
                    <span />
                  </div>
                  <div className="st-card-body">
                    <span className="st-ghost-line" />
                    <span className="st-ghost-line st-ghost-short" />
                  </div>
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        {state === 'error' ? (
          <div className="st-alert" role="alert">
            <p>{t('errors.class.safe_same_inputs')}</p>
            <button type="button" className="st-btn st-btn-soft st-btn-small" onClick={retry}>
              {t('common.tryAgain')}
            </button>
          </div>
        ) : null}

        {ready && model.active.length === 0 && !formOpen ? (
          <div className="st-empty">
            <div className="st-empty-art" aria-hidden="true">
              <StudioMark />
            </div>
            <h2>{t('projects.emptyTitle')}</h2>
            <p>{t('projects.emptyBody')}</p>
            <button type="button" className="st-btn st-btn-primary st-btn-large" onClick={openForm}>
              {t('projects.emptyAction')}
            </button>
          </div>
        ) : null}

        {ready && (model.active.length > 0 || formOpen) ? (
          <section aria-labelledby="st-list-title">
            <h2 id="st-list-title" className="sr-only">
              {t('projects.yourProjects')}
            </h2>
            <ul className="st-grid">
              {newCard}
              {model.active.map((project) => (
                <Card key={project.id} project={project} model={model} />
              ))}
            </ul>
          </section>
        ) : null}

        {ready && model.pending.length > 0 ? (
          <section className="st-pending" aria-labelledby="st-pending-title">
            <h2 id="st-pending-title">{t('projects.pendingTitle')}</h2>
            <p>{t('projects.pendingBody')}</p>
            <ul className="st-pending-list">
              {model.pending.map((project) => (
                <li key={project.id}>
                  <div>
                    <bdi dir="auto" className="st-strong">
                      {project.name}
                    </bdi>
                    <p>{project.delete_after ? t('project.deletesOn', { date: formatDate(project.delete_after) }) : null}</p>
                  </div>
                  <button type="button" className="st-btn st-btn-soft st-btn-small" onClick={() => model.restore(project.id)}>
                    {t('project.restore')}
                  </button>
                </li>
              ))}
            </ul>
          </section>
        ) : null}
      </main>
      <footer className="st-wrap st-footer">{t('layout.footer')}</footer>
    </div>
  );
}

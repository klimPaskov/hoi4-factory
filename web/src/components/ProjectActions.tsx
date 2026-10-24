import { useEffect, useRef, useState, type FormEvent } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { deleteProject, renameProject, restoreProject } from '../api/accounts';
import { ApiError } from '../api/client';
import type { Project } from '../api/types';
import { describeError } from '../errors';
import { formatDate } from '../format';
import { useFocusOnError } from '../hooks';
import { ErrorMessage, Field } from './ui';

type Mode = 'idle' | 'rename' | 'delete';

/** Rename and delete (with the 30-day restore window) for one project. */
export function ProjectActions({
  project,
  onChanged,
}: {
  project: Project;
  onChanged: (project: Project) => void;
}) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<Mode>('idle');
  const [name, setName] = useState(project.name);
  const [fieldError, setFieldError] = useState<string | undefined>();
  const [failure, setFailure] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const leftIdle = useRef(false);
  useFocusOnError(formRef, version);

  // Closing a panel returns focus to the button that opened it.
  useEffect(() => {
    if (mode === 'idle' && leftIdle.current) triggerRef.current?.focus();
    if (mode !== 'idle') leftIdle.current = true;
  }, [mode]);

  const canRename = project.role === 'owner' || project.role === 'maintainer';
  const canDelete = project.role === 'owner';
  if (project.deleted_at) return null;
  if (!canRename && !canDelete) return null;

  function close() {
    setMode('idle');
    setFailure(null);
    setFieldError(undefined);
  }

  async function submitRename(event: FormEvent) {
    event.preventDefault();
    setFailure(null);
    if (!name.trim()) {
      setFieldError(t('validation.nameRequired'));
      setVersion((v) => v + 1);
      return;
    }
    setFieldError(undefined);
    setBusy(true);
    try {
      onChanged(await renameProject(project.id, name.trim()));
      close();
    } catch (error) {
      // The typed name stays in the field.
      const described = describeError(error);
      if (error instanceof ApiError && error.code === 'invalid_request') {
        setFieldError(described.message);
      } else {
        setFailure(error);
      }
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  async function confirmDelete() {
    setFailure(null);
    setBusy(true);
    try {
      onChanged(await deleteProject(project.id));
      close();
    } catch (error) {
      setFailure(error);
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="project-actions">
      {mode === 'idle' ? (
        <div className="button-row">
          {canRename ? (
            <button
              type="button"
              className="button secondary small-button"
              ref={triggerRef}
              onClick={() => {
                setName(project.name);
                setMode('rename');
              }}
            >
              {t('project.rename')}
            </button>
          ) : null}
          {canDelete ? (
            <button
              type="button"
              className="button danger-outline small-button"
              onClick={() => setMode('delete')}
            >
              {t('project.delete')}
            </button>
          ) : null}
        </div>
      ) : null}

      {mode === 'rename' ? (
        <div ref={formRef}>
          <form onSubmit={(e) => void submitRename(e)} noValidate className="inline-form">
            {failure ? <ErrorMessage error={failure} focusable /> : null}
            <Field
              label={t('project.newName')}
              value={name}
              onChange={setName}
              error={fieldError}
              maxLength={400}
            />
            <div className="button-row">
              <button type="submit" className="button primary small-button" disabled={busy}>
                {busy ? t('common.saving') : t('common.save')}
              </button>
              <button type="button" className="button secondary small-button" onClick={close}>
                {t('common.cancel')}
              </button>
            </div>
          </form>
        </div>
      ) : null}

      {mode === 'delete' ? (
        <div ref={formRef} className="confirm" role="group" aria-label={t('project.confirmDeleteLabel')}>
          {failure ? <ErrorMessage error={failure} focusable /> : null}
          <p>
            <Trans
              i18nKey="project.confirmDelete"
              values={{ name: project.name }}
              components={{ name: <bdi dir="auto" className="strong" /> }}
            />
          </p>
          <div className="button-row">
            <button
              type="button"
              className="button danger small-button"
              disabled={busy}
              onClick={() => void confirmDelete()}
            >
              {busy ? t('project.deleting') : t('project.confirmDeleteButton')}
            </button>
            <button type="button" className="button secondary small-button" onClick={close}>
              {t('common.cancel')}
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

/** A project waiting out its grace period, with the way back. */
export function PendingDeletion({
  project,
  onChanged,
}: {
  project: Project;
  onChanged: (project: Project) => void;
}) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);

  async function restore() {
    setBusy(true);
    setFailure(null);
    try {
      onChanged(await restoreProject(project.id));
    } catch (error) {
      setFailure(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      {failure ? <ErrorMessage error={failure} /> : null}
      <p className="small muted">
        {project.delete_after
          ? t('project.deletesOn', { date: formatDate(project.delete_after) })
          : t('project.deletePending')}
      </p>
      {project.role === 'owner' ? (
        <button
          type="button"
          className="button secondary small-button"
          disabled={busy}
          onClick={() => void restore()}
        >
          {busy ? t('project.restoring') : t('project.restore')}
        </button>
      ) : null}
    </div>
  );
}

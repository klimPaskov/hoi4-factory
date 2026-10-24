import { useMemo, useRef, useState, type ChangeEvent, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { GITHUB_LINK_URL } from '../api/accounts';
import { ApiError } from '../api/client';
import {
  importFromGithub,
  listGithubRepositories,
  scanSetup,
  uploadMod,
  type SetupScan,
  type UploadItem,
} from '../api/intake';
import { useAuth } from '../auth';
import { formatBytes, formatNumber } from '../format';
import { useAsync, useFocusOnError } from '../hooks';
import { Alert, ErrorMessage, Field, Loading, NotAvailableYet } from './ui';

type Path = 'create' | 'upload' | 'github';

interface Props {
  projectId: string;
  /** The workspace changed (a mod was uploaded or imported). */
  onBroughtIn: () => void;
  /** A scan of a new mod is ready for review. */
  onScanned: (scan: SetupScan) => void;
}

/**
 * A failed call to an intake or Setup route. "Not available yet" is its own, calm state:
 * those routes are built separately and answer 404 until they land.
 */
function Failure({ error, what, onRetry }: { error: unknown; what: string; onRetry?: () => void }) {
  if (error instanceof ApiError && error.isUnavailable) return <NotAvailableYet what={what} />;
  return <ErrorMessage error={error} onRetry={onRetry} focusable />;
}

export function BringInMod({ projectId, onBroughtIn, onScanned }: Props) {
  const { t } = useTranslation();
  const [path, setPath] = useState<Path>('create');
  const [githubOpened, setGithubOpened] = useState(false);

  function choose(next: Path) {
    setPath(next);
    if (next === 'github') setGithubOpened(true);
  }

  const options: { value: Path; title: string; body: string }[] = [
    { value: 'create', title: t('bring.create.title'), body: t('bring.create.body') },
    { value: 'upload', title: t('bring.upload.title'), body: t('bring.upload.body') },
    { value: 'github', title: t('bring.github.title'), body: t('bring.github.body') },
  ];

  return (
    <section className="card" aria-labelledby="bring-title">
      <h2 id="bring-title">{t('bring.title')}</h2>
      <fieldset className="path-choice">
        <legend className="sr-only">{t('bring.legend')}</legend>
        {options.map((option) => (
          // The label text is option.title and option.body, rendered below from the translation file.
          // eslint-disable-next-line jsx-a11y/label-has-associated-control
          <label key={option.value} className={`path-option${path === option.value ? ' selected' : ''}`}>
            <input
              type="radio"
              name="bring-path"
              value={option.value}
              checked={path === option.value}
              onChange={() => choose(option.value)}
            />
            <span className="path-text">
              <span className="strong">{option.title}</span>
              <span className="small muted">{option.body}</span>
            </span>
          </label>
        ))}
      </fieldset>

      {/* Panels stay mounted so what was typed survives switching between paths. */}
      <div hidden={path !== 'create'}>
        <CreatePath projectId={projectId} onScanned={onScanned} />
      </div>
      <div hidden={path !== 'upload'}>
        <UploadPath projectId={projectId} onBroughtIn={onBroughtIn} />
      </div>
      <div hidden={path !== 'github'}>
        {githubOpened ? <GithubPath projectId={projectId} onBroughtIn={onBroughtIn} /> : null}
      </div>
    </section>
  );
}

// ----- Create ----------------------------------------------------------------------------

function CreatePath({ projectId, onScanned }: Pick<Props, 'projectId' | 'onScanned'>) {
  const { t } = useTranslation();
  const [description, setDescription] = useState('');
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setFailure(null);
    try {
      onScanned(
        await scanSetup(projectId, {
          mode: 'create',
          description: description.trim() || undefined,
        }),
      );
    } catch (error) {
      setFailure(error);
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  return (
    <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate>
      {failure ? <Failure error={failure} what={t('bring.create.what')} /> : null}
      <Field
        label={t('bring.create.descriptionLabel')}
        hint={t('bring.create.descriptionHint')}
        optional
        multiline
        value={description}
        onChange={setDescription}
        maxLength={2000}
      />
      <button type="submit" className="button primary" disabled={busy}>
        {busy ? t('bring.create.working') : t('bring.create.button')}
      </button>
    </form>
  );
}

// ----- Upload ----------------------------------------------------------------------------

interface Picked {
  kind: 'zip' | 'folder';
  items: UploadItem[];
}

function UploadPath({ projectId, onBroughtIn }: Pick<Props, 'projectId' | 'onBroughtIn'>) {
  const { t } = useTranslation();
  const [picked, setPicked] = useState<Picked | null>(null);
  const [missing, setMissing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  const totalBytes = useMemo(
    () => picked?.items.reduce((sum, item) => sum + item.file.size, 0) ?? 0,
    [picked],
  );

  function pick(kind: Picked['kind']) {
    return (event: ChangeEvent<HTMLInputElement>) => {
      const files = Array.from(event.target.files ?? []);
      setDone(false);
      setMissing(false);
      setPicked(
        files.length === 0
          ? null
          : {
              kind,
              items: files.map((file) => ({
                file,
                path: kind === 'folder' && file.webkitRelativePath ? file.webkitRelativePath : file.name,
              })),
            },
      );
    };
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    setFailure(null);
    setDone(false);
    if (!picked) {
      setMissing(true);
      setVersion((v) => v + 1);
      return;
    }
    setBusy(true);
    try {
      await uploadMod(projectId, picked.kind, picked.items);
      setDone(true);
      onBroughtIn();
    } catch (error) {
      // The chosen files stay selected so the upload can be retried as is.
      setFailure(error);
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  return (
    <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate>
      {failure ? <Failure error={failure} what={t('bring.upload.what')} /> : null}
      {done ? <Alert kind="success">{t('bring.upload.done')}</Alert> : null}
      <div className="field">
        <label htmlFor="upload-zip">{t('bring.upload.zipLabel')}</label>
        <input id="upload-zip" type="file" accept=".zip,application/zip" onChange={pick('zip')} aria-invalid={missing ? true : undefined} aria-describedby={missing ? 'upload-missing' : undefined} />
      </div>
      <div className="field">
        <label htmlFor="upload-folder">{t('bring.upload.folderLabel')}</label>
        <input
          id="upload-folder"
          type="file"
          // Folder selection is a widely supported but non-standard attribute.
          {...({ webkitdirectory: '' } as Record<string, string>)}
          onChange={pick('folder')}
        />
      </div>
      {missing ? (
        <p id="upload-missing" className="field-error">
          {t('bring.upload.missing')}
        </p>
      ) : null}
      {picked ? (
        <p className="small" role="status">
          {t('bring.upload.picked', {
            count: picked.items.length,
            files: formatNumber(picked.items.length),
            size: formatBytes(totalBytes),
          })}
        </p>
      ) : null}
      <button type="submit" className="button primary" disabled={busy}>
        {busy ? t('bring.upload.working') : t('bring.upload.button')}
      </button>
    </form>
  );
}

// ----- GitHub ----------------------------------------------------------------------------

function GithubPath({ projectId, onBroughtIn }: Pick<Props, 'projectId' | 'onBroughtIn'>) {
  const { t } = useTranslation();
  const { state: auth } = useAuth();
  const linked = auth.status === 'authenticated' && auth.account.github_linked;
  if (!linked) {
    return (
      <div>
        <p>{t('bring.github.notLinked')}</p>
        <a className="button secondary" href={GITHUB_LINK_URL}>
          {t('bring.github.connect')}
        </a>
      </div>
    );
  }
  return <GithubImport projectId={projectId} onBroughtIn={onBroughtIn} />;
}

function GithubImport({ projectId, onBroughtIn }: Pick<Props, 'projectId' | 'onBroughtIn'>) {
  const { t } = useTranslation();
  const repos = useAsync((signal) => listGithubRepositories(projectId, signal));
  const [filter, setFilter] = useState('');
  const [repository, setRepository] = useState('');
  const [ref, setRef] = useState('');
  const [missing, setMissing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  if (repos.status === 'loading') return <Loading label={t('bring.github.loading')} />;
  if (repos.status === 'error') {
    return <Failure error={repos.error} what={t('bring.github.what')} onRetry={repos.reload} />;
  }

  const needle = filter.trim().toLowerCase();
  const shown = repos.data.filter((r) => r.full_name.toLowerCase().includes(needle));
  const selected = repos.data.find((r) => r.full_name === repository);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setFailure(null);
    setDone(false);
    if (!repository) {
      setMissing(true);
      setVersion((v) => v + 1);
      return;
    }
    setMissing(false);
    setBusy(true);
    try {
      await importFromGithub(projectId, { repository, ref: ref.trim() || undefined });
      setDone(true);
      onBroughtIn();
    } catch (error) {
      setFailure(error);
      setVersion((v) => v + 1);
    } finally {
      setBusy(false);
    }
  }

  return (
    <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate>
      {failure ? <Failure error={failure} what={t('bring.github.what')} /> : null}
      {done ? <Alert kind="success">{t('bring.github.done')}</Alert> : null}
      {repos.data.length === 0 ? (
        <p>{t('bring.github.noRepositories')}</p>
      ) : (
        <>
          <Field
            label={t('bring.github.filter')}
            type="search"
            dir="ltr"
            value={filter}
            onChange={setFilter}
          />
          <div className="field">
            <label htmlFor="github-repo">{t('bring.github.repository')}</label>
            <select
              id="github-repo"
              value={repository}
              onChange={(event) => setRepository(event.target.value)}
              aria-invalid={missing && !repository ? true : undefined}
              aria-describedby={missing && !repository ? 'github-missing' : undefined}
            >
              <option value="">{t('bring.github.choose')}</option>
              {shown.map((r) => (
                <option key={r.full_name} value={r.full_name}>
                  {r.full_name}
                  {r.private ? ` (${t('bring.github.private')})` : ''}
                </option>
              ))}
            </select>
            {missing && !repository ? (
              <p id="github-missing" className="field-error">
                {t('bring.github.missing')}
              </p>
            ) : null}
          </div>
          <Field
            label={t('bring.github.ref')}
            hint={
              selected
                ? t('bring.github.refHint', { branch: selected.default_branch })
                : t('bring.github.refHintGeneric')
            }
            optional
            dir="ltr"
            value={ref}
            onChange={setRef}
            spellCheck={false}
          />
          <button type="submit" className="button primary" disabled={busy}>
            {busy ? t('bring.github.working') : t('bring.github.button')}
          </button>
        </>
      )}
    </form>
  );
}

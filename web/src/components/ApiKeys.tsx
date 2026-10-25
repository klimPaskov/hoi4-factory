import { useRef, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import {
  addApiKey,
  deleteApiKey,
  listApiKeys,
  type ApiKeyRecord,
  type KeyProvider,
} from '../api/tasks';
import { ApiError } from '../api/client';
import { formatDate } from '../format';
import { useAsync, useFocusOnError } from '../hooks';
import { Alert, EmptyState, ErrorMessage, Field, Loading, NotAvailableYet } from './ui';

const PROVIDERS: KeyProvider[] = ['anthropic', 'openai', 'openai_compatible'];

/** The person's saved model API keys. The key itself is never shown or kept after saving. */
export function ApiKeysSection() {
  const { t } = useTranslation();
  const state = useAsync((signal) => listApiKeys(signal));

  return (
    <section className="card" aria-labelledby="api-keys-title">
      <h2 id="api-keys-title">{t('apiKeys.title')}</h2>
      {state.status === 'loading' ? <Loading label={t('apiKeys.loading')} /> : null}
      {state.status === 'error' ? (
        state.error instanceof ApiError && state.error.isUnavailable ? (
          <NotAvailableYet what={t('apiKeys.what')} />
        ) : (
          <ErrorMessage error={state.error} onRetry={state.reload} />
        )
      ) : null}
      {state.status === 'ready' && !state.data.available ? (
        <Alert kind="info">{t('apiKeys.unavailable')}</Alert>
      ) : null}
      {state.status === 'ready' && state.data.available ? (
        <KeysBody keys={state.data.keys} onChanged={state.reload} />
      ) : null}
    </section>
  );
}

function KeysBody({ keys, onChanged }: { keys: ApiKeyRecord[]; onChanged: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      <p className="muted">{t('apiKeys.intro')}</p>
      {keys.length === 0 ? (
        <EmptyState title={t('apiKeys.empty')} />
      ) : (
        <ul className="project-list" aria-label={t('apiKeys.listLabel')}>
          {keys.map((key) => (
            <KeyItem key={key.id} apiKey={key} onChanged={onChanged} />
          ))}
        </ul>
      )}
      <AddKeyForm onAdded={onChanged} />
    </>
  );
}

function KeyItem({ apiKey, onChanged }: { apiKey: ApiKeyRecord; onChanged: () => void }) {
  const { t } = useTranslation();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);

  async function remove() {
    setBusy(true);
    setFailure(null);
    try {
      await deleteApiKey(apiKey.id);
      onChanged();
    } catch (error) {
      setFailure(error);
      setBusy(false);
    }
  }

  return (
    <li className="card">
      <div className="project-item">
        <div>
          <h3>
            <bdi dir="auto">{apiKey.label}</bdi>
          </h3>
          <p className="muted small">
            {t(`apiKeys.providers.${apiKey.provider}`)} {'· '}
            {t('apiKeys.hint', { hint: apiKey.key_hint })} {'· '}
            {t('apiKeys.added', { date: formatDate(apiKey.created_at) })} {'· '}
            {apiKey.last_used_at
              ? t('apiKeys.lastUsed', { date: formatDate(apiKey.last_used_at) })
              : t('apiKeys.neverUsed')}
          </p>
          {apiKey.base_url ? (
            <p className="muted small" dir="ltr">
              {apiKey.base_url}
            </p>
          ) : null}
        </div>
        {confirming ? null : (
          <button type="button" className="button danger-outline" onClick={() => setConfirming(true)}>
            {t('apiKeys.delete')}
          </button>
        )}
      </div>
      {failure ? <ErrorMessage error={failure} /> : null}
      {confirming ? (
        <div className="confirm" role="group" aria-label={t('apiKeys.confirmDeleteLabel')}>
          <Alert kind="warning">{t('apiKeys.confirmDelete', { label: apiKey.label })}</Alert>
          <div className="button-row">
            <button type="button" className="button danger" disabled={busy} onClick={() => void remove()}>
              {busy ? t('apiKeys.deleting') : t('apiKeys.confirmDeleteButton')}
            </button>
            <button type="button" className="button secondary" onClick={() => setConfirming(false)}>
              {t('common.cancel')}
            </button>
          </div>
        </div>
      ) : null}
    </li>
  );
}

function AddKeyForm({ onAdded }: { onAdded: () => void }) {
  const { t } = useTranslation();
  const [provider, setProvider] = useState<KeyProvider>('anthropic');
  const [label, setLabel] = useState('');
  const [secret, setSecret] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [problems, setProblems] = useState<{ label?: string; key?: string; baseUrl?: string }>({});
  const [failure, setFailure] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);
  const compatible = provider === 'openai_compatible';

  async function submit(event: FormEvent) {
    event.preventDefault();
    setFailure(null);
    setSaved(false);
    const found: typeof problems = {};
    if (!label.trim()) found.label = t('apiKeys.needLabel');
    if (!secret.trim()) found.key = t('apiKeys.needKey');
    if (compatible && !baseUrl.trim()) found.baseUrl = t('apiKeys.needBaseUrl');
    setProblems(found);
    if (Object.keys(found).length > 0) {
      setVersion((v) => v + 1);
      return;
    }
    setBusy(true);
    try {
      await addApiKey({
        provider,
        label: label.trim(),
        api_key: secret.trim(),
        ...(compatible ? { base_url: baseUrl.trim() } : {}),
      });
      setLabel('');
      setBaseUrl('');
      setSaved(true);
      onAdded();
    } catch (error) {
      setFailure(error);
      setVersion((v) => v + 1);
    } finally {
      // The key never stays in the page after the request returns, whatever the answer was.
      setSecret('');
      setBusy(false);
    }
  }

  return (
    <form ref={formRef} onSubmit={(e) => void submit(e)} noValidate aria-labelledby="add-key-title">
      <h3 id="add-key-title">{t('apiKeys.addTitle')}</h3>
      {failure ? <ErrorMessage error={failure} focusable /> : null}
      {saved ? <Alert kind="success">{t('apiKeys.added_ok')}</Alert> : null}
      <div className="field">
        <label htmlFor="key-provider">{t('apiKeys.provider')}</label>
        <select
          id="key-provider"
          value={provider}
          onChange={(event) => setProvider(event.target.value as KeyProvider)}
        >
          {PROVIDERS.map((p) => (
            <option key={p} value={p}>
              {t(`apiKeys.providers.${p}`)}
            </option>
          ))}
        </select>
      </div>
      <Field
        label={t('apiKeys.label')}
        hint={t('apiKeys.labelHint')}
        value={label}
        onChange={setLabel}
        error={problems.label}
        maxLength={100}
      />
      <Field
        label={t('apiKeys.key')}
        type="password"
        autoComplete="off"
        dir="ltr"
        spellCheck={false}
        value={secret}
        onChange={setSecret}
        error={problems.key}
      />
      {compatible ? (
        <Field
          label={t('apiKeys.baseUrl')}
          hint={t('apiKeys.baseUrlHint')}
          dir="ltr"
          spellCheck={false}
          value={baseUrl}
          onChange={setBaseUrl}
          error={problems.baseUrl}
        />
      ) : null}
      <button type="submit" className="button primary" disabled={busy}>
        {busy ? t('apiKeys.adding') : t('apiKeys.add')}
      </button>
    </form>
  );
}

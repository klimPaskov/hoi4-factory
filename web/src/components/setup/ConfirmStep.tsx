import { useRef, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../../api/client';
import { confirmSetupValues, type ReviewValues, type SetupAnalysis } from '../../api/intake';
import { fieldForError } from '../../errors';
import { formatNumber } from '../../format';
import { useFocusOnError } from '../../hooks';
import { Alert, Field } from '../ui';
import { StepError } from './StepError';

type FieldName =
  | 'description'
  | 'folder_profile'
  | 'display_name'
  | 'project_id'
  | 'author'
  | 'version'
  | 'supported_game_version'
  | 'script_prefix'
  | 'primary_namespace'
  | 'default_branch'
  | 'descriptor_tags'
  | 'agents_profile'
  | 'localisation_convention'
  | 'documentation_convention';

const FIELD_NAMES: FieldName[] = [
  'description',
  'folder_profile',
  'display_name',
  'project_id',
  'author',
  'version',
  'supported_game_version',
  'script_prefix',
  'primary_namespace',
  'default_branch',
  'descriptor_tags',
  'agents_profile',
  'localisation_convention',
  'documentation_convention',
];

/** The server names the field in affected_ids.field, or in the message; this matches the message. The longest names come first so "supported_game_version" wins over "version". */
const PATTERNS = Object.fromEntries(
  [...FIELD_NAMES].sort((a, b) => b.length - a.length).map((name) => [name, new RegExp(name.replace(/_/g, '[ _]?'), 'i')]),
) as Record<FieldName, RegExp>;

type FormState = Record<FieldName, string>;

function toForm(values: ReviewValues): FormState {
  return {
    description: values.description,
    folder_profile: values.folder_profile.join('\n'),
    display_name: values.identity.display_name,
    project_id: values.identity.project_id,
    author: values.identity.author,
    version: values.identity.version,
    supported_game_version: values.identity.supported_game_version,
    script_prefix: values.identity.script_prefix,
    primary_namespace: values.identity.primary_namespace,
    default_branch: values.identity.default_branch,
    descriptor_tags: values.identity.descriptor_tags.join(', '),
    agents_profile: values.conventions.agents_profile,
    localisation_convention: values.conventions.localisation_convention,
    documentation_convention: values.conventions.documentation_convention,
  };
}

function toValues(form: FormState): ReviewValues {
  return {
    description: form.description,
    folder_profile: form.folder_profile
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean),
    identity: {
      display_name: form.display_name,
      project_id: form.project_id,
      author: form.author,
      version: form.version,
      supported_game_version: form.supported_game_version,
      script_prefix: form.script_prefix,
      primary_namespace: form.primary_namespace,
      default_branch: form.default_branch,
      descriptor_tags: form.descriptor_tags
        .split(',')
        .map((tag) => tag.trim())
        .filter(Boolean),
    },
    conventions: {
      agents_profile: form.agents_profile,
      localisation_convention: form.localisation_convention,
      documentation_convention: form.documentation_convention,
    },
  };
}

function rejectedField(error: unknown): FieldName | null {
  if (!(error instanceof ApiError)) return null;
  const named = error.affectedIds.field?.split('.').pop();
  if (named && (FIELD_NAMES as string[]).includes(named)) return named as FieldName;
  return fieldForError(error, PATTERNS);
}

function percent(confidence: number): number {
  return Math.round(Math.min(1, Math.max(0, confidence)) * 100);
}

/**
 * Step 4: what the model found, and the editable values. Confirming records exactly what the
 * form shows. Nothing is written to the mod.
 */
export function ConfirmStep({
  projectId,
  analysis,
  confirmed,
  state,
  canEdit,
  onConfirmed,
  onEdit,
  onCancelEdit,
  onApiError,
  onRestart,
}: {
  projectId: string;
  analysis: SetupAnalysis;
  confirmed: ReviewValues | null;
  state: 'current' | 'done';
  /** Editing again is only offered while no plan exists. */
  canEdit: boolean;
  onConfirmed: (values: ReviewValues) => void;
  onEdit: () => void;
  onCancelEdit: () => void;
  onApiError: (error: unknown) => boolean;
  onRestart: () => void;
}) {
  const { t } = useTranslation();

  if (state === 'done') {
    return (
      <div>
        <p>{t('setup.confirm.done')}</p>
        {canEdit ? (
          <p>
            <button type="button" className="button secondary" onClick={onEdit}>
              {t('setup.confirm.edit')}
            </button>
          </p>
        ) : null}
      </div>
    );
  }

  return (
    <ConfirmForm
      projectId={projectId}
      analysis={analysis}
      start={confirmed ?? analysis.values}
      onConfirmed={onConfirmed}
      onCancelEdit={confirmed ? onCancelEdit : undefined}
      onApiError={onApiError}
      onRestart={onRestart}
    />
  );
}

function ConfirmForm({
  projectId,
  analysis,
  start,
  onConfirmed,
  onCancelEdit,
  onApiError,
  onRestart,
}: {
  projectId: string;
  analysis: SetupAnalysis;
  start: ReviewValues;
  onConfirmed: (values: ReviewValues) => void;
  onCancelEdit?: () => void;
  onApiError: (error: unknown) => boolean;
  onRestart: () => void;
}) {
  const { t } = useTranslation();
  const [form, setForm] = useState<FormState>(() => toForm(start));
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);
  const [version, setVersion] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  useFocusOnError(formRef, version);

  const bad = failure ? rejectedField(failure) : null;
  const badMessage = bad && failure instanceof ApiError ? failure.message : undefined;

  function set(name: FieldName) {
    return (value: string) => setForm((previous) => ({ ...previous, [name]: value }));
  }

  function text(name: FieldName, label: string, extra: { hint?: string; ltr?: boolean } = {}) {
    return (
      <Field
        label={label}
        hint={extra.hint}
        dir={extra.ltr ? 'ltr' : 'auto'}
        value={form[name]}
        onChange={set(name)}
        error={bad === name ? badMessage : undefined}
        spellCheck={false}
      />
    );
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setFailure(null);
    const values = toValues(form);
    try {
      await confirmSetupValues(projectId, { analysis_id: analysis.analysis_id, values });
      onConfirmed(values);
    } catch (error) {
      if (!onApiError(error)) {
        setFailure(error);
        setVersion((v) => v + 1);
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <p>{t('setup.confirm.intro')}</p>

      {analysis.summary ? (
        <>
          <h4>{t('setup.confirm.summaryTitle')}</h4>
          <p dir="auto">{analysis.summary}</p>
        </>
      ) : null}
      {analysis.warnings.length > 0 ? (
        <Alert kind="warning">
          <p>
            <strong>{t('setup.confirm.warningsTitle')}</strong>
          </p>
          <ul>
            {analysis.warnings.map((warning, index) => (
              <li key={`${index}:${warning}`} dir="auto">
                {warning}
              </li>
            ))}
          </ul>
        </Alert>
      ) : null}
      {analysis.proposals.length > 0 ? (
        <>
          <h4>{t('setup.confirm.proposalsTitle')}</h4>
          <div className="table-wrap">
            <table>
              <caption className="sr-only">{t('setup.confirm.proposalsCaption')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('setup.confirm.colKey')}</th>
                  <th scope="col">{t('setup.confirm.colProposed')}</th>
                  <th scope="col">{t('setup.confirm.colConfidence')}</th>
                  <th scope="col">{t('setup.confirm.colReason')}</th>
                </tr>
              </thead>
              <tbody>
                {analysis.proposals.map((proposal, index) => (
                  <tr key={`${index}:${proposal.key}`}>
                    <td dir="ltr" className="path">
                      {proposal.key}
                    </td>
                    <td dir="auto">{String(proposal.value)}</td>
                    <td>{t('setup.confirm.percent', { percent: formatNumber(percent(proposal.confidence)) })}</td>
                    <td dir="auto">{proposal.reason}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      ) : null}
      {analysis.component_recommendations.length > 0 ? (
        <>
          <h4>{t('setup.confirm.componentsTitle')}</h4>
          <p className="muted small">{t('setup.confirm.componentsNote')}</p>
          <ul>
            {analysis.component_recommendations.map((item, index) => (
              <li key={`${index}:${item.component_id}`}>
                <strong dir="ltr">{item.component_id}</strong>: <bdi dir="auto">{item.recommendation}</bdi>
                {item.reason ? (
                  <>
                    {' '}
                    <span className="muted" dir="auto">
                      {item.reason}
                    </span>
                  </>
                ) : null}
              </li>
            ))}
          </ul>
        </>
      ) : null}

      <form
        ref={formRef}
        onSubmit={(event) => void submit(event)}
        noValidate
        aria-label={t('setup.confirm.formLabel')}
      >
        {failure ? (
          bad ? (
            <Alert kind="error" focusable>
              {t('setup.confirm.rejected')}
            </Alert>
          ) : (
            <StepError error={failure} what={t('setup.confirm.what')} onRestart={onRestart} />
          )
        ) : null}
        <Field
          label={t('setup.confirm.description')}
          multiline
          rows={4}
          value={form.description}
          onChange={set('description')}
          error={bad === 'description' ? badMessage : undefined}
        />
        <Field
          label={t('setup.confirm.folderProfile')}
          hint={t('setup.confirm.folderProfileHint')}
          multiline
          rows={5}
          dir="ltr"
          value={form.folder_profile}
          onChange={set('folder_profile')}
          error={bad === 'folder_profile' ? badMessage : undefined}
          spellCheck={false}
        />
        <fieldset className="plain-fieldset">
          <legend>{t('setup.confirm.identityTitle')}</legend>
          {text('display_name', t('setup.confirm.displayName'))}
          {text('project_id', t('setup.confirm.projectId'), { ltr: true })}
          {text('author', t('setup.confirm.author'))}
          {text('version', t('setup.confirm.version'), { ltr: true })}
          {text('supported_game_version', t('setup.confirm.gameVersion'), { ltr: true })}
          {text('script_prefix', t('setup.confirm.scriptPrefix'), { ltr: true })}
          {text('primary_namespace', t('setup.confirm.namespace'), { ltr: true })}
          {text('default_branch', t('setup.confirm.defaultBranch'), { ltr: true })}
          {text('descriptor_tags', t('setup.confirm.tags'), { hint: t('setup.confirm.tagsHint') })}
        </fieldset>
        <fieldset className="plain-fieldset">
          <legend>{t('setup.confirm.conventionsTitle')}</legend>
          {text('agents_profile', t('setup.confirm.agentsProfile'))}
          {text('localisation_convention', t('setup.confirm.localisation'))}
          {text('documentation_convention', t('setup.confirm.documentation'))}
        </fieldset>
        <div className="button-row">
          <button type="submit" className="button primary" disabled={busy}>
            {busy ? t('setup.confirm.working') : t('setup.confirm.button')}
          </button>
          {onCancelEdit ? (
            <button type="button" className="button secondary" onClick={onCancelEdit} disabled={busy}>
              {t('setup.confirm.cancelEdit')}
            </button>
          ) : null}
        </div>
      </form>
    </div>
  );
}

// The parts of the focus editor that the variants arrange differently: search and add, the
// node list, the inspector, the problems list, the agent's suggestion banner and the notices.
// Text is plain words. Anything technical sits behind the one Details toggle of the inspector.
import { useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Chip, Field, SelectField } from '../../components/ui';
import type { FocusNode } from '../../focus/types';
import { PENDING_BEFORE } from './fixtures';
import type { FocusModel } from './useFocusModel';

export function sourceHref(path: string, line: number): string {
  return `#source=${path}:${line}`;
}

export function SourceLink({ path, line }: { path: string; line: number }) {
  const { t } = useTranslation();
  return (
    <a href={sourceHref(path, line)} className="fx-source-link">
      {t('designLab.focus.viewSource')} <span className="mono">{`${path.split('/').pop()}:${line}`}</span>
    </a>
  );
}

const labelOf = (model: FocusModel, id: string): string => model.nodes.find((n) => n.id === id)?.label ?? id;

// ----- Search and add ---------------------------------------------------------------------

export function SearchBox({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  const id = useId();
  return (
    <div className="fx-search">
      <label htmlFor={id} className="sr-only">
        {t('designLab.focus.search.label')}
      </label>
      <input
        id={id}
        type="search"
        value={model.query}
        placeholder={t('designLab.focus.search.placeholder')}
        onChange={(event) => model.setQuery(event.target.value)}
        onKeyDown={(event) => {
          const first = model.matches[0];
          if (event.key === 'Enter' && first) {
            event.preventDefault();
            model.reveal(first.id);
          }
        }}
      />
    </div>
  );
}

export function AddButton({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  return (
    <Button size="sm" variant="ghost" icon="plus" onClick={model.addFocus} disabled={model.refreshing}>
      {t('designLab.focus.add')}
    </Button>
  );
}

export function Toolbar({ model, children }: { model: FocusModel; children?: ReactNode }) {
  return (
    <div className="fx-toolbar">
      <SearchBox model={model} />
      <AddButton model={model} />
      {children}
    </div>
  );
}

// ----- The agent's suggestion and notices -------------------------------------------------

export function PendingBar({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  if (model.pendingCount === 0) return null;
  return (
    <div className="fx-pending-bar" role="region" aria-label={t('designLab.focus.pending.region')}>
      <p>{t('designLab.focus.pending.banner', { count: model.pendingCount })}</p>
      <div className="button-row">
        <Button size="sm" variant="primary" onClick={model.applyAll} disabled={model.refreshing}>
          {t('designLab.focus.pending.applyAll')}
        </Button>
        <Button size="sm" onClick={model.discardAll} disabled={model.refreshing}>
          {t('designLab.focus.pending.discardAll')}
        </Button>
      </div>
    </div>
  );
}

export function Banners({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  const notice = model.notice;
  const quiet = notice && (notice.key === 'added' || notice.key === 'linked');
  const text = notice ? t(`designLab.focus.notice.${notice.key}`, { id: labelOf(model, notice.id ?? ''), other: labelOf(model, notice.other ?? '') }) : '';
  return (
    <>
      {model.refreshing ? (
        <p className="fx-note" role="status">
          <span className="spinner" aria-hidden="true" /> {t('designLab.focus.refreshing')}
        </p>
      ) : null}
      {notice && !quiet ? <p className="fx-note is-warn">{text}</p> : null}
      {notice && quiet ? (
        <p className="sr-only" role="status">
          {text}
        </p>
      ) : null}
    </>
  );
}

// ----- Node list and table ----------------------------------------------------------------

export function NodeList({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  const withProblems = new Set(model.diagnostics.map((d) => d.focusId));
  return (
    <div className="fx-list">
      {model.matches.length === 0 ? (
        <p className="muted">{t('designLab.focus.list.empty')}</p>
      ) : (
        <ul>
          {model.matches.map((node) => (
            <li key={node.id}>
              <button type="button" className={`fx-row${node.id === model.selectedId ? ' is-selected' : ''}`} aria-pressed={node.id === model.selectedId} onClick={() => model.reveal(node.id)}>
                <span className="fx-row-label">{node.label}</span>
                <span className="fx-row-meta">
                  {node.pending ? <span className={`fx-flag is-${node.pending.change}`}>{t(`designLab.focus.node.${node.pending.change}`)}</span> : null}
                  {withProblems.has(node.id) ? <span className="fx-flag is-problem">{t('designLab.focus.list.problem')}</span> : null}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** The same list as a table with the cost editable in the row. */
export function NodeTable({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  return (
    <div className="table-wrap fx-table-wrap">
      <table className="fx-table">
        <caption className="sr-only">{t('designLab.focus.list.title')}</caption>
        <thead>
          <tr>
            <th scope="col">{t('designLab.focus.table.focus')}</th>
            <th scope="col">{t('designLab.focus.fields.cost')}</th>
            <th scope="col">{t('designLab.focus.table.requires')}</th>
          </tr>
        </thead>
        <tbody>
          {model.matches.map((node) => (
            <tr key={node.id} className={node.id === model.selectedId ? 'is-selected' : undefined}>
              <th scope="row">
                <button type="button" className="fx-link" aria-pressed={node.id === model.selectedId} onClick={() => model.reveal(node.id)}>
                  {node.label}
                </button>{' '}
                {node.pending ? <span className={`fx-flag is-${node.pending.change}`}>{t(`designLab.focus.node.${node.pending.change}`)}</span> : null}
              </th>
              <td>
                <input
                  type="number"
                  className="fx-cell-input"
                  aria-label={t('designLab.focus.table.costOf', { name: node.label })}
                  value={node.cost ?? ''}
                  disabled={!node.editable || model.refreshing}
                  onChange={(event) => model.setCost(node.id, event.target.value === '' ? null : Number(event.target.value))}
                />
              </td>
              <td>{node.prerequisiteGroups.map((g) => g.map((id) => labelOf(model, id)).join(` ${t('designLab.focus.or')} `)).join(', ') || t('designLab.focus.none')}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ----- Problems ---------------------------------------------------------------------------

/** One plain sentence per problem. A problem about a focus selects that focus. */
export function DiagnosticsList({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  if (model.diagnostics.length === 0) return <p className="muted">{t('designLab.focus.noProblems')}</p>;
  return (
    <ul className="fx-diags">
      {model.diagnostics.map((d) => (
        <li key={`${d.code}:${d.focusId ?? ''}`}>
          <span className={`fx-dot is-${d.severity}`} aria-hidden="true" />
          {d.focusId ? (
            <button type="button" className="fx-link" onClick={() => model.reveal(d.focusId as string)}>
              {d.message}
            </button>
          ) : (
            <span>{d.message}</span>
          )}
        </li>
      ))}
    </ul>
  );
}

// ----- Inspector --------------------------------------------------------------------------

function NumberField({ label, value, onChange, disabled, min }: { label: string; value: number | null; onChange: (value: number) => void; disabled?: boolean; min?: number }) {
  const id = useId();
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <input id={id} type="number" min={min} step={1} value={value ?? ''} disabled={disabled} onChange={(event) => event.target.value !== '' && onChange(Math.round(Number(event.target.value)))} />
    </div>
  );
}

function Requirements({ model, node, disabled }: { model: FocusModel; node: FocusNode; disabled: boolean }) {
  const { t } = useTranslation();
  const [pick, setPick] = useState('');
  const [group, setGroup] = useState('new');
  const candidates = model.nodes.filter((n) => n.id !== node.id && !node.prerequisiteGroups.flat().includes(n.id));
  return (
    <fieldset className="fx-fieldset" disabled={disabled}>
      <legend>{t('designLab.focus.fields.requires')}</legend>
      {node.prerequisiteGroups.length === 0 ? <p className="muted small">{t('designLab.focus.noRequirements')}</p> : null}
      <ul className="fx-groups">
        {node.prerequisiteGroups.map((members) => (
          <li key={members.join('|')}>
            {members.length > 1 ? <p className="small muted">{t('designLab.focus.anyOf')}</p> : null}
            <div className="chips">
              {members.map((id) => (
                <Chip key={id} onRemove={() => model.unlink(node.id, id)} removeLabel={t('designLab.focus.removeRequirement', { name: labelOf(model, id) })}>
                  {labelOf(model, id)}
                </Chip>
              ))}
            </div>
          </li>
        ))}
      </ul>
      <SelectField
        label={t('designLab.focus.addRequirement')}
        value={pick}
        onChange={setPick}
        options={[{ value: '', label: t('designLab.focus.chooseFocus') }, ...candidates.map((n) => ({ value: n.id, label: n.label }))]}
      />
      {node.prerequisiteGroups.length > 0 && pick ? (
        <SelectField
          label={t('designLab.focus.asPartOf')}
          value={group}
          onChange={setGroup}
          options={[
            { value: 'new', label: t('designLab.focus.newGroup') },
            ...node.prerequisiteGroups.map((g, index) => ({ value: String(index), label: t('designLab.focus.alternativeTo', { name: g.map((id) => labelOf(model, id)).join(', ') }) })),
          ]}
        />
      ) : null}
      {pick ? (
        <Button
          size="sm"
          onClick={() => {
            model.link(pick, node.id, group === 'new' ? undefined : Number(group));
            setPick('');
          }}
        >
          {t('designLab.focus.addRequirementButton')}
        </Button>
      ) : null}
    </fieldset>
  );
}

export function Inspector({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  const node = model.selected;
  if (!node) {
    return (
      <div className="fx-inspector fx-inspector-empty">
        <p className="muted">{t('designLab.focus.inspector.none')}</p>
      </div>
    );
  }
  return <InspectorFor key={node.id} model={model} node={node} />;
}

function InspectorFor({ model, node }: { model: FocusModel; node: FocusNode }) {
  const { t } = useTranslation();
  const locked = !node.editable;
  const disabled = locked || model.refreshing;
  const text = model.texts[node.id] ?? { title: '', description: '' };
  const mine = model.diagnostics.filter((d) => d.focusId === node.id);
  const before = PENDING_BEFORE[node.id];
  return (
    <div className="fx-inspector">
      <header className="fx-inspector-head">
        <h2>{node.label}</h2>
        {node.pending ? (
          <p className="small muted">
            {node.pending.change === 'changed' && before?.cost !== undefined
              ? t('designLab.focus.pending.costChange', { before: before.cost, after: node.cost ?? 0 })
              : t(`designLab.focus.pending.${node.pending.change}Title`)}
          </p>
        ) : null}
      </header>

      {locked ? <p className="fx-note">{t('designLab.focus.lockedNote', { reason: node.notEditableReason })}</p> : null}

      <fieldset className="fx-fieldset" disabled={disabled}>
        <legend className="sr-only">{t('designLab.focus.fields.legend')}</legend>
        <Field label={t('designLab.focus.fields.title')} value={text.title} onChange={(value) => model.setText(node.id, 'title', value)} hint={text.title ? undefined : t('designLab.focus.missingText')} />
        <Field label={t('designLab.focus.fields.description')} value={text.description} multiline rows={3} onChange={(value) => model.setText(node.id, 'description', value)} />
        <div className="fx-numbers">
          <NumberField label={t('designLab.focus.fields.cost')} value={node.cost} min={0} onChange={(value) => model.setCost(node.id, value)} />
          <NumberField label={t('designLab.focus.fields.x')} value={node.x} onChange={(value) => model.move(node.id, value, node.y)} />
          <NumberField label={t('designLab.focus.fields.y')} value={node.y} min={0} onChange={(value) => model.move(node.id, node.x, value)} />
        </div>
      </fieldset>

      <Requirements model={model} node={node} disabled={disabled} />

      {mine.length > 0 ? (
        <ul className="fx-diags" aria-label={t('designLab.focus.problemsHere')}>
          {mine.map((d) => (
            <li key={d.code}>
              <span className={`fx-dot is-${d.severity}`} aria-hidden="true" />
              <span>{d.message}</span>
            </li>
          ))}
        </ul>
      ) : null}

      <details className="fx-more">
        <summary>{t('designLab.details')}</summary>
        <dl className="facts fx-facts">
          <div className="fact">
            <dt>{t('designLab.focus.fields.id')}</dt>
            <dd className="mono">{node.id}</dd>
          </div>
          <div className="fact">
            <dt>{t('designLab.focus.fields.excludes')}</dt>
            <dd>{node.mutuallyExclusive.length ? node.mutuallyExclusive.map((id) => labelOf(model, id)).join(', ') : t('designLab.focus.none')}</dd>
          </div>
          <div className="fact">
            <dt>{t('designLab.focus.fields.placed')}</dt>
            <dd>{node.declared.mode === 'relative' ? t('designLab.focus.placedRelative', { to: labelOf(model, node.declared.relativeTo ?? '') }) : t(`designLab.focus.placed.${node.declared.mode}`)}</dd>
          </div>
          <div className="fact">
            <dt>{t('designLab.focus.fields.icon')}</dt>
            <dd>{node.icon ? <span className="mono">{node.icon.sprite}</span> : t('designLab.focus.none')}</dd>
          </div>
        </dl>
        <SourceLink path={node.source.path} line={node.source.line} />
      </details>
    </div>
  );
}

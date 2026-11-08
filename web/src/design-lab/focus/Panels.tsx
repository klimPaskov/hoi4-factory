// The parts of the focus editor that the variants arrange differently: search and actions, the
// node list, the inspector, diagnostics, the pending-change bar and the refresh banner.
import { useId, useState, type ReactNode } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { Alert, Badge, Button, Chip, Field, SelectField, type Tone } from '../../components/ui';
import type { Diagnostic, FocusNode, LayoutVerdict } from '../../focus/types';
import { PENDING_BEFORE } from './fixtures';
import type { FocusModel } from './useFocusModel';

const LAYOUT_TONE: Record<LayoutVerdict, Tone> = { pass: 'success', revise: 'warning', blocked: 'danger', unknown: 'neutral' };

export function LayoutBadge({ verdict }: { verdict: LayoutVerdict }) {
  const { t } = useTranslation();
  return <Badge tone={LAYOUT_TONE[verdict]}>{t(`designLab.shared.layout.${verdict}`)}</Badge>;
}

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

const SEVERITY_TONE: Record<Diagnostic['severity'], Tone> = { error: 'danger', warning: 'warning', info: 'info' };

// ----- Search and actions -----------------------------------------------------------------

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
      {model.query ? (
        <span className="fx-search-count" role="status">
          {t('designLab.focus.search.count', { shown: model.matches.length, total: model.nodes.length })}
        </span>
      ) : null}
    </div>
  );
}

export function Toolbar({ model, children }: { model: FocusModel; children?: ReactNode }) {
  const { t } = useTranslation();
  const problems = model.diagnostics.length;
  return (
    <div className="fx-toolbar">
      <SearchBox model={model} />
      <LayoutBadge verdict={model.layoutVerdict} />
      <Badge tone={problems ? 'warning' : 'success'}>{t('designLab.focus.problemCount', { count: problems })}</Badge>
      <Button size="sm" icon="plus" onClick={model.addFocus} disabled={model.refreshing}>
        {t('designLab.focus.add')}
      </Button>
      {children}
    </div>
  );
}

// ----- Pending change bar and banners -----------------------------------------------------

export function PendingBar({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  if (model.pendingCount === 0) return null;
  const added = model.nodes.filter((n) => n.pending?.change === 'added').length;
  const changed = model.nodes.filter((n) => n.pending?.change === 'changed').length;
  return (
    <div className="fx-pending-bar" role="region" aria-label={t('designLab.focus.pending.region')}>
      <p>
        <strong>{t('designLab.focus.pending.title')}</strong> {t('designLab.focus.pending.counts', { added, changed })}
      </p>
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
  return (
    <>
      {model.refreshing ? (
        <Alert kind="info">
          <p>
            <span className="spinner" aria-hidden="true" /> {t('designLab.focus.refreshing')}
          </p>
        </Alert>
      ) : null}
      {notice && notice.key !== 'added' && notice.key !== 'linked' ? (
        <Alert kind="warning">
          <p>{t(`designLab.focus.notice.${notice.key}`, { id: notice.id, other: notice.other })}</p>
        </Alert>
      ) : null}
      {notice && (notice.key === 'added' || notice.key === 'linked') ? (
        <p className="sr-only" role="status">
          {t(`designLab.focus.notice.${notice.key}`, { id: notice.id, other: notice.other })}
        </p>
      ) : null}
    </>
  );
}

// ----- Node list --------------------------------------------------------------------------

function stateText(node: FocusNode, t: TFunction): string {
  if (node.pending) return t(`designLab.focus.node.${node.pending.change}`);
  if (!node.editable) return t('designLab.focus.node.locked');
  return '';
}

export function NodeList({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  const counts = new Map<string, number>();
  for (const d of model.diagnostics) if (d.focusId) counts.set(d.focusId, (counts.get(d.focusId) ?? 0) + 1);
  return (
    <div className="fx-list">
      {model.matches.length === 0 ? (
        <p className="muted">{t('designLab.focus.list.empty')}</p>
      ) : (
        <ul>
          {model.matches.map((node) => (
            <li key={node.id}>
              <button type="button" className={`fx-row${node.id === model.selectedId ? ' is-selected' : ''}`} aria-pressed={node.id === model.selectedId} onClick={() => model.reveal(node.id)}>
                <span className="fx-row-main">
                  <span className="fx-row-label">{node.label}</span>
                  <span className="fx-row-id mono">{node.id}</span>
                </span>
                <span className="fx-row-meta">
                  {stateText(node, t) ? <Badge tone={node.pending ? 'accent' : 'neutral'}>{stateText(node, t)}</Badge> : null}
                  {counts.get(node.id) ? <Badge tone="warning">{t('designLab.focus.list.problems', { count: counts.get(node.id) })}</Badge> : null}
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
            <th scope="col">{t('designLab.focus.table.position')}</th>
            <th scope="col">{t('designLab.focus.table.requires')}</th>
            <th scope="col">{t('designLab.focus.table.state')}</th>
          </tr>
        </thead>
        <tbody>
          {model.matches.map((node) => (
            <tr key={node.id} className={node.id === model.selectedId ? 'is-selected' : undefined}>
              <th scope="row">
                <button type="button" className="fx-link" aria-pressed={node.id === model.selectedId} onClick={() => model.reveal(node.id)}>
                  {node.label}
                </button>
                <span className="fx-row-id mono">{node.id}</span>
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
              <td className="mono">{`${node.x}, ${node.y}`}</td>
              <td>{node.prerequisiteGroups.map((g) => g.join(' | ')).join(' + ') || t('designLab.focus.none')}</td>
              <td>{stateText(node, t)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ----- Diagnostics ------------------------------------------------------------------------

export function DiagnosticsList({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  if (model.diagnostics.length === 0) return <p className="muted">{t('designLab.focus.noProblems')}</p>;
  return (
    <ul className="fx-diags">
      {model.diagnostics.map((d) => (
        <li key={`${d.code}:${d.focusId ?? ''}`}>
          <Badge tone={SEVERITY_TONE[d.severity]}>{t(`designLab.focus.severity.${d.severity}`)}</Badge>
          <div>
            <p>{d.message}</p>
            <p className="small muted">
              {d.focusId ? (
                <button type="button" className="fx-link" onClick={() => model.reveal(d.focusId as string)}>
                  {d.focusId}
                </button>
              ) : null}{' '}
              {d.source ? <SourceLink path={d.source.path} line={d.source.line} /> : null}
            </p>
          </div>
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
  const label = (id: string) => model.nodes.find((n) => n.id === id)?.label ?? id;
  const candidates = model.nodes.filter((n) => n.id !== node.id && !node.prerequisiteGroups.flat().includes(n.id));
  return (
    <fieldset className="fx-fieldset" disabled={disabled}>
      <legend>{t('designLab.focus.fields.requires')}</legend>
      {node.prerequisiteGroups.length === 0 ? <p className="muted small">{t('designLab.focus.noRequirements')}</p> : null}
      <ul className="fx-groups">
        {node.prerequisiteGroups.map((members) => (
          <li key={members.join('|')}>
            <p className="small muted">{members.length > 1 ? t('designLab.focus.anyOf') : t('designLab.focus.allOf')}</p>
            <div className="chips">
              {members.map((id) => (
                <Chip key={id} onRemove={() => model.unlink(node.id, id)} removeLabel={t('designLab.focus.removeRequirement', { name: label(id) })}>
                  {label(id)}
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
        options={[{ value: '', label: t('designLab.focus.chooseFocus') }, ...candidates.map((n) => ({ value: n.id, label: `${n.label} (${n.id})` }))]}
      />
      {node.prerequisiteGroups.length > 0 ? (
        <SelectField
          label={t('designLab.focus.asPartOf')}
          value={group}
          onChange={setGroup}
          options={[
            { value: 'new', label: t('designLab.focus.newGroup') },
            ...node.prerequisiteGroups.map((g, index) => ({ value: String(index), label: t('designLab.focus.alternativeTo', { name: g.map(label).join(', ') }) })),
          ]}
        />
      ) : null}
      <Button
        size="sm"
        disabled={!pick}
        onClick={() => {
          model.link(pick, node.id, group === 'new' ? undefined : Number(group));
          setPick('');
        }}
      >
        {t('designLab.focus.addRequirementButton')}
      </Button>
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
        <p className="mono small muted">{node.id}</p>
        {locked ? null : <SourceLink path={node.source.path} line={node.source.line} />}
      </header>

      {locked ? (
        <Alert kind="info">
          <p>
            <strong>{t('designLab.focus.readOnly')}</strong> {node.notEditableReason}
          </p>
          <p>
            <SourceLink path={node.source.path} line={node.source.line} />
          </p>
        </Alert>
      ) : null}

      {node.pending ? (
        <div className="fx-pending-card">
          <p>
            <strong>{t(`designLab.focus.pending.${node.pending.change}Title`)}</strong>
          </p>
          {node.pending.change === 'changed' && before?.cost !== undefined ? (
            <p className="small">{t('designLab.focus.pending.costChange', { before: before.cost, after: node.cost ?? 0 })}</p>
          ) : null}
          <div className="button-row">
            <Button size="sm" variant="primary" onClick={() => model.apply(node.id)} disabled={model.refreshing}>
              {t('designLab.focus.pending.apply')}
            </Button>
            <Button size="sm" onClick={() => model.discard(node.id)} disabled={model.refreshing}>
              {t('designLab.focus.pending.discard')}
            </Button>
          </div>
        </div>
      ) : null}

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

      <dl className="facts fx-facts">
        <div className="fact">
          <dt>{t('designLab.focus.fields.excludes')}</dt>
          <dd>{node.mutuallyExclusive.length ? node.mutuallyExclusive.join(', ') : t('designLab.focus.none')}</dd>
        </div>
        <div className="fact">
          <dt>{t('designLab.focus.fields.placed')}</dt>
          <dd>{node.declared.mode === 'relative' ? t('designLab.focus.placedRelative', { to: node.declared.relativeTo }) : t(`designLab.focus.placed.${node.declared.mode}`)}</dd>
        </div>
        <div className="fact">
          <dt>{t('designLab.focus.fields.icon')}</dt>
          <dd>{node.icon ? <span className="mono">{node.icon.sprite}</span> : t('designLab.focus.none')} {node.icon && !node.icon.resolved ? <Badge tone="warning">{t('designLab.focus.iconMissing')}</Badge> : null}</dd>
        </div>
      </dl>

      {mine.length > 0 ? (
        <div className="fx-node-diags">
          <h3>{t('designLab.focus.problemsHere')}</h3>
          <ul className="fx-diags">
            {mine.map((d) => (
              <li key={d.code}>
                <Badge tone={SEVERITY_TONE[d.severity]}>{t(`designLab.focus.severity.${d.severity}`)}</Badge>
                <p>{d.message}</p>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

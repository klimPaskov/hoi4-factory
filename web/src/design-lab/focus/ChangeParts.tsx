// The sections of the change card. The variants arrange the same sections differently: where
// the verdicts sit, where the diff lives and where the actions are.
import { useId, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, Badge, Button, CheckboxField, SegmentedControl, type Tone } from '../../components/ui';
import type { ChangeCard, CheckRow, SourceRef, Verdict } from '../../focus/types';
import { GamePreview } from './GamePreview';
import { makeCard, NODES, type CardScenario } from './fixtures';
import { LayoutBadge } from './Panels';
import './change.css';

export const short = (hash: string | null): string => (hash ? hash.replace('sha256:', '').slice(0, 8) : '-');

// ----- State of the card in the lab -------------------------------------------------------

export interface CardControl {
  card: ChangeCard;
  /** Why approving is not possible now, or null. */
  blocked: 'stale' | 'failed' | 'done' | null;
  needsAck: boolean;
  ack: boolean;
  setAck: (value: boolean) => void;
  canApprove: boolean;
  approve: () => void;
  discard: () => void;
  discarded: boolean;
}

export function useCard(scenario: CardScenario): CardControl {
  const [card, setCard] = useState<ChangeCard>(() => makeCard(scenario));
  const [ack, setAck] = useState(false);
  const [discarded, setDiscarded] = useState(false);
  const failed = card.checks.some((c) => c.result === 'failed') || card.closure.verdict === 'fail';
  const blocked = card.state === 'committed' || discarded ? 'done' : card.state === 'stale' ? 'stale' : failed ? 'failed' : null;
  const needsAck = card.closure.verdict === 'incomplete' || card.checks.some((c) => c.result === 'unavailable');
  return {
    card,
    blocked,
    needsAck,
    ack,
    setAck,
    canApprove: blocked === null && (!needsAck || ack),
    approve: () => setCard((c) => ({ ...c, state: 'committed' })),
    discard: () => setDiscarded(true),
    discarded,
  };
}

// ----- Header and verdicts ----------------------------------------------------------------

const VERDICT_TONE: Record<Verdict, Tone> = { pass: 'success', incomplete: 'warning', fail: 'danger' };
const CHECK_TONE: Record<CheckRow['result'], Tone> = { passed: 'success', failed: 'danger', unavailable: 'warning', skipped: 'neutral' };

export function CardHeader({ ctl }: { ctl: CardControl }) {
  const { t } = useTranslation();
  const { card } = ctl;
  const state = ctl.discarded ? 'discarded' : card.state;
  return (
    <header className="fc-head">
      <h1>{t('designLab.change.title')}</h1>
      <p className="muted">
        {t('designLab.change.by', { actor: card.actor.name, kind: t(`designLab.change.actor.${card.actor.kind}`) })}{' '}
        <span className="mono">{card.operation}</span>{' '}
        <span className="mono" title={card.proposalHash}>
          {short(card.proposalHash)}
        </span>
      </p>
      <Badge tone={state === 'committed' ? 'success' : state === 'stale' ? 'warning' : state === 'discarded' ? 'neutral' : 'accent'}>
        {t(`designLab.change.state.${state}`)}
      </Badge>
    </header>
  );
}

export function ClosureBadge({ closure }: { closure: ChangeCard['closure'] }) {
  const { t } = useTranslation();
  return <Badge tone={VERDICT_TONE[closure.verdict]}>{t(`designLab.change.closure.${closure.verdict}`)}</Badge>;
}

export function ClosureDetail({ closure }: { closure: ChangeCard['closure'] }) {
  const { t } = useTranslation();
  return (
    <div className={`fc-closure is-${closure.verdict}`}>
      <p>
        <ClosureBadge closure={closure} />{' '}
        {t('designLab.change.closure.counts', { resolved: closure.resolved, unresolved: closure.unresolved, unknown: closure.unknown })}
      </p>
      {closure.verdict === 'incomplete' ? (
        <p className="fc-warn">
          <strong>{t('designLab.change.closure.notAPass')}</strong> {t('designLab.change.closure.unknownMeaning', { count: closure.unknown })}
        </p>
      ) : null}
    </div>
  );
}

export function FindingDelta({ delta }: { delta: ChangeCard['findingDelta'] }) {
  const { t } = useTranslation();
  const keys = ['introduced', 'worsened', 'changed', 'improved', 'unchanged', 'resolved'] as const;
  const bad = (key: (typeof keys)[number]) => (key === 'introduced' || key === 'worsened') && delta[key] > 0;
  return (
    <dl className="fc-delta">
      {keys.map((key) => (
        <div key={key} className={`fc-delta-item${bad(key) ? ' is-bad' : ''}${key === 'improved' || key === 'resolved' ? (delta[key] > 0 ? ' is-good' : '') : ''}`}>
          <dt>{t(`designLab.change.delta.${key}`)}</dt>
          <dd>{delta[key]}</dd>
        </div>
      ))}
    </dl>
  );
}

export function VerdictStrip({ ctl }: { ctl: CardControl }) {
  const { t } = useTranslation();
  const { card } = ctl;
  return (
    <div className="fc-strip" role="group" aria-label={t('designLab.change.verdicts')}>
      <ClosureBadge closure={card.closure} />
      <LayoutBadge verdict={card.layoutVerdict} />
      <Badge tone={card.findingDelta.introduced + card.findingDelta.worsened > 0 ? 'danger' : 'success'}>
        {t('designLab.change.delta.short', { introduced: card.findingDelta.introduced, worsened: card.findingDelta.worsened })}
      </Badge>
    </div>
  );
}

// ----- Summary ----------------------------------------------------------------------------

export function jump(file: string, line: number, files: ChangeCard['files']): void {
  const index = files.findIndex((f) => f.path === file);
  document.getElementById(`diff-${index}-${line}`)?.scrollIntoView({ block: 'center', behavior: 'smooth' });
}

export function SourceButton({ source, onJump, label }: { source: SourceRef; onJump: (source: SourceRef) => void; label?: string }) {
  const { t } = useTranslation();
  return (
    <button type="button" className="fx-link mono" onClick={() => onJump(source)} aria-label={label ?? t('designLab.change.jump', { place: `${source.path}:${source.line}` })}>
      {`${source.path.split('/').pop()}:${source.line}`}
    </button>
  );
}

export function SummarySection({ card, onJump }: { card: ChangeCard; onJump: (source: SourceRef) => void }) {
  const { t } = useTranslation();
  const link = (id: string) => card.sourceLinks.find((l) => l.focusId === id)?.source;
  return (
    <div className="fc-summary">
      <section aria-labelledby="fc-added">
        <h3 id="fc-added">{t('designLab.change.summary.added', { count: card.summary.added.length })}</h3>
        <ul>
          {card.summary.added.map((id) => {
            const source = link(id);
            return (
              <li key={id}>
                <Badge tone="success">{t('designLab.change.summary.plus')}</Badge> <span className="mono">{id}</span> {source ? <SourceButton source={source} onJump={onJump} /> : null}
              </li>
            );
          })}
        </ul>
      </section>
      <section aria-labelledby="fc-changed">
        <h3 id="fc-changed">{t('designLab.change.summary.changed', { count: card.summary.changed.length })}</h3>
        <ul>
          {card.summary.changed.map((change) => {
            const source = link(change.id);
            return (
              <li key={change.id}>
                <span className="mono">{change.id}</span> {source ? <SourceButton source={source} onJump={onJump} /> : null}
                <ul className="fc-fields">
                  {change.fields.map((field) => (
                    <li key={field.name}>
                      <span className="muted">{field.name}</span> <del>{field.before}</del> <span aria-hidden="true">&rarr;</span>
                      <span className="sr-only">{t('designLab.change.summary.to')}</span> <ins>{field.after}</ins>
                    </li>
                  ))}
                </ul>
              </li>
            );
          })}
        </ul>
      </section>
      <section aria-labelledby="fc-removed">
        <h3 id="fc-removed">{t('designLab.change.summary.removed', { count: card.summary.removed.length })}</h3>
        {card.summary.removed.length === 0 ? (
          <p className="muted">{t('designLab.change.summary.nothingRemoved')}</p>
        ) : (
          <ul>
            {card.summary.removed.map((id) => (
              <li key={id} className="mono">
                {id}
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

// ----- Diff -------------------------------------------------------------------------------

interface DiffLine {
  kind: 'add' | 'del' | 'hunk' | 'ctx' | 'meta';
  text: string;
  newLine?: number;
}

export function parseDiff(diff: string): DiffLine[] {
  const out: DiffLine[] = [];
  let next = 0;
  for (const text of diff.split('\n')) {
    if (text.startsWith('---') || text.startsWith('+++')) out.push({ kind: 'meta', text });
    else if (text.startsWith('@@')) {
      next = Number(/\+(\d+)/.exec(text)?.[1] ?? 0);
      out.push({ kind: 'hunk', text });
    } else if (text.startsWith('+')) out.push({ kind: 'add', text, newLine: next++ });
    else if (text.startsWith('-')) out.push({ kind: 'del', text });
    else out.push({ kind: 'ctx', text, newLine: next++ });
  }
  return out;
}

export function DiffSection({ card, open = true }: { card: ChangeCard; open?: boolean }) {
  const { t } = useTranslation();
  return (
    <div className="fc-diff">
      {card.files.map((file, index) => {
        const lines = parseDiff(file.unifiedDiff);
        const added = lines.filter((l) => l.kind === 'add').length;
        const removed = lines.filter((l) => l.kind === 'del').length;
        return (
          <details key={file.path} open={open} className="fc-file">
            <summary>
              <span className="mono">{file.path}</span>
              <span className="fc-file-meta">
                <span className="fc-plus">+{added}</span> <span className="fc-minus">-{removed}</span>{' '}
                <span className="muted mono" title={t('designLab.change.diff.hashes')}>
                  {short(file.beforeSha256)} &rarr; {short(file.afterSha256)}
                </span>
              </span>
            </summary>
            <pre className="fc-pre">
              <code>
                {lines.map((line, i) => (
                  <span key={i} id={line.newLine !== undefined ? `diff-${index}-${line.newLine}` : undefined} className={`fc-line fc-${line.kind}`}>
                    {line.text || ' '}
                  </span>
                ))}
              </code>
            </pre>
          </details>
        );
      })}
    </div>
  );
}

// ----- Preview ----------------------------------------------------------------------------

/** How the before and after renders sit together: next to each other, one at a time, or under a divider. */
export type PreviewMode = 'side' | 'toggle' | 'slider';

const AFTER_NODES = NODES;
// The before render does not draw the focuses the agent added.
const BEFORE_NODES = NODES.filter((node) => node.pending?.change !== 'added');

/** The before and after game renders of the proposal, the changed focuses outlined on both. */
export function PreviewSection({ card, onJump, mode, className }: { card: ChangeCard; onJump: (source: SourceRef) => void; mode: PreviewMode; className?: string }) {
  const { t } = useTranslation();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const [which, setWhich] = useState('after');
  const [pos, setPos] = useState(0.5);
  const changed = useMemo(() => NODES.filter((node) => node.pending), []);

  const select = (id: string | null) => {
    setSelectedId(id);
    const source = card.sourceLinks.find((l) => l.focusId === id)?.source;
    if (source) onJump(source);
  };
  const reveal = (id: string) => {
    select(id);
    setTick((n) => n + 1);
  };
  const common = { diagnostics: [], selectedId, onSelect: select, readOnly: true, revealTick: tick, revealId: selectedId };

  return (
    <div className={`fc-preview fc-preview-${mode}${className ? ` ${className}` : ''}`}>
      <div className="fc-preview-bar">
        {mode === 'toggle' ? (
          <SegmentedControl
            legend={t('designLab.change.preview.show')}
            value={which}
            onChange={setWhich}
            options={[
              { value: 'after', label: t('designLab.change.compare.after') },
              { value: 'before', label: t('designLab.change.compare.before') },
            ]}
          />
        ) : null}
        <ul className="fc-chips" aria-label={t('designLab.change.preview.changedList')}>
          {changed.map((node) => (
            <li key={node.id}>
              <button type="button" className={`fc-chip is-${node.pending?.change}${node.id === selectedId ? ' is-selected' : ''}`} aria-pressed={node.id === selectedId} onClick={() => reveal(node.id)}>
                <span className="fc-chip-kind">{t(`designLab.focus.node.${node.pending?.change}`)}</span> {node.label}
              </button>
            </li>
          ))}
        </ul>
      </div>
      {mode === 'side' ? (
        <div className="fc-sides">
          <figure>
            <figcaption>{t('designLab.change.compare.before')}</figcaption>
            <GamePreview {...common} image="base" nodes={BEFORE_NODES} label={t('designLab.change.preview.labelBefore')} />
          </figure>
          <figure>
            <figcaption>{t('designLab.change.compare.after')}</figcaption>
            <GamePreview {...common} image="proposed" nodes={AFTER_NODES} label={t('designLab.change.preview.labelAfter')} />
          </figure>
        </div>
      ) : null}
      {mode === 'toggle' ? (
        <GamePreview
          {...common}
          image={which === 'before' ? 'base' : 'proposed'}
          nodes={which === 'before' ? BEFORE_NODES : AFTER_NODES}
          label={t(which === 'before' ? 'designLab.change.preview.labelBefore' : 'designLab.change.preview.labelAfter')}
        />
      ) : null}
      {mode === 'slider' ? <GamePreview {...common} image="proposed" nodes={AFTER_NODES} compare={{ pos, onChange: setPos }} label={t('designLab.change.preview.labelSlider')} /> : null}
      <ul className="fc-legend small">
        <li>
          <span className="fc-swatch is-added" aria-hidden="true" /> {t('designLab.change.preview.legendAdded')}
        </li>
        <li>
          <span className="fc-swatch is-changed" aria-hidden="true" /> {t('designLab.change.preview.legendChanged')}
        </li>
        <li className="muted">{t('designLab.change.preview.hint')}</li>
      </ul>
    </div>
  );
}

// ----- Checks, limits, restore point ------------------------------------------------------

export function ChecksSection({ checks }: { checks: CheckRow[] }) {
  const { t } = useTranslation();
  return (
    <ul className="fc-checks">
      {checks.map((check) => (
        <li key={check.name}>
          <Badge tone={CHECK_TONE[check.result]}>{t(`designLab.change.checks.${check.result}`)}</Badge>
          <div>
            <p className="fc-check-name">{check.name}</p>
            <p className="small muted">{check.detail}</p>
          </div>
        </li>
      ))}
    </ul>
  );
}

export function LimitsSection({ limits }: { limits: string[] }) {
  return (
    <ul className="fc-limits">
      {limits.map((limit) => (
        <li key={limit}>{limit}</li>
      ))}
    </ul>
  );
}

export function RestorePoint({ card }: { card: ChangeCard }) {
  const { t } = useTranslation();
  if (!card.restorePoint) return <p className="muted">{t('designLab.change.restore.none')}</p>;
  return (
    <p>
      {t('designLab.change.restore.label')} <span className="mono">{card.restorePoint.ref}</span> <span className="mono muted">{card.restorePoint.oid.slice(0, 10)}</span>
    </p>
  );
}

// ----- Actions ----------------------------------------------------------------------------

const REASON: Record<'stale' | 'failed' | 'done', string> = {
  stale: 'designLab.change.actions.staleReason',
  failed: 'designLab.change.actions.failedReason',
  done: 'designLab.change.actions.doneReason',
};

export function Acknowledge({ ctl }: { ctl: CardControl }) {
  const { t } = useTranslation();
  if (!ctl.needsAck || ctl.blocked) return null;
  return (
    <CheckboxField
      label={t('designLab.change.actions.acknowledge', { count: ctl.card.closure.unknown })}
      checked={ctl.ack}
      onChange={ctl.setAck}
    />
  );
}

export function Actions({ ctl, showAck = true }: { ctl: CardControl; showAck?: boolean }) {
  const { t } = useTranslation();
  const noteId = useId();
  const { card } = ctl;
  const reason = ctl.blocked ? t(REASON[ctl.blocked]) : null;
  const done = card.state === 'committed' || ctl.discarded;
  return (
    <div className="fc-actions">
      {showAck ? <Acknowledge ctl={ctl} /> : null}
      <div className="button-row">
        <Button variant="primary" disabled={!ctl.canApprove} onClick={ctl.approve} aria-describedby={reason ? noteId : undefined}>
          {t('designLab.change.actions.approve')}
        </Button>
        <Button disabled={done} onClick={ctl.discard}>
          {t('designLab.change.actions.discard')}
        </Button>
        <Button disabled aria-describedby={`${noteId}-push`}>
          {t('designLab.change.actions.push')}
        </Button>
      </div>
      {reason && !done ? (
        <p id={noteId} className="small muted">
          {reason}
        </p>
      ) : null}
      {card.state === 'committed' ? (
        <Alert kind="success">
          <p>{t('designLab.change.actions.committed')}</p>
        </Alert>
      ) : null}
      {ctl.discarded ? (
        <Alert kind="info">
          <p>{t('designLab.change.actions.discarded')}</p>
        </Alert>
      ) : null}
      <p id={`${noteId}-push`} className="small muted">
        {card.publication.canPush ? t('designLab.change.actions.pushReady') : (card.publication.reason ?? t('designLab.change.actions.pushNoGrant'))}
      </p>
    </div>
  );
}

export function StaleBanner({ ctl }: { ctl: CardControl }) {
  const { t } = useTranslation();
  if (ctl.card.state !== 'stale') return null;
  return (
    <Alert kind="warning">
      <p>
        <strong>{t('designLab.change.stale.title')}</strong> {t('designLab.change.stale.body')}
      </p>
    </Alert>
  );
}


// The parts of the change card. The variants arrange the same parts differently: where the render
// sits, where the actions are and how the one Details toggle opens. The default view holds only
// what a modder needs to decide: what changed, the render and the actions. Checks, counts and the
// diff sit behind Details.
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, CheckboxField, SegmentedControl, type Tone } from '../../components/ui';
import type { ChangeCard, CheckRow, LayoutVerdict } from '../../focus/types';
import { GamePreview } from './GamePreview';
import { makeCard, NODES, type CardScenario } from './fixtures';
import './change.css';

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

const CHECK_TONE: Record<CheckRow['result'], Tone> = { passed: 'success', failed: 'danger', unavailable: 'warning', skipped: 'neutral' };
const labelOf = (id: string): string => NODES.find((n) => n.id === id)?.label ?? id;

// ----- Header and summary -----------------------------------------------------------------

export function CardHeader({ ctl }: { ctl: CardControl }) {
  const { t } = useTranslation();
  return (
    <header className="fc-head">
      <h1>{t('designLab.change.title')}</h1>
      <p className="muted">{t('designLab.change.by', { who: t(`designLab.change.actor.${ctl.card.actor.kind}`) })}</p>
    </header>
  );
}

/** What changed, in a few plain lines. The colors match the outlines on the render. */
export function SummarySection({ card }: { card: ChangeCard }) {
  const { t } = useTranslation();
  const { added, changed, removed } = card.summary;
  return (
    <ul className="fc-changes" aria-label={t('designLab.change.summary.label')}>
      {added.length > 0 ? (
        <li>
          <span className="fc-swatch is-added" aria-hidden="true" />
          {t('designLab.change.summary.added', { count: added.length, names: added.map(labelOf).join(', ') })}
        </li>
      ) : null}
      {changed.map((change) => (
        <li key={change.id}>
          <span className="fc-swatch is-changed" aria-hidden="true" />
          {t('designLab.change.summary.changed', {
            name: labelOf(change.id),
            what: change.fields.map((f) => t('designLab.change.summary.field', { name: f.name, before: f.before, after: f.after })).join(', '),
          })}
        </li>
      ))}
      {removed.length > 0 ? (
        <li>
          <span className="fc-swatch is-removed" aria-hidden="true" />
          {t('designLab.change.summary.removed', { count: removed.length, names: removed.map(labelOf).join(', ') })}
        </li>
      ) : null}
    </ul>
  );
}

// ----- Preview ----------------------------------------------------------------------------

/** How the before and after renders sit together: next to each other, one at a time, or under a divider. */
export type PreviewMode = 'side' | 'toggle' | 'slider';

const AFTER_NODES = NODES;
// The before render does not draw the focuses the agent added.
const BEFORE_NODES = NODES.filter((node) => node.pending?.change !== 'added');

/** The before and after game renders of the proposal. Only the selection ring and the small pins on changed focuses sit on them. */
export function PreviewSection({ mode, className }: { mode: PreviewMode; className?: string }) {
  const { t } = useTranslation();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [which, setWhich] = useState('after');
  const [pos, setPos] = useState(0.5);
  const common = { diagnostics: [], selectedId, onSelect: setSelectedId, readOnly: true, revealId: selectedId };

  return (
    <div className={`fc-preview fc-preview-${mode}${className ? ` ${className}` : ''}`}>
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
    </div>
  );
}

// ----- Details: counts, checks, diff, limits ----------------------------------------------

interface DiffLine {
  kind: 'add' | 'del' | 'hunk' | 'ctx' | 'meta';
  text: string;
}

export function parseDiff(diff: string): DiffLine[] {
  return diff.split('\n').map((text) => {
    if (text.startsWith('---') || text.startsWith('+++')) return { kind: 'meta', text };
    if (text.startsWith('@@')) return { kind: 'hunk', text };
    if (text.startsWith('+')) return { kind: 'add', text };
    if (text.startsWith('-')) return { kind: 'del', text };
    return { kind: 'ctx', text };
  });
}

function DiffSection({ card }: { card: ChangeCard }) {
  return (
    <div className="fc-diff">
      {card.files.map((file) => {
        const lines = parseDiff(file.unifiedDiff);
        const added = lines.filter((l) => l.kind === 'add').length;
        const removed = lines.filter((l) => l.kind === 'del').length;
        return (
          <details key={file.path} className="fc-file">
            <summary>
              <span className="mono">{file.path}</span>
              <span className="fc-file-meta">
                <span className="fc-plus">+{added}</span> <span className="fc-minus">-{removed}</span>
              </span>
            </summary>
            <pre className="fc-pre">
              <code>
                {lines.map((line, i) => (
                  <span key={i} className={`fc-line fc-${line.kind}`}>
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

function Counts({ card }: { card: ChangeCard }) {
  const { t } = useTranslation();
  const { closure, findingDelta: delta } = card;
  const keys = ['introduced', 'worsened', 'changed', 'improved', 'unchanged', 'resolved'] as const;
  const layout: LayoutVerdict = card.layoutVerdict;
  return (
    <>
      <p>{t('designLab.change.counts.closure', { resolved: closure.resolved, unresolved: closure.unresolved, unknown: closure.unknown })}</p>
      <p>{keys.map((key) => t(`designLab.change.delta.${key}`, { count: delta[key] })).join(', ')}.</p>
      <p>{t(`designLab.shared.layout.${layout}`)}</p>
    </>
  );
}

function ChecksSection({ checks }: { checks: CheckRow[] }) {
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

/** Everything a modder does not need to decide. One toggle, closed by default. */
export function DetailsBody({ card }: { card: ChangeCard }) {
  const { t } = useTranslation();
  return (
    <div className="fc-details-body">
      <section aria-labelledby="fc-d-files">
        <h3 id="fc-d-files">{t('designLab.change.sections.diff')}</h3>
        <DiffSection card={card} />
      </section>
      <section aria-labelledby="fc-d-checks">
        <h3 id="fc-d-checks">{t('designLab.change.sections.checks')}</h3>
        <ChecksSection checks={card.checks} />
        <Counts card={card} />
      </section>
      <section aria-labelledby="fc-d-limits">
        <h3 id="fc-d-limits">{t('designLab.change.sections.limits')}</h3>
        <ul className="fc-limits">
          {card.limits.map((limit) => (
            <li key={limit}>{limit}</li>
          ))}
        </ul>
        <p>
          {card.restorePoint ? (
            <>
              {t('designLab.change.restore.label')} <span className="mono">{card.restorePoint.ref}</span>
            </>
          ) : (
            t('designLab.change.restore.none')
          )}
        </p>
      </section>
    </div>
  );
}

export function ReviewDetails({ card }: { card: ChangeCard }) {
  const { t } = useTranslation();
  return (
    <details className="fc-details">
      <summary>{t('designLab.details')}</summary>
      <DetailsBody card={card} />
    </details>
  );
}

// ----- Actions ----------------------------------------------------------------------------

const REASON: Record<'stale' | 'failed', string> = {
  stale: 'designLab.change.actions.staleReason',
  failed: 'designLab.change.actions.failedReason',
};

/**
 * One main action and one secondary action. A button that cannot be used is not drawn. A blocker is
 * one short sentence. When a check was incomplete, a checkbox with one line sits next to Approve.
 */
export function Actions({ ctl }: { ctl: CardControl }) {
  const { t } = useTranslation();
  const { card } = ctl;
  if (card.state === 'committed') return <p className="fc-result">{t('designLab.change.actions.committed')}</p>;
  if (ctl.discarded) return <p className="fc-result">{t('designLab.change.actions.discarded')}</p>;
  const blocker = ctl.blocked === 'stale' || ctl.blocked === 'failed' ? ctl.blocked : null;
  return (
    <div className="fc-actions">
      {blocker ? <p className="fc-blocker">{t(REASON[blocker])}</p> : null}
      <div className="fc-action-row">
        {ctl.needsAck && !blocker ? (
          <CheckboxField
            label={card.closure.unknown > 0 ? t('designLab.change.actions.acknowledge', { count: card.closure.unknown }) : t('designLab.change.actions.acknowledgeChecks')}
            checked={ctl.ack}
            onChange={ctl.setAck}
          />
        ) : null}
        {blocker ? null : (
          <Button variant="primary" disabled={!ctl.canApprove} onClick={ctl.approve}>
            {t('designLab.change.actions.approve')}
          </Button>
        )}
        <Button onClick={ctl.discard}>{t('designLab.change.actions.discard')}</Button>
      </div>
    </div>
  );
}

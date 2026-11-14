// The parts of the change card (D-61). The variants arrange the same parts differently. What a
// modder sees by default is the game picture of the change, one plain sentence about it and two
// buttons. The diff, the checks, the counts, the limits and the restore point sit behind Details.
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, SegmentedControl, type Tone } from '../../components/ui';
import type { ChangeCard, CheckRow, LayoutVerdict } from '../../focus/types';
import { GamePreview } from './GamePreview';
import { makeCard, NODES, type CardScenario } from './fixtures';
import './change.css';

// ----- State of the card in the lab -------------------------------------------------------

export interface CardControl {
  card: ChangeCard;
  /** What stands in the way of a plain Approve, or null. */
  trouble: 'stale' | 'failed' | 'unchecked' | null;
  /** How it ended, or null while the modder still has to decide. */
  outcome: 'approved' | 'rejected' | 'sentBack' | null;
  approve: () => void;
  reject: () => void;
  /** Hands the change back to the agent to fix or redo. */
  sendBack: () => void;
}

export function useCard(scenario: CardScenario): CardControl {
  const [card] = useState<ChangeCard>(() => makeCard(scenario));
  const [outcome, setOutcome] = useState<CardControl['outcome']>(card.state === 'committed' ? 'approved' : null);
  const failed = card.checks.some((c) => c.result === 'failed') || card.closure.verdict === 'fail';
  const unchecked = card.closure.verdict === 'incomplete' || card.checks.some((c) => c.result === 'unavailable');
  return {
    card,
    trouble: card.state === 'stale' ? 'stale' : failed ? 'failed' : unchecked ? 'unchecked' : null,
    outcome,
    approve: () => setOutcome('approved'),
    reject: () => setOutcome('rejected'),
    sendBack: () => setOutcome('sentBack'),
  };
}

const CHECK_TONE: Record<CheckRow['result'], Tone> = { passed: 'success', failed: 'danger', unavailable: 'warning', skipped: 'neutral' };
const labelOf = (id: string): string => NODES.find((n) => n.id === id)?.label ?? id;

// ----- What changed, in plain words -------------------------------------------------------

/**
 * The agent's own sentence about the change, then one line for each change the picture cannot
 * show (a new cost). New focuses are not listed: the picture outlines them.
 */
export function Summary({ card }: { card: ChangeCard }) {
  const { t } = useTranslation();
  return (
    <div className="cx-summary">
      <p>{t('designLab.change.sampleSummary')}</p>
      {card.summary.changed.map((change) => (
        <p key={change.id} className="cx-fine">
          {t('designLab.change.summary.changed', {
            name: labelOf(change.id),
            what: change.fields.map((f) => t('designLab.change.summary.field', { name: f.name, before: f.before, after: f.after })).join(', '),
          })}
        </p>
      ))}
    </div>
  );
}

// ----- The picture ------------------------------------------------------------------------

/** How the before and after pictures sit together: one at a time, under a divider, or next to each other. */
export type PictureMode = 'toggle' | 'slider' | 'side';

const AFTER_NODES = NODES;
// The before picture does not draw the focuses the agent added, and marks nothing.
const BEFORE_NODES = NODES.filter((node) => node.pending?.change !== 'added').map((node) => ({ ...node, pending: undefined }));

/** The game pictures of the tree before and after the change. New and changed focuses are outlined. */
export function Picture({ mode, under = 0 }: { mode: PictureMode; under?: number }) {
  const { t } = useTranslation();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [which, setWhich] = useState('after');
  const [pos, setPos] = useState(0.5);
  const common = { plain: true, fitInset: under, diagnostics: [], selectedId, onSelect: setSelectedId, readOnly: true, revealId: selectedId };
  if (mode === 'side') {
    return (
      <div className="cx-sides">
        <figure>
          <figcaption>{t('designLab.change.compare.before')}</figcaption>
          <GamePreview {...common} image="base" alignTo="proposed" nodes={BEFORE_NODES} label={t('designLab.change.preview.labelBefore')} />
        </figure>
        <figure>
          <figcaption>{t('designLab.change.compare.after')}</figcaption>
          <GamePreview {...common} image="proposed" nodes={AFTER_NODES} label={t('designLab.change.preview.labelAfter')} />
        </figure>
      </div>
    );
  }
  if (mode === 'slider') {
    return <GamePreview {...common} image="proposed" nodes={AFTER_NODES} compare={{ pos, onChange: setPos }} label={t('designLab.change.preview.labelSlider')} />;
  }
  return (
    <div className="cx-toggle">
      <GamePreview
        {...common}
        image={which === 'before' ? 'base' : 'proposed'}
        alignTo="proposed"
        nodes={which === 'before' ? BEFORE_NODES : AFTER_NODES}
        label={t(which === 'before' ? 'designLab.change.preview.labelBefore' : 'designLab.change.preview.labelAfter')}
      />
      <div className="cx-switch">
        <SegmentedControl
          legend={t('designLab.change.preview.show')}
          value={which}
          onChange={setWhich}
          options={[
            { value: 'after', label: t('designLab.change.compare.after') },
            { value: 'before', label: t('designLab.change.compare.before') },
          ]}
        />
      </div>
    </div>
  );
}

// ----- The decision -----------------------------------------------------------------------

/**
 * One main action and one secondary action. When something stands in the way, one sentence says
 * what, and the main action hands the change back to the agent instead.
 */
export function Decision({ ctl }: { ctl: CardControl }) {
  const { t } = useTranslation();
  if (ctl.outcome) {
    return (
      <p className="cx-outcome" role="status">
        {t(`designLab.change.outcome.${ctl.outcome}`)}
      </p>
    );
  }
  const { trouble } = ctl;
  return (
    <div className="cx-decision">
      {trouble ? <p className="cx-trouble">{t(`designLab.change.trouble.${trouble}`)}</p> : null}
      <div className="button-row">
        {trouble === 'stale' || trouble === 'failed' ? (
          <Button variant="primary" onClick={ctl.sendBack}>
            {t(`designLab.change.actions.${trouble === 'stale' ? 'redo' : 'fix'}`)}
          </Button>
        ) : (
          <Button variant="primary" onClick={ctl.approve}>
            {t(trouble === 'unchecked' ? 'designLab.change.actions.approveAnyway' : 'designLab.change.actions.approve')}
          </Button>
        )}
        <Button onClick={ctl.reject}>{t('designLab.change.actions.reject')}</Button>
      </div>
    </div>
  );
}

// ----- Details: files, checks, counts, limits ----------------------------------------------

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

function DetailsBody({ card }: { card: ChangeCard }) {
  const { t } = useTranslation();
  const { added, changed } = card.summary;
  return (
    <div className="fc-details-body">
      <section aria-labelledby="fc-d-what">
        <h3 id="fc-d-what">{t('designLab.change.sections.summary')}</h3>
        <ul className="fc-limits">
          {added.map((id) => (
            <li key={id}>{t('designLab.change.summary.added', { name: labelOf(id) })}</li>
          ))}
          {changed.map((change) => (
            <li key={change.id}>
              {t('designLab.change.summary.changed', {
                name: labelOf(change.id),
                what: change.fields.map((f) => t('designLab.change.summary.field', { name: f.name, before: f.before, after: f.after })).join(', '),
              })}
            </li>
          ))}
        </ul>
        <p>{t('designLab.change.by', { who: t(`designLab.change.actor.${card.actor.kind}`) })}</p>
      </section>
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
          {t(card.restorePoint ? 'designLab.change.restore.label' : 'designLab.change.restore.none')}
        </p>
      </section>
    </div>
  );
}

/** Everything a modder does not need to decide. One toggle, closed by default. */
export function ReviewDetails({ card }: { card: ChangeCard }) {
  const { t } = useTranslation();
  return (
    <details className="fc-details">
      <summary>{t('designLab.details')}</summary>
      <DetailsBody card={card} />
    </details>
  );
}

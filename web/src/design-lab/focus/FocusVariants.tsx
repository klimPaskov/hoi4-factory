// The focus editor (D-61, chosen 2026-11-14). The game render fills the page. The modder asks the
// agent for a change in plain words and approves or rejects what comes back, drags a focus to
// move it, and clicks one to type its title, description and cost. There are no layout numbers,
// no internal names and no lists. Everything a modder rarely needs sits behind the one Details toggle.
import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Field } from '../../components/ui';
import type { Diagnostic, FocusNode } from '../../focus/types';
import type { VariantProps } from '../rounds';
import { GamePreview } from './GamePreview';
import { DiagnosticsList, labelOf, SourceLink } from './Panels';
import { useFocusModel, type FocusModel, type FocusScenario } from './useFocusModel';
import './focus.css';

// ----- The agent (faked in the lab) -------------------------------------------------------

interface Agent {
  /** The last request, shown back to the modder. */
  asked: string;
  /** The agent's own one-sentence account of what it did. A real proposal carries it. */
  summary: string;
  working: boolean;
  ask: (text: string) => void;
}

/** In the lab every request is answered, after a short wait, with the sample proposal. */
function useAgent(model: FocusModel, scenario: FocusScenario, sample: string, summary: string): Agent {
  const [asked, setAsked] = useState(scenario === 'pending' || scenario === 'working' ? sample : '');
  const [working, setWorking] = useState(scenario === 'working');
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  return {
    asked,
    summary,
    working,
    ask: (text) => {
      setAsked(text);
      setWorking(true);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => {
        model.propose();
        setWorking(false);
      }, 1500);
    },
  };
}

interface Editor {
  model: FocusModel;
  agent: Agent;
  /** Nothing can be asked or changed while the agent works or the tree reloads. */
  busy: boolean;
}

function useEditor(state: string): Editor {
  const { t } = useTranslation();
  const scenario = state as FocusScenario;
  const model = useFocusModel(scenario);
  const agent = useAgent(model, scenario, t('designLab.focus.sampleRequest'), t('designLab.focus.sampleSummary'));
  return { model, agent, busy: agent.working || model.refreshing };
}

// ----- What the agent changed, in plain sentences ------------------------------------------

function summaryText(t: (key: string, options?: Record<string, unknown>) => string, model: FocusModel): string {
  const added = model.nodes.filter((n) => n.pending?.change === 'added').length;
  const changed = model.pendingCount - added;
  if (added > 0 && changed > 0) return t('designLab.focus.summary.both', { added: t('designLab.focus.summary.added', { count: added }), changed: t('designLab.focus.summary.changed', { count: changed }) });
  if (added > 0) return t('designLab.focus.summary.only', { what: t('designLab.focus.summary.added', { count: added }) });
  return t('designLab.focus.summary.only', { what: t('designLab.focus.summary.changed', { count: changed }) });
}

/** The first problem a modder can ask the agent to fix on this focus. */
function fixable(model: FocusModel, id: string): Diagnostic | undefined {
  return model.diagnostics.find((d) => d.focusId === id && d.severity !== 'info');
}

/** Room kept free under the fitted tree for the bar that floats over it, in pixels. */
const DOCK_ROOM = 172;

// ----- Shared parts ------------------------------------------------------------------------

/** True while the bar floats over the tree. On a narrow screen it sits below instead. */
function useWide(): boolean {
  const query = '(min-width: 48.01rem)';
  const [wide, setWide] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const media = window.matchMedia(query);
    const on = () => setWide(media.matches);
    media.addEventListener('change', on);
    return () => media.removeEventListener('change', on);
  }, []);
  return wide;
}

function Tree({ editor, under = 0 }: { editor: Editor; under?: number }) {
  const { t } = useTranslation();
  const { model, busy } = editor;
  return (
    <GamePreview
      plain
      fitInset={under}
      image={model.render}
      nodes={model.nodes}
      diagnostics={model.diagnostics.filter((d) => d.severity !== 'info')}
      selectedId={model.selectedId}
      onSelect={model.select}
      redrawing={model.redrawing}
      readOnly={busy}
      onMove={model.move}
      revealTick={model.revealTick}
      revealId={model.selectedId}
      label={t('designLab.focus.previewLabel')}
      className="mx-tree"
    />
  );
}

/** The page title and the one Details toggle. Details hold what a modder rarely needs. */
function Header({ editor }: { editor: Editor }) {
  const { t } = useTranslation();
  const { model } = editor;
  const [open, setOpen] = useState(false);
  const id = useId();
  const node = model.selected;
  return (
    <header className="mx-head">
      <h1 className="fx-page-title">{t('designLab.focus.pageTitle')}</h1>
      <Button size="sm" variant="ghost" aria-expanded={open} aria-controls={id} onClick={() => setOpen(!open)}>
        {t('designLab.details')}
      </Button>
      {open ? (
        <section id={id} className="mx-details" aria-label={t('designLab.details')}>
          {node ? (
            <div>
              <h2>{node.label}</h2>
              <dl className="mx-facts">
                <dt>{t('designLab.focus.fields.requires')}</dt>
                <dd>{node.prerequisiteGroups.map((g) => g.map((p) => labelOf(model, p)).join(` ${t('designLab.focus.or')} `)).join(', ') || t('designLab.focus.none')}</dd>
                <dt>{t('designLab.focus.fields.excludes')}</dt>
                <dd>{node.mutuallyExclusive.map((p) => labelOf(model, p)).join(', ') || t('designLab.focus.none')}</dd>
              </dl>
              <SourceLink path={node.source.path} line={node.source.line} />
            </div>
          ) : null}
          <div>
            <h2>{t('designLab.focus.problems')}</h2>
            <DiagnosticsList model={model} />
          </div>
        </section>
      ) : null}
    </header>
  );
}

/** What the agent did and the two actions: approve it or reject it. */
function Verdict({ editor }: { editor: Editor }) {
  const { t } = useTranslation();
  const { model, agent, busy } = editor;
  if (model.pendingCount === 0) return null;
  return (
    <div className="mx-verdict" role="region" aria-label={t('designLab.focus.pending.region')}>
      <p>{agent.summary || summaryText(t, model)}</p>
      <div className="button-row">
        <Button size="sm" variant="primary" onClick={model.applyAll} disabled={busy}>
          {t('designLab.focus.approve')}
        </Button>
        <Button size="sm" onClick={model.discardAll} disabled={busy}>
          {t('designLab.focus.reject')}
        </Button>
      </div>
    </div>
  );
}

/** One line that says what is happening, or nothing. */
function Status({ editor }: { editor: Editor }) {
  const { t } = useTranslation();
  const { model, agent } = editor;
  const text = agent.working ? t('designLab.focus.working') : model.refreshing ? t('designLab.focus.refreshing') : model.redrawing ? t('designLab.focus.render.redrawing') : '';
  return (
    <p className="mx-status" role="status">
      {text ? (
        <>
          <span className="spinner" aria-hidden="true" /> {text}
        </>
      ) : null}
    </p>
  );
}

/** The request box. A selected focus becomes the subject of the request. */
function Ask({ editor }: { editor: Editor }) {
  const { t } = useTranslation();
  const { model, agent, busy } = editor;
  const [text, setText] = useState('');
  const id = useId();
  const node = model.selected;
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const request = text.trim();
    if (!request || busy) return;
    agent.ask(node ? t('designLab.focus.ask.about', { name: node.label, request }) : request);
    setText('');
  };
  return (
    <form className="mx-ask" onSubmit={submit}>
      <label htmlFor={id} className="sr-only">
        {t('designLab.focus.ask.label')}
      </label>
      <input
        id={id}
        type="text"
        dir="auto"
        value={text}
        disabled={busy}
        placeholder={node ? t('designLab.focus.ask.placeholderFor', { name: node.label }) : t('designLab.focus.ask.placeholder')}
        onChange={(event) => setText(event.target.value)}
      />
      <Button type="submit" variant={model.pendingCount === 0 ? 'primary' : 'secondary'} disabled={busy || text.trim() === ''}>
        {t('designLab.focus.ask.send')}
      </Button>
    </form>
  );
}

// ----- The card of the selected focus -----------------------------------------------------

function CostField({ model, node, disabled }: { model: FocusModel; node: FocusNode; disabled: boolean }) {
  const { t } = useTranslation();
  const id = useId();
  return (
    <div className="field mx-cost">
      <label htmlFor={id}>{t('designLab.focus.fields.cost')}</label>
      <input id={id} type="number" min={0} step={1} value={node.cost ?? ''} disabled={disabled} onChange={(event) => event.target.value !== '' && model.setCost(node.id, Math.round(Number(event.target.value)))} />
    </div>
  );
}

function FocusCard({ editor }: { editor: Editor }) {
  const { t } = useTranslation();
  const { model, agent, busy } = editor;
  const node = model.selected;
  if (!node) return null;
  const text = model.texts[node.id] ?? { title: '', description: '' };
  const problem = fixable(model, node.id);
  const disabled = busy;
  return (
    <aside className="mx-card" aria-label={t('designLab.focus.inspector.title')}>
      <Button size="sm" variant="ghost" className="mx-card-close" onClick={() => model.select(null)}>
        {t('designLab.focus.inspector.close')}
      </Button>
      {!node.editable ? <p className="mx-card-note">{t('designLab.focus.lockedNote', { name: node.label, reason: node.notEditableReason })}</p> : null}
      <fieldset className="fx-fieldset" disabled={disabled}>
        <legend className="sr-only">{t('designLab.focus.fields.legend')}</legend>
        <Field label={t('designLab.focus.fields.title')} value={text.title} onChange={(value) => model.setText(node.id, 'title', value)} />
        <Field label={t('designLab.focus.fields.description')} value={text.description} multiline rows={3} onChange={(value) => model.setText(node.id, 'description', value)} />
        <CostField model={model} node={node} disabled={disabled} />
      </fieldset>
      {problem ? (
        <div className="mx-card-problem">
          <p>{problem.message}</p>
          <Button size="sm" disabled={busy} onClick={() => agent.ask(t('designLab.focus.ask.fix', { problem: problem.message }))}>
            {t('designLab.focus.fix')}
          </Button>
        </div>
      ) : null}
    </aside>
  );
}

/** The tree, the card of the selected focus, and one bar: what the agent did, approve or reject, and the request box. */
export function FocusEditor({ state }: VariantProps) {
  const editor = useEditor(state);
  const wide = useWide();
  return (
    <div className="mx-page">
      <Header editor={editor} />
      <div className="mx-stage">
        <Tree editor={editor} under={wide ? DOCK_ROOM : 0} />
        <FocusCard editor={editor} />
        <div className="mx-dock">
          <Status editor={editor} />
          <Verdict editor={editor} />
          <Ask editor={editor} />
        </div>
      </div>
    </div>
  );
}

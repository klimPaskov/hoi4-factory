// Three minimal layouts of the focus editor (D-61). In each the game render fills the page, the
// modder asks the agent for a change in plain words and approves or rejects what comes back.
// There are no layout numbers, no internal names and no lists: a focus moves by dragging it on
// the tree or by asking. Everything a modder rarely needs sits behind the one Details toggle.
//   A. Only a prompt bar over the tree. No fields at all.
//   B. The prompt bar, plus a small card with the title, description and cost of the selected focus.
//   C. A side column that lists what the agent changed, with the prompt under it.
import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Field } from '../../components/ui';
import type { Diagnostic, FocusNode } from '../../focus/types';
import type { VariantProps } from '../rounds';
import { PENDING_BEFORE } from './fixtures';
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

function changeText(t: (key: string, options?: Record<string, unknown>) => string, node: FocusNode): string {
  if (node.pending?.change === 'added') return t('designLab.focus.change.added', { name: node.label });
  const before = PENDING_BEFORE[node.id];
  if (before?.cost !== undefined) return t('designLab.focus.change.cost', { name: node.label, before: before.cost, after: node.cost ?? 0 });
  return t('designLab.focus.change.changed', { name: node.label });
}

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
function Verdict({ editor, children }: { editor: Editor; children?: ReactNode }) {
  const { t } = useTranslation();
  const { model, agent, busy } = editor;
  if (model.pendingCount === 0) return null;
  return (
    <div className="mx-verdict" role="region" aria-label={t('designLab.focus.pending.region')}>
      {children ?? <p>{agent.summary || summaryText(t, model)}</p>}
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

/** What there is to say about the selected focus: the agent's change, a problem to fix, or why it is locked. */
function SelectedNote({ editor }: { editor: Editor }) {
  const { t } = useTranslation();
  const { model, agent, busy } = editor;
  const node = model.selected;
  if (!node) return null;
  const problem = fixable(model, node.id);
  const text = node.pending ? changeText(t, node) : problem ? problem.message : !node.editable ? t('designLab.focus.lockedNote', { name: node.label, reason: node.notEditableReason }) : null;
  return (
    <div className="mx-selected">
      <p>{text ?? <strong>{node.label}</strong>}</p>
      {problem && !node.pending ? (
        <Button size="sm" disabled={busy} onClick={() => agent.ask(t('designLab.focus.ask.fix', { problem: problem.message }))}>
          {t('designLab.focus.fix')}
        </Button>
      ) : null}
      <Button size="sm" variant="ghost" aria-label={t('designLab.focus.deselect', { name: node.label })} onClick={() => model.select(null)}>
        <span aria-hidden="true">&times;</span>
      </Button>
    </div>
  );
}

// ----- A. Only a prompt bar ------------------------------------------------------------------

/** A. The tree and one bar: what the agent did, approve or reject, and the request box. No fields. */
export function FocusA({ state }: VariantProps) {
  const editor = useEditor(state);
  const wide = useWide();
  return (
    <div className="mx-page mxa">
      <Header editor={editor} />
      <div className="mx-stage">
        <Tree editor={editor} under={wide ? DOCK_ROOM : 0} />
        <div className="mx-dock">
          <Status editor={editor} />
          <Verdict editor={editor} />
          <SelectedNote editor={editor} />
          <Ask editor={editor} />
        </div>
      </div>
    </div>
  );
}

// ----- B. Prompt bar and a card for the selected focus ---------------------------------------

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

/** B. As A, and selecting a focus opens a small card with its title, description and cost. */
export function FocusB({ state }: VariantProps) {
  const editor = useEditor(state);
  const wide = useWide();
  return (
    <div className="mx-page mxb">
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

// ----- C. A side column with the change list -------------------------------------------------

function ChangeList({ editor }: { editor: Editor }) {
  const { t } = useTranslation();
  const { model } = editor;
  const pending = model.nodes.filter((n) => n.pending);
  return (
    <ul className="mx-changes" aria-label={t('designLab.focus.pending.region')}>
      {pending.map((node) => (
        <li key={node.id}>
          <button type="button" className={`mx-change${node.id === model.selectedId ? ' is-selected' : ''}`} aria-pressed={node.id === model.selectedId} onClick={() => model.reveal(node.id)}>
            <span className={`mx-change-pin is-${node.pending?.change ?? 'changed'}`} aria-hidden="true">
              {node.pending?.change === 'added' ? '+' : ''}
            </span>
            {changeText(t, node)}
          </button>
        </li>
      ))}
    </ul>
  );
}

/** C. The tree beside one narrow column: your request, the list of what the agent changed, approve or reject, and the request box. */
export function FocusC({ state }: VariantProps) {
  const { t } = useTranslation();
  const editor = useEditor(state);
  const { model, agent } = editor;
  const idle = model.pendingCount === 0 && !agent.working && !model.refreshing;
  return (
    <div className="mx-page mxc">
      <Header editor={editor} />
      <div className="mxc-grid">
        <Tree editor={editor} />
        <aside className="mxc-side" aria-label={t('designLab.focus.agentColumn')}>
          <div className="mxc-thread">
            {agent.asked && !idle ? (
              <p className="mxc-asked">
                <span className="sr-only">{t('designLab.focus.youAsked')} </span>
                {agent.asked}
              </p>
            ) : null}
            {idle ? <p className="muted">{t('designLab.focus.idleHint')}</p> : null}
            <Status editor={editor} />
            {model.pendingCount > 0 ? (
              <>
                <ChangeList editor={editor} />
                <Verdict editor={editor}>
                  <p className="sr-only">{agent.summary || summaryText(t, model)}</p>
                </Verdict>
              </>
            ) : null}
          </div>
          <SelectedNote editor={editor} />
          <Ask editor={editor} />
        </aside>
      </div>
    </div>
  );
}

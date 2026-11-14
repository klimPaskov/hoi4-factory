// The focus editor (S04, D-61 and D-63). The game render fills the page. The modder asks the agent
// for a change in plain words and approves or rejects what comes back, drags a focus to move it,
// and clicks one to change its title, description and cost. There are no layout numbers, no
// internal names and no lists. What a modder rarely needs sits behind the one Details toggle.
import { useId, useState, type FormEvent, type ReactNode } from 'react';
import { Link, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import { Button, ErrorMessage, Loading, NotAvailableYet } from '../components/ui';
import { GamePreview } from '../focus/GamePreview';
import { useFocusSession, type FocusSession } from '../focus/session';
import type { Diagnostic, FocusNode } from '../focus/types';
import { labelOf, namesSentence, useWide } from '../focus/words';
import { usePageTitle } from '../hooks';
import '../focus/focus.css';

export function FocusEditorPage() {
  const { id = '', taskId = '', treeId = '' } = useParams();
  return <Editor key={`${id}/${taskId}/${treeId}`} projectId={id} taskId={taskId} treeId={treeId} />;
}

/** Room kept free under the fitted tree for the bar that floats over it, in pixels. */
const BAR_ROOM = 172;

/** The first problem a modder can ask the agent to fix on this focus. */
function fixable(diagnostics: Diagnostic[], id: string): Diagnostic | undefined {
  return diagnostics.find((d) => d.focusId === id && d.severity !== 'info');
}

function Editor({ projectId, taskId, treeId }: { projectId: string; taskId: string; treeId: string }) {
  const { t } = useTranslation();
  usePageTitle(t('focus.title'));
  const session = useFocusSession(projectId, taskId, treeId);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const wide = useWide();
  const back = (
    <p className="breadcrumb">
      <Link to={`/projects/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(taskId)}`}>{t('focus.backToTask')}</Link>
    </p>
  );
  if (session.status === 'loading') {
    return (
      <>
        {back}
        <Loading label={t('focus.loading')} />
      </>
    );
  }
  if (session.status === 'error' || !session.projection) {
    return (
      <>
        {back}
        {session.error instanceof ApiError && session.error.code === 'not_found' ? (
          <NotAvailableYet what={t('focus.title')} />
        ) : (
          <>
            <h1>{t('focus.errorTitle')}</h1>
            <ErrorMessage error={session.error} onRetry={session.reload} />
          </>
        )}
      </>
    );
  }
  const selected = session.nodes.find((n) => n.id === selectedId);
  const frozen = session.busy || session.working || !session.editable;
  return (
    <div className="fx-page">
      <Header session={session} selected={selected} back={back} />
      <div className="fx-stage">
        {session.picture ? (
          <GamePreview
            picture={session.picture}
            nodes={session.nodes}
            diagnostics={session.projection.diagnostics}
            selectedId={selectedId}
            onSelect={setSelectedId}
            readOnly={frozen}
            redrawing={session.redrawing}
            onMove={session.move}
            fitInset={wide ? BAR_ROOM : 0}
            label={t('focus.pictureLabel')}
            className="fx-tree"
          />
        ) : session.pictureError ? (
          <div className="fx-nopicture">
            <ErrorMessage error={session.pictureError} onRetry={session.retryPicture} />
          </div>
        ) : (
          <div className="fx-nopicture">
            <Loading label={t('focus.drawing')} />
          </div>
        )}
        {selected ? <FocusCard key={selected.id} session={session} node={selected} frozen={frozen} onClose={() => setSelectedId(null)} /> : null}
        <div className="fx-dock">
          <Status session={session} />
          {session.failure ? <ErrorMessage error={session.failure} /> : null}
          <Verdict session={session} />
          <Ask session={session} selected={selected} />
        </div>
      </div>
    </div>
  );
}

/** The page title and the one Details toggle. Details hold what a modder rarely needs. */
function Header({ session, selected, back }: { session: FocusSession; selected: FocusNode | undefined; back: ReactNode }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const id = useId();
  const diagnostics = session.projection?.diagnostics ?? [];
  const name = (focusId: string) => labelOf(session.nodes, focusId);
  return (
    <header className="fx-head">
      <div>
        {back}
        <h1 className="fx-title">{t('focus.title')}</h1>
      </div>
      <Button size="sm" variant="ghost" aria-expanded={open} aria-controls={id} onClick={() => setOpen(!open)}>
        {t('focus.details')}
      </Button>
      {open ? (
        <section id={id} className="fx-details" aria-label={t('focus.details')}>
          {selected ? (
            <div>
              <h2>{selected.label}</h2>
              <dl className="fx-facts">
                <dt>{t('focus.requires')}</dt>
                <dd>{selected.prerequisiteGroups.map((g) => g.map(name).join(` ${t('focus.or')} `)).join(', ') || t('focus.none')}</dd>
                <dt>{t('focus.excludes')}</dt>
                <dd>{selected.mutuallyExclusive.map(name).join(', ') || t('focus.none')}</dd>
                <dt>{t('focus.source')}</dt>
                <dd dir="ltr">{t('focus.sourceAt', { path: selected.source.path, line: selected.source.line })}</dd>
              </dl>
            </div>
          ) : null}
          <div>
            <h2>{t('focus.problems')}</h2>
            {diagnostics.length === 0 ? (
              <p className="muted">{t('focus.noProblems')}</p>
            ) : (
              <ul className="fx-diags">
                {diagnostics.map((d) => (
                  <li key={`${d.code}:${d.focusId ?? ''}:${d.source?.line ?? 0}`}>
                    <span className={`fx-dot is-${d.severity}`} aria-hidden="true" />
                    <span dir="auto">{d.message}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>
      ) : null}
    </header>
  );
}

/** One line that says what is happening, or nothing. */
function Status({ session }: { session: FocusSession }) {
  const { t } = useTranslation();
  const text = session.working ? t('focus.working') : session.busy ? t('focus.saving') : session.redrawing ? t('focus.redrawing') : '';
  return (
    <p className="fx-status" role="status">
      {text ? (
        <>
          <span className="spinner" aria-hidden="true" /> {text}
        </>
      ) : null}
    </p>
  );
}

/**
 * What changed and the two actions. The agent's changes are approved or rejected. A person's own
 * edits are saved or undone. When something stands in the way, one sentence says what.
 */
function Verdict({ session }: { session: FocusSession }) {
  const { t } = useTranslation();
  if (session.open.length === 0 || session.working) return null;
  const sentence = session.agentSentence ?? namesSentence(t, session.open, session.nodes);
  const words = session.fromAgent ? 'agent' : 'own';
  const disabled = session.busy || !session.editable;
  return (
    <div className="fx-verdict" role="region" aria-label={t('focus.changesRegion')}>
      <div>
        <p dir="auto">{sentence}</p>
        {session.trouble ? <p className="fx-trouble">{t(`focus.trouble.${session.trouble}`)}</p> : null}
      </div>
      <div className="button-row">
        {session.trouble === 'failed' || session.trouble === 'stale' ? null : (
          <Button size="sm" variant="primary" disabled={disabled} onClick={() => session.approve(sentence, session.trouble === 'unchecked')}>
            {session.trouble === 'unchecked' ? t('focus.approveAnyway') : t(`focus.${words}.approve`)}
          </Button>
        )}
        <Button size="sm" disabled={disabled} onClick={session.reject}>
          {t(`focus.${words}.reject`)}
        </Button>
      </div>
    </div>
  );
}

/** The request box. A selected focus becomes the subject of the request. */
function Ask({ session, selected }: { session: FocusSession; selected: FocusNode | undefined }) {
  const { t } = useTranslation();
  const [text, setText] = useState('');
  const id = useId();
  const disabled = session.busy || session.working || !session.editable;
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const request = text.trim();
    if (!request || disabled) return;
    session.ask(selected ? t('focus.ask.about', { name: selected.label, request }) : request);
    setText('');
  };
  return (
    <form className="fx-ask" onSubmit={submit}>
      <label htmlFor={id} className="sr-only">
        {t('focus.ask.label')}
      </label>
      <input
        id={id}
        type="text"
        dir="auto"
        maxLength={2000}
        value={text}
        disabled={disabled}
        placeholder={selected ? t('focus.ask.placeholderFor', { name: selected.label }) : t('focus.ask.placeholder')}
        onChange={(event) => setText(event.target.value)}
      />
      <Button type="submit" variant={session.open.length === 0 ? 'primary' : 'secondary'} disabled={disabled || text.trim() === ''}>
        {t('focus.ask.send')}
      </Button>
    </form>
  );
}

/** The small card of the selected focus: its title, description and cost, and a problem the agent can fix. */
function FocusCard({ session, node, frozen, onClose }: { session: FocusSession; node: FocusNode; frozen: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const ids = useId();
  const [title, setTitle] = useState(node.label);
  const [description, setDescription] = useState(node.description ?? '');
  const [cost, setCost] = useState(node.cost === null ? '' : String(node.cost));
  const canText = (session.projection?.supportedOperations ?? []).includes('set_text');
  const problem = fixable(session.projection?.diagnostics ?? [], node.id);
  const saveCost = () => {
    const value = Number(cost);
    if (frozen) return;
    if (cost.trim() === '' || !Number.isFinite(value) || value < 0 || value === node.cost) return;
    session.setCost(node.id, value);
  };
  const saveText = () => {
    if (frozen) return;
    const next = { title: title.trim(), description: description.trim() };
    if (!next.title || (next.title === node.label && next.description === (node.description ?? ''))) return;
    session.setText(node.id, next.title, next.description);
  };
  return (
    <aside className="fx-card" aria-label={t('focus.card.label', { name: node.label })}>
      <Button size="sm" variant="ghost" className="fx-card-close" onClick={onClose}>
        {t('focus.card.close')}
      </Button>
      {!node.editable ? <p className="fx-card-note">{t('focus.card.cantDrag', { name: node.label })}</p> : null}
      <div className="field">
        <label htmlFor={`${ids}-title`}>{t('focus.card.title')}</label>
        <input id={`${ids}-title`} type="text" dir="auto" maxLength={400} value={title} disabled={frozen || !canText} onChange={(event) => setTitle(event.target.value)} onBlur={saveText} />
      </div>
      {canText ? (
        <div className="field">
          <label htmlFor={`${ids}-description`}>{t('focus.card.description')}</label>
          <textarea id={`${ids}-description`} dir="auto" rows={3} maxLength={400} value={description} disabled={frozen} onChange={(event) => setDescription(event.target.value)} onBlur={saveText} />
        </div>
      ) : null}
      <div className="field fx-cost">
        <label htmlFor={`${ids}-cost`}>{t('focus.card.cost')}</label>
        <input
          id={`${ids}-cost`}
          type="number"
          min={0}
          step={1}
          value={cost}
          disabled={frozen || !node.editable}
          onChange={(event) => setCost(event.target.value)}
          onBlur={saveCost}
          onKeyDown={(event) => {
            if (event.key === 'Enter') saveCost();
          }}
        />
      </div>
      {problem ? (
        <div className="fx-card-problem">
          <p dir="auto">{problem.message}</p>
          <Button size="sm" disabled={frozen} onClick={() => session.ask(t('focus.ask.fix', { problem: problem.message }))}>
            {t('focus.card.fix')}
          </Button>
        </div>
      ) : null}
    </aside>
  );
}

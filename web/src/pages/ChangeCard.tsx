// The change card (S04, D-61 and D-63): the review of one change before it is saved. The game
// picture of the result fills the page, with one plain sentence, one main and one secondary
// action. The diff, the checks, the limits and the restore point sit behind the one Details toggle.
import { useState, type ReactNode } from 'react';
import { Link, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import {
  commitProposals,
  discardProposal,
  getProjection,
  getProposal,
  listPublications,
  newIdempotencyKey,
  type Proposal,
  type Publication,
} from '../api/focus';
import { askAgent, getTask, type TaskDetail } from '../api/tasks';
import { Badge, Button, ErrorMessage, Loading, NotAvailableYet, type Tone } from '../components/ui';
import { GamePreview } from '../focus/GamePreview';
import { agentSentence, troubleOf, usePicture, withPending, type Trouble } from '../focus/session';
import type { FocusProjection } from '../focus/types';
import { costLines, labelOf, namesSentence, useWide } from '../focus/words';
import { useAsync, usePageTitle } from '../hooks';
import '../focus/focus.css';

export function ChangeCardPage() {
  const { id = '', taskId = '', proposalId = '' } = useParams();
  return <Card key={`${id}/${taskId}/${proposalId}`} projectId={id} taskId={taskId} proposalId={proposalId} />;
}

interface Loaded {
  proposal: Proposal;
  detail: TaskDetail;
  projection: FocusProjection;
  publications: Publication[];
}

/** Room kept free under the fitted picture for the bar that floats over it, in pixels. */
const BAR_ROOM = 150;

function Card({ projectId, taskId, proposalId }: { projectId: string; taskId: string; proposalId: string }) {
  const { t } = useTranslation();
  usePageTitle(t('change.title'));
  const state = useAsync<Loaded>(async (signal) => {
    const [proposal, detail, publications] = await Promise.all([getProposal(projectId, taskId, proposalId, signal), getTask(projectId, taskId, signal), listPublications(projectId, taskId, signal)]);
    const projection = await getProjection(projectId, taskId, proposal.treeId, signal);
    return { proposal, detail, projection, publications };
  });
  const back = (
    <p className="breadcrumb">
      <Link to={`/projects/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(taskId)}`}>{t('focus.backToTask')}</Link>
    </p>
  );
  if (state.status === 'loading') {
    return (
      <>
        {back}
        <Loading label={t('change.loading')} />
      </>
    );
  }
  if (state.status === 'error') {
    return (
      <>
        {back}
        {state.error instanceof ApiError && state.error.code === 'not_found' ? (
          <NotAvailableYet what={t('change.title')} />
        ) : (
          <>
            <h1>{t('change.errorTitle')}</h1>
            <ErrorMessage error={state.error} onRetry={state.reload} />
          </>
        )}
      </>
    );
  }
  return <Review projectId={projectId} taskId={taskId} loaded={state.data} back={back} onLoaded={state.replace} />;
}

type Outcome = 'approved' | 'rejected' | 'sentBack';

function Review({ projectId, taskId, loaded, back, onLoaded }: { projectId: string; taskId: string; loaded: Loaded; back: ReactNode; onLoaded: (data: Loaded) => void }) {
  const { t } = useTranslation();
  const { proposal, detail, projection } = loaded;
  const wide = useWide();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);
  const [trouble, setTrouble] = useState<Trouble | null>(null);
  const [sentBack, setSentBack] = useState(false);
  const pic = usePicture(projectId, taskId, proposal.treeId, projection.projectionHash);
  const open = proposal.state === 'applied';
  const nodes = withPending(projection.nodes, open ? [proposal] : []);
  const sentence = agentSentence(detail, [proposal]) ?? namesSentence(t, [proposal], nodes);
  const outcome: Outcome | null = sentBack ? 'sentBack' : proposal.state === 'committed' ? 'approved' : proposal.state === 'applied' ? null : 'rejected';

  async function act(action: () => Promise<void>) {
    setBusy(true);
    setFailure(null);
    setTrouble(null);
    try {
      await action();
    } catch (cause) {
      const found = troubleOf(cause);
      if (found) setTrouble(found);
      else setFailure(cause);
    }
    try {
      // The tree is read again too: a rejected change is gone from the picture.
      const [fresh, publications, tree] = await Promise.all([getProposal(projectId, taskId, proposal.id), listPublications(projectId, taskId), getProjection(projectId, taskId, proposal.treeId)]);
      onLoaded({ ...loaded, proposal: fresh, publications, projection: tree });
    } catch (cause) {
      setFailure((current: unknown) => current ?? cause);
    }
    setBusy(false);
  }

  const approve = (anyway: boolean) =>
    void act(async () => {
      await commitProposals(projectId, taskId, [{ id: proposal.id, proposal_hash: proposal.proposalHash }], sentence, newIdempotencyKey('commit'), anyway);
    });
  const reject = () => void act(() => discardProposal(projectId, taskId, proposal.id).then(() => undefined));
  const handBack = (instruction: string) =>
    void act(async () => {
      await askAgent(projectId, taskId, detail.task.version, instruction);
      setSentBack(true);
    });

  return (
    <div className="fx-page fx-grow">
      <header className="fx-head">
        <div>
          {back}
          <h1 className="fx-title">{t('change.title')}</h1>
        </div>
      </header>
      <div className="fx-stage">
        {pic.picture ? (
          <GamePreview picture={pic.picture} nodes={nodes} selectedId={selectedId} onSelect={setSelectedId} readOnly redrawing={pic.redrawing} fitInset={wide ? BAR_ROOM : 0} label={t('change.pictureLabel')} className="fx-tree" />
        ) : pic.error ? (
          <div className="fx-nopicture">
            <ErrorMessage error={pic.error} onRetry={pic.retry} />
          </div>
        ) : (
          <div className="fx-nopicture">
            <Loading label={t('focus.drawing')} />
          </div>
        )}
        <div className="fx-dock fx-dock-row">
          <div className="fx-summary">
            <p dir="auto">{sentence}</p>
            {costLines(t, [proposal], nodes).map((line) => (
              <p key={line} className="fx-fine">
                {line}
              </p>
            ))}
          </div>
          {outcome ? (
            <p className="fx-outcome" role="status">
              {t(`change.outcome.${outcome}`)}
            </p>
          ) : (
            <div className="fx-decision">
              {trouble ? <p className="fx-trouble">{t(`focus.trouble.${trouble}`)}</p> : null}
              {failure ? <ErrorMessage error={failure} /> : null}
              <div className="button-row">
                {trouble === 'stale' || trouble === 'failed' ? (
                  <Button variant="primary" disabled={busy} onClick={() => handBack(t(trouble === 'stale' ? 'change.instruction.redo' : 'change.instruction.fix'))}>
                    {t(trouble === 'stale' ? 'change.redo' : 'change.fix')}
                  </Button>
                ) : (
                  <Button variant="primary" disabled={busy} onClick={() => approve(trouble === 'unchecked')}>
                    {trouble === 'unchecked' ? t('focus.approveAnyway') : t('focus.agent.approve')}
                  </Button>
                )}
                <Button disabled={busy} onClick={reject}>
                  {t('focus.agent.reject')}
                </Button>
              </div>
            </div>
          )}
        </div>
      </div>
      <Details loaded={loaded} names={(id) => labelOf(nodes, id)} />
    </div>
  );
}

const VERDICT_TONE: Record<string, Tone> = { pass: 'success', fail: 'danger', incomplete: 'warning' };

/** Everything a modder does not need to decide. One toggle, closed by default. */
function Details({ loaded, names }: { loaded: Loaded; names: (id: string) => string }) {
  const { t } = useTranslation();
  const { proposal, projection, publications } = loaded;
  const check = proposal.engineReport;
  const delta = proposal.findingDelta;
  const commit = publications.find((x) => x.kind === 'commit' && x.state === 'succeeded' && JSON.stringify(x.intent.proposals ?? []).includes(proposal.id));
  const restorable = Boolean(commit?.result?.restore_point);
  return (
    <details className="fc-details">
      <summary>{t('focus.details')}</summary>
      <div className="fc-details-body">
        <section aria-labelledby="fc-d-what">
          <h2 id="fc-d-what">{t('change.sections.what')}</h2>
          <ul className="fc-plain">
            {proposal.summary.added.map((id) => (
              <li key={id}>{t('change.added', { name: names(id) })}</li>
            ))}
            {proposal.summary.changed.map((change) => (
              <li key={change.id}>{t('change.changed', { name: names(change.id), what: change.fields.map((f) => t('change.field', { name: f.name, before: f.before, after: f.after })).join(', ') })}</li>
            ))}
            {proposal.summary.removed.map((id) => (
              <li key={id}>{t('change.removed', { name: names(id) })}</li>
            ))}
          </ul>
          <p>{t(`change.by.${proposal.actorKind}`)}</p>
        </section>
        <section aria-labelledby="fc-d-files">
          <h2 id="fc-d-files">{t('change.sections.files')}</h2>
          {proposal.diffs.map((file) => (
            <details key={file.path} className="fc-file">
              <summary>
                <span className="mono" dir="ltr">
                  {file.path}
                </span>
              </summary>
              <pre className="fc-pre" dir="ltr">
                <code>
                  {file.diff.split('\n').map((line, i) => (
                    <span key={i} className={`fc-line ${line.startsWith('+++') || line.startsWith('---') ? 'fc-meta' : line.startsWith('@@') ? 'fc-hunk' : line.startsWith('+') ? 'fc-add' : line.startsWith('-') ? 'fc-del' : ''}`}>
                      {line || ' '}
                    </span>
                  ))}
                </code>
              </pre>
            </details>
          ))}
        </section>
        <section aria-labelledby="fc-d-checks">
          <h2 id="fc-d-checks">{t('change.sections.checks')}</h2>
          {check ? (
            <>
              <p>
                <Badge tone={VERDICT_TONE[check.verdict] ?? 'neutral'}>{t(`change.verdict.${check.verdict}`)}</Badge> {t('change.checkFindings', { fresh: check.new_findings, known: check.baselined_findings })}
              </p>
              <ul className="fc-plain">
                {check.blocking.map((b) => (
                  <li key={`${b.code}:${b.path}:${b.line}`} dir="auto">
                    {b.message}
                  </li>
                ))}
              </ul>
              <p>{t('change.references', { resolved: check.unknown.references_resolved, unresolved: check.unknown.references_unresolved, unknown: check.unknown.references_unknown })}</p>
            </>
          ) : (
            <p>{t('change.checkNotRun')}</p>
          )}
          {delta ? <p>{t('change.delta', { introduced: delta.introduced, worsened: delta.worsened, improved: delta.improved, resolved: delta.resolved })}</p> : null}
          <p>{t(`change.layout.${projection.layoutVerdict}`)}</p>
        </section>
        <section aria-labelledby="fc-d-limits">
          <h2 id="fc-d-limits">{t('change.sections.limits')}</h2>
          <ul className="fc-plain">
            <li>{t('change.limits.render')}</li>
            <li>{t('change.limits.game')}</li>
          </ul>
          <p>{t(restorable ? 'change.restore.saved' : proposal.state === 'committed' ? 'change.restore.none' : 'change.restore.later')}</p>
        </section>
      </div>
    </details>
  );
}

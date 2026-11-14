// What the focus editor and the change card know about one tree of one task, and what they can
// do to it. Every change goes to the server as a focus operation and comes back as a proposal:
// nothing is kept only in the page. A person's gesture and the agent's edit share that path.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ApiError, isAbort } from '../api/client';
import {
  applyOperations,
  commitProposals,
  discardProposal,
  getPreviewImage,
  getPreviewMap,
  getProjection,
  listProposals,
  newIdempotencyKey,
  type FocusOp,
  type Proposal,
} from '../api/focus';
import { ACTIVE_ATTEMPT_STATES, askAgent, getTask, type TaskDetail } from '../api/tasks';
import type { Picture } from './GamePreview';
import type { FocusNode, FocusProjection } from './types';

/** What stands in the way of saving, in the order the server reports it. */
export type Trouble = 'stale' | 'failed' | 'unchecked';

/** The error codes of the commit route that mean one of the troubles. */
export function troubleOf(error: unknown): Trouble | null {
  if (!(error instanceof ApiError)) return null;
  if (error.code === 'check_incomplete') return 'unchecked';
  if (error.code === 'commit_blocked') return 'failed';
  if (error.code === 'proposal_changed' || error.code === 'proposal_not_open' || error.code === 'stale_base') return 'stale';
  return null;
}

/** Marks the focuses that open proposals added or changed, so the picture can outline them. */
export function withPending(nodes: FocusNode[], open: Proposal[]): FocusNode[] {
  const marks = new Map<string, NonNullable<FocusNode['pending']>>();
  for (const p of open) {
    for (const id of p.changedIds) {
      if (!marks.has(id)) marks.set(id, { proposalId: p.id, actor: p.actorKind, change: 'changed' });
    }
    for (const change of p.summary.changed) {
      if (!marks.has(change.id)) marks.set(change.id, { proposalId: p.id, actor: p.actorKind, change: 'changed' });
    }
    for (const id of p.summary.added) marks.set(id, { proposalId: p.id, actor: p.actorKind, change: 'added' });
  }
  return nodes.map((node) => ({ ...node, pending: marks.get(node.id) }));
}

/** The agent's own sentence about the open changes, when the agent made them and said one. */
export function agentSentence(detail: TaskDetail | null, open: Proposal[]): string | null {
  const attempts = new Set(open.filter((p) => p.actorKind !== 'person').map((p) => p.attemptId));
  const said = (detail?.attempts ?? [])
    .filter((a) => attempts.has(a.id) && a.outcome?.summary)
    .sort((a, b) => b.attempt_no - a.attempt_no)[0];
  return said?.outcome?.summary ?? null;
}

/** Loads the picture of the tree whenever its projection changes, and frees the old one. */
export function usePicture(projectId: string, taskId: string, treeId: string, hash: string | null) {
  const [state, setState] = useState<{ hash: string; picture: Picture } | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  const url = useRef<string | null>(null);
  useEffect(() => {
    if (!hash) return;
    const controller = new AbortController();
    Promise.all([getPreviewMap(projectId, taskId, treeId, controller.signal), getPreviewImage(projectId, taskId, treeId, controller.signal)]).then(
      ([map, blob]) => {
        if (controller.signal.aborted) return;
        const next = URL.createObjectURL(blob);
        if (url.current) URL.revokeObjectURL(url.current);
        url.current = next;
        setError(null);
        setState({ hash, picture: { url: next, map } });
      },
      (cause: unknown) => {
        if (!controller.signal.aborted && !isAbort(cause)) setError(cause);
      },
    );
    return () => controller.abort();
  }, [projectId, taskId, treeId, hash, attempt]);
  useEffect(
    () => () => {
      if (url.current) URL.revokeObjectURL(url.current);
    },
    [],
  );
  return {
    picture: state?.picture ?? null,
    /** The picture on screen is older than the tree. */
    redrawing: hash !== null && state !== null && state.hash !== hash && error === null,
    error,
    retry: useCallback(() => {
      setError(null);
      setAttempt((n) => n + 1);
    }, []),
  };
}

interface Loaded {
  projection: FocusProjection;
  proposals: Proposal[];
  detail: TaskDetail;
}

export interface FocusSession {
  status: 'loading' | 'error' | 'ready';
  /** Why the tree could not be loaded. */
  error: unknown;
  reload: () => void;
  projection: FocusProjection | null;
  /** The focuses, with what the open changes added or changed marked. */
  nodes: FocusNode[];
  picture: Picture | null;
  pictureError: unknown;
  retryPicture: () => void;
  redrawing: boolean;
  /** Changes that are written to the task but not saved yet, oldest first. */
  open: Proposal[];
  /** True when the agent made at least one of the open changes. */
  fromAgent: boolean;
  agentSentence: string | null;
  /** The agent is running on this task now. */
  working: boolean;
  /** A request of this page is on its way. */
  busy: boolean;
  /** The last thing that went wrong with an action, to show beside the buttons. */
  failure: unknown;
  trouble: Trouble | null;
  /** The task may be edited by this person. */
  editable: boolean;
  move: (id: string, x: number, y: number) => void;
  setCost: (id: string, cost: number) => void;
  /** Sets the title, the description or both. Leave out what stays as it is. */
  setText: (id: string, text: { title?: string; description?: string }) => void;
  /** Saves every open change. `anyway` accepts a check that could not finish. */
  approve: (message: string, anyway: boolean) => void;
  reject: () => void;
  ask: (request: string) => void;
}

const POLL_MS = 3000;

export function useFocusSession(projectId: string, taskId: string, treeId: string): FocusSession {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [round, setRound] = useState(0);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);
  const [trouble, setTrouble] = useState<Trouble | null>(null);

  const read = useCallback(
    async (signal?: AbortSignal): Promise<Loaded> => {
      const [projection, proposals, detail] = await Promise.all([
        getProjection(projectId, taskId, treeId, signal),
        listProposals(projectId, taskId, signal),
        getTask(projectId, taskId, signal),
      ]);
      return { projection, proposals, detail };
    },
    [projectId, taskId, treeId],
  );

  useEffect(() => {
    const controller = new AbortController();
    read(controller.signal).then(
      (data) => {
        if (!controller.signal.aborted) setLoaded(data);
      },
      (cause: unknown) => {
        if (!controller.signal.aborted && !isAbort(cause)) setError(cause);
      },
    );
    return () => controller.abort();
  }, [read, round]);

  /** Reads everything again without the loading state, so the page does not jump. */
  const refresh = useCallback(async () => {
    try {
      setLoaded(await read());
    } catch (cause) {
      setFailure((current: unknown) => current ?? cause);
    }
  }, [read]);

  const working = (loaded?.detail.attempts ?? []).some((a) => ACTIVE_ATTEMPT_STATES.includes(a.state));
  // While the agent runs, look again every few seconds. When it stops, its changes are read in.
  useEffect(() => {
    if (!working) return;
    const timer = window.setInterval(() => void refresh(), POLL_MS);
    return () => window.clearInterval(timer);
  }, [working, refresh]);

  const hash = loaded?.projection.projectionHash ?? null;
  const pic = usePicture(projectId, taskId, treeId, hash);
  const open = useMemo(() => (loaded?.proposals ?? []).filter((p) => p.state === 'applied' && p.treeId === treeId), [loaded, treeId]);
  const nodes = useMemo(() => withPending(loaded?.projection.nodes ?? [], open), [loaded, open]);

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
      // A refusal means the page was out of date, so read the tree again.
      await refresh();
    }
    setBusy(false);
  }

  function operate(operations: FocusOp[]) {
    if (!loaded) return;
    void act(async () => {
      const done = await applyOperations(projectId, taskId, treeId, loaded.projection.projectionHash, operations);
      setLoaded((current) => (current ? { ...current, projection: done.projection, proposals: [...current.proposals, done.proposal] } : current));
    });
  }

  const task = loaded?.detail.task;
  return {
    status: loaded ? 'ready' : error ? 'error' : 'loading',
    error,
    reload: () => {
      setError(null);
      setLoaded(null);
      setRound((n) => n + 1);
    },
    projection: loaded?.projection ?? null,
    nodes,
    picture: pic.picture,
    pictureError: pic.error,
    retryPicture: pic.retry,
    redrawing: pic.redrawing,
    open,
    fromAgent: open.some((p) => p.actorKind !== 'person'),
    agentSentence: agentSentence(loaded?.detail ?? null, open),
    working,
    busy,
    failure,
    trouble,
    editable: task !== undefined && task.mode === 'edit' && task.state !== 'completed' && task.state !== 'cancelled',
    move: (id, x, y) => operate([{ op: 'move', focus_id: id, x, y }]),
    setCost: (id, cost) => operate([{ op: 'set_cost', focus_id: id, cost }]),
    setText: (id, text) => operate([{ op: 'set_text', focus_id: id, ...text }]),
    approve: (message, anyway) =>
      void act(async () => {
        await commitProposals(
          projectId,
          taskId,
          open.map((p) => ({ id: p.id, proposal_hash: p.proposalHash })),
          message,
          newIdempotencyKey('commit'),
          anyway,
        );
        await refresh();
      }),
    reject: () =>
      void act(async () => {
        // Newest first, so each change is put back on top of the one before it.
        for (const p of [...open].reverse()) await discardProposal(projectId, taskId, p.id);
        await refresh();
      }),
    ask: (request) =>
      void act(async () => {
        if (!task) return;
        await askAgent(projectId, taskId, task.version, request);
        await refresh();
      }),
  };
}

// Local state behind the focus editor variants. Gestures change only this state: in the lab no
// request leaves the page. A real page would turn each action into one focus_rewrite request.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { Diagnostic, FocusNode, LayoutVerdict } from '../../focus/types';
import { DIAGNOSTICS, FOCUS_TEXT, NODES, PENDING_BEFORE, PROJECTION, type FocusText } from './fixtures';

/** Which of the two real renders the preview shows. */
export type RenderName = 'base' | 'proposed';

export type FocusScenario = 'pending' | 'settled' | 'refreshing' | 'readonly';

export type Notice = { key: 'self' | 'duplicate' | 'cycle' | 'readonly' | 'added' | 'linked'; id?: string; other?: string } | null;

export interface FocusModel {
  nodes: FocusNode[];
  diagnostics: Diagnostic[];
  layoutVerdict: LayoutVerdict;
  /** The source changed under the editor (a scenario): editing is paused until the redraw lands. */
  refreshing: boolean;
  /** A render is being redrawn, after the source changed or after an edit. The old picture stays, dimmed. */
  redrawing: boolean;
  /** The render the preview shows now. It swaps to the other one when a redraw ends. */
  render: RenderName;
  selectedId: string | null;
  selected: FocusNode | undefined;
  select: (id: string | null) => void;
  texts: Record<string, FocusText>;
  query: string;
  setQuery: (query: string) => void;
  matches: FocusNode[];
  /** Counts up whenever the preview should bring the selected focus into view. */
  revealTick: number;
  reveal: (id: string) => void;
  notice: Notice;
  move: (id: string, x: number, y: number) => void;
  link: (prerequisite: string, target: string, group?: number) => void;
  unlink: (target: string, prerequisite: string) => void;
  setCost: (id: string, cost: number | null) => void;
  setText: (id: string, field: keyof FocusText, value: string) => void;
  addFocus: () => void;
  apply: (id: string) => void;
  discard: (id: string) => void;
  applyAll: () => void;
  discardAll: () => void;
  pendingCount: number;
}

const settle = (node: FocusNode): FocusNode => ({ ...node, pending: undefined });

function reaches(nodes: FocusNode[], from: string, to: string, seen = new Set<string>()): boolean {
  if (from === to) return true;
  if (seen.has(from)) return false;
  seen.add(from);
  const node = nodes.find((n) => n.id === from);
  return (node?.prerequisiteGroups.flat() ?? []).some((id) => reaches(nodes, id, to, seen));
}

export function matchesQuery(node: FocusNode, query: string): boolean {
  const q = query.trim().toLowerCase();
  return !q || node.id.toLowerCase().includes(q) || node.label.toLowerCase().includes(q);
}

export function useFocusModel(scenario: FocusScenario): FocusModel {
  const [nodes, setNodes] = useState<FocusNode[]>(() => (scenario === 'settled' || scenario === 'readonly' ? NODES.map(settle) : NODES));
  const [texts, setTexts] = useState<Record<string, FocusText>>(FOCUS_TEXT);
  const [selectedId, setSelectedId] = useState<string | null>(scenario === 'readonly' ? 'SWE_coastal_battery' : null);
  const [query, setQuery] = useState('');
  const [revealTick, setRevealTick] = useState(0);
  const [notice, setNotice] = useState<Notice>(null);
  const [busy, setBusy] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const wants: RenderName = nodes.some((n) => n.id === 'SWE_nordic_trade_pact') ? 'proposed' : 'base';
  const [render, setRender] = useState<RenderName>(wants);

  // A real page asks the render tool again after every source change and swaps the picture in
  // when it arrives. The lab fakes the wait.
  const redraw = useCallback(() => {
    setBusy(true);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setBusy(false), 1100);
  }, []);
  useEffect(() => {
    if (wants === render) return;
    const id = window.setTimeout(() => setRender(wants), 1000);
    return () => window.clearTimeout(id);
  }, [wants, render]);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const update = (id: string, change: (node: FocusNode) => FocusNode) =>
    setNodes((list) => list.map((n) => (n.id === id && n.editable ? change(n) : n)));
  const writable = (id: string): boolean => {
    const ok = nodes.find((n) => n.id === id)?.editable ?? false;
    if (!ok) setNotice({ key: 'readonly', id });
    return ok;
  };

  const diagnostics = useMemo(() => DIAGNOSTICS.filter((d) => !d.focusId || nodes.some((n) => n.id === d.focusId)), [nodes]);
  const matches = useMemo(() => nodes.filter((n) => matchesQuery(n, query)), [nodes, query]);

  return {
    nodes,
    diagnostics,
    layoutVerdict: PROJECTION.layoutVerdict,
    refreshing: scenario === 'refreshing',
    redrawing: scenario === 'refreshing' || busy || wants !== render,
    render,
    selectedId,
    selected: nodes.find((n) => n.id === selectedId),
    select: (id) => {
      setSelectedId(id);
      setNotice(null);
    },
    texts,
    query,
    setQuery,
    matches,
    revealTick,
    reveal: (id) => {
      setSelectedId(id);
      setRevealTick((n) => n + 1);
    },
    notice,
    move: (id, x, y) => {
      if (!writable(id)) return;
      update(id, (n) => ({ ...n, x, y, declared: { mode: 'absolute', x, y } }));
      redraw();
    },
    link: (prerequisite, target, group) => {
      const targetNode = nodes.find((n) => n.id === target);
      if (!writable(target)) return;
      if (prerequisite === target) return setNotice({ key: 'self', id: target });
      if (targetNode?.prerequisiteGroups.flat().includes(prerequisite)) return setNotice({ key: 'duplicate', id: target, other: prerequisite });
      if (reaches(nodes, prerequisite, target)) return setNotice({ key: 'cycle', id: target, other: prerequisite });
      update(target, (n) => {
        const groups = n.prerequisiteGroups.map((g) => [...g]);
        const slot = group === undefined ? undefined : groups[group];
        if (slot) slot.push(prerequisite);
        else groups.push([prerequisite]);
        return { ...n, prerequisiteGroups: groups };
      });
      setNotice({ key: 'linked', id: target, other: prerequisite });
      redraw();
    },
    unlink: (target, prerequisite) => {
      update(target, (n) => ({
        ...n,
        prerequisiteGroups: n.prerequisiteGroups.map((g) => g.filter((id) => id !== prerequisite)).filter((g) => g.length > 0),
      }));
      redraw();
    },
    setCost: (id, cost) => {
      update(id, (n) => ({ ...n, cost }));
      redraw();
    },
    setText: (id, field, value) => {
      if (!writable(id)) return;
      setTexts((all) => ({ ...all, [id]: { title: '', description: '', ...all[id], [field]: value } }));
      if (field === 'title') update(id, (n) => ({ ...n, label: value.trim() || n.id }));
    },
    addFocus: () => {
      let n = nodes.length + 1;
      while (nodes.some((node) => node.id === `SWE_new_focus_${n}`)) n += 1;
      const id = `SWE_new_focus_${n}`;
      const bottom = Math.max(...nodes.map((node) => node.y));
      const added: FocusNode = {
        id,
        label: id,
        titleKey: id,
        descriptionKey: `${id}_desc`,
        cost: 10,
        icon: null,
        x: 0,
        y: bottom + 1,
        declared: { mode: 'absolute', x: 0, y: bottom + 1 },
        prerequisiteGroups: [],
        mutuallyExclusive: [],
        source: { path: PROJECTION.sourcePath, line: 230 + n, column: 1 },
        editable: true,
        pending: { proposalId: 'prop_local', actor: 'person', change: 'added' },
      };
      setNodes((list) => [...list, added]);
      setSelectedId(id);
      setRevealTick((t) => t + 1);
      setNotice({ key: 'added', id });
      redraw();
    },
    apply: (id) => setNodes((list) => list.map((node) => (node.id === id ? settle(node) : node))),
    discard: (id) => {
      const node = nodes.find((n) => n.id === id);
      if (!node?.pending) return;
      if (node.pending.change === 'added') {
        setNodes((list) =>
          list
            .filter((n) => n.id !== id)
            .map((n) => ({
              ...n,
              prerequisiteGroups: n.prerequisiteGroups.map((g) => g.filter((p) => p !== id)).filter((g) => g.length > 0),
            })),
        );
        if (selectedId === id) setSelectedId(null);
      } else {
        setNodes((list) => list.map((n) => (n.id === id ? settle({ ...n, ...PENDING_BEFORE[id] }) : n)));
      }
    },
    applyAll: () => setNodes((list) => list.map(settle)),
    discardAll: () => {
      const gone = new Set(nodes.filter((n) => n.pending?.change === 'added').map((n) => n.id));
      setNodes((list) =>
        list
          .filter((n) => !gone.has(n.id))
          .map((n) =>
            settle({
              ...n,
              ...(n.pending ? PENDING_BEFORE[n.id] : {}),
              prerequisiteGroups: n.prerequisiteGroups.map((g) => g.filter((p) => !gone.has(p))).filter((g) => g.length > 0),
            }),
          ),
      );
      if (selectedId && gone.has(selectedId)) setSelectedId(null);
    },
    pendingCount: nodes.filter((n) => n.pending).length,
  };
}

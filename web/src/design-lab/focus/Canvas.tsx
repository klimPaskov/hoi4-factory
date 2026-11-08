// The focus tree canvas, built on @xyflow/react. Nodes sit on the HOI4 grid, prerequisites draw
// as connectors (solid for a single requirement, dashed for an OR group) and mutual exclusions
// as a red line with a bar at each end. View zoom and pan never change game coordinates.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Background,
  BackgroundVariant,
  Handle,
  Panel,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Connection,
  type Edge,
  type Node,
  type NodeChange,
  type NodeProps,
} from '@xyflow/react';
import '@xyflow/react/dist/base.css';
import { useTranslation } from 'react-i18next';
import type { FocusNode } from '../../focus/types';
import { GRID_H, GRID_W } from './fixtures';
import { matchesQuery, type FocusModel } from './useFocusModel';
import './focus.css';

interface NodeData extends Record<string, unknown> {
  focus: FocusNode;
  dim: boolean;
  worst: 'error' | 'warning' | 'info' | null;
  problems: number;
  readOnly: boolean;
  onApply: (id: string) => void;
  onDiscard: (id: string) => void;
}

type FocusFlowNode = Node<NodeData, 'focus'>;

const MARKER = 'fx-exclusion-marker';

function FocusNodeView({ data, selected }: NodeProps<FocusFlowNode>) {
  const { t } = useTranslation();
  const { focus, dim, worst, problems, readOnly } = data;
  const classes = [
    'fx-node',
    selected ? 'is-selected' : '',
    dim ? 'is-dim' : '',
    focus.pending ? `is-pending is-${focus.pending.change}` : '',
    focus.editable ? '' : 'is-locked',
  ]
    .filter(Boolean)
    .join(' ');
  return (
    <div className={classes}>
      <Handle id="in" type="target" position={Position.Top} isConnectable={focus.editable && !readOnly} />
      <Handle id="ex-l" type="target" position={Position.Left} isConnectable={false} className="fx-side-handle" />
      <Handle id="ex-r" type="source" position={Position.Right} isConnectable={false} className="fx-side-handle" />
      {focus.pending ? <span className="fx-tag fx-tag-pending">{t(`designLab.focus.node.${focus.pending.change}`)}</span> : null}
      {!focus.editable ? <span className="fx-tag fx-tag-locked">{t('designLab.focus.node.locked')}</span> : null}
      <div className="fx-node-head">
        <span className={`fx-icon${focus.icon?.resolved ? '' : ' is-missing'}`} aria-hidden="true">
          {focus.icon?.resolved ? 'G' : '?'}
        </span>
        <span className="fx-cost" aria-hidden="true">
          {focus.cost ?? '-'}
        </span>
        {worst ? (
          <span className={`fx-problem fx-problem-${worst}`} aria-hidden="true">
            {worst === 'info' ? 'i' : '!'}
          </span>
        ) : null}
      </div>
      <p className="fx-title">{focus.label}</p>
      {focus.pending && !readOnly ? (
        <div className="fx-actions nodrag nopan">
          <button type="button" className="fx-mini fx-mini-apply" aria-label={t('designLab.focus.pending.applyOne', { name: focus.label })} onClick={() => data.onApply(focus.id)}>
            <span aria-hidden="true">&#10003;</span>
          </button>
          <button type="button" className="fx-mini fx-mini-discard" aria-label={t('designLab.focus.pending.discardOne', { name: focus.label })} onClick={() => data.onDiscard(focus.id)}>
            <span aria-hidden="true">&#10005;</span>
          </button>
        </div>
      ) : null}
      <Handle id="out" type="source" position={Position.Bottom} isConnectable={focus.editable && !readOnly} />
      {problems > 0 ? <span className="sr-only">{t('designLab.focus.node.problems', { count: problems })}</span> : null}
    </div>
  );
}

const NODE_TYPES = { focus: FocusNodeView };

function ViewControls() {
  const { t } = useTranslation();
  const flow = useReactFlow();
  return (
    <Panel position="top-left" className="fx-view">
      <div role="group" aria-label={t('designLab.focus.view.label')}>
        <button type="button" className="fx-view-button" onClick={() => void flow.zoomIn({ duration: 150 })} aria-label={t('designLab.focus.view.zoomIn')}>
          <span aria-hidden="true">+</span>
        </button>
        <button type="button" className="fx-view-button" onClick={() => void flow.zoomOut({ duration: 150 })} aria-label={t('designLab.focus.view.zoomOut')}>
          <span aria-hidden="true">&minus;</span>
        </button>
        <button type="button" className="fx-view-button fx-view-fit" onClick={() => void flow.fitView({ duration: 200, padding: 0.12 })}>
          {t('designLab.focus.view.fit')}
        </button>
      </div>
    </Panel>
  );
}

/** Brings the selected node into view when the model asks for it (search, add focus, list). */
function Reveal({ tick, id }: { tick: number; id: string | null }) {
  const flow = useReactFlow();
  const handled = useRef(tick);
  useEffect(() => {
    if (tick === handled.current) return;
    handled.current = tick;
    if (id) void flow.fitView({ nodes: [{ id }], duration: 250, maxZoom: 1.1, padding: 1.2 });
  }, [tick, id, flow]);
  return null;
}

export interface FocusCanvasProps {
  model: FocusModel;
  /** No dragging, linking or per-node apply and discard (the change card preview). */
  readOnly?: boolean;
  /** A node was clicked or activated. */
  onActivate?: (node: FocusNode) => void;
  label: string;
  className?: string;
}

function Inner({ model, readOnly = false, onActivate, label, className }: FocusCanvasProps) {
  const { t } = useTranslation();
  const [drag, setDrag] = useState<Record<string, { x: number; y: number }>>({});
  const frozen = readOnly || model.refreshing;
  const ariaLabels = useMemo(
    () => ({
      'node.a11yDescription.default': t('designLab.focus.aria.node'),
      'node.a11yDescription.keyboardDisabled': t('designLab.focus.aria.nodeNoKeys'),
      'node.a11yDescription.ariaLiveMessage': ({ direction, x, y }: { direction: string; x: number; y: number }) =>
        t('designLab.focus.aria.moved', { direction, x: Math.round(x / GRID_W), y: Math.round(y / GRID_H) }),
      'edge.a11yDescription.default': t('designLab.focus.aria.edge'),
      'handle.ariaLabel': t('designLab.focus.aria.handle'),
    }),
    [t],
  );

  const worst = useMemo(() => {
    const out: Record<string, { worst: NodeData['worst']; count: number }> = {};
    const rank = { error: 3, warning: 2, info: 1 } as const;
    for (const d of model.diagnostics) {
      if (!d.focusId) continue;
      const current = out[d.focusId];
      const next = current && current.worst && rank[current.worst] >= rank[d.severity] ? current.worst : d.severity;
      out[d.focusId] = { worst: next, count: (current?.count ?? 0) + 1 };
    }
    return out;
  }, [model.diagnostics]);

  const { apply, discard } = model;
  const nodes = useMemo<FocusFlowNode[]>(
    () =>
      model.nodes.map((focus) => {
        const live = drag[focus.id];
        const info = worst[focus.id];
        return {
          id: focus.id,
          type: 'focus',
          position: live ?? { x: focus.x * GRID_W, y: focus.y * GRID_H },
          selected: model.selectedId === focus.id,
          draggable: focus.editable && !frozen,
          ariaLabel: t('designLab.focus.node.aria', { name: focus.label, id: focus.id, cost: focus.cost ?? 0 }),
          data: {
            focus,
            dim: !matchesQuery(focus, model.query),
            worst: info?.worst ?? null,
            problems: info?.count ?? 0,
            readOnly: frozen,
            onApply: apply,
            onDiscard: discard,
          },
        };
      }),
    [model.nodes, model.selectedId, model.query, drag, worst, frozen, apply, discard, t],
  );

  const edges = useMemo<Edge[]>(() => {
    const out: Edge[] = [];
    const active = model.selectedId;
    const done = new Set<string>();
    for (const node of model.nodes) {
      for (const group of node.prerequisiteGroups) {
        for (const from of group) {
          out.push({
            id: `req:${from}:${node.id}`,
            source: from,
            sourceHandle: 'out',
            targetHandle: 'in',
            target: node.id,
            type: 'smoothstep',
            className: `fx-edge ${group.length > 1 ? 'fx-or' : 'fx-and'}${active && (active === from || active === node.id) ? ' is-active' : ''}`,
            focusable: false,
          });
        }
      }
      for (const other of node.mutuallyExclusive) {
        const key = [node.id, other].sort().join(':');
        if (done.has(key)) continue;
        done.add(key);
        const peer = model.nodes.find((n) => n.id === other);
        if (!peer) continue;
        const [left, right] = node.x <= peer.x ? [node, peer] : [peer, node];
        out.push({
          id: `excl:${key}`,
          source: left.id,
          sourceHandle: 'ex-r',
          target: right.id,
          targetHandle: 'ex-l',
          type: 'straight',
          className: 'fx-edge fx-excl',
          markerStart: MARKER,
          markerEnd: MARKER,
          focusable: false,
        });
      }
    }
    return out;
  }, [model.nodes, model.selectedId]);

  const commit = useCallback(
    (id: string, x: number, y: number) => {
      model.move(id, Math.round(x / GRID_W), Math.round(y / GRID_H));
      setDrag((all) => {
        const next = { ...all };
        delete next[id];
        return next;
      });
    },
    [model],
  );

  const onNodesChange = useCallback(
    (changes: NodeChange<FocusFlowNode>[]) => {
      for (const change of changes) {
        if (change.type !== 'position' || !change.position) continue;
        const { id, position } = change;
        if (change.dragging) setDrag((all) => ({ ...all, [id]: position }));
        else commit(id, position.x, position.y);
      }
    },
    [commit],
  );

  const onConnect = useCallback((c: Connection) => model.link(c.source, c.target), [model]);

  return (
    <div className={`fx-canvas${className ? ` ${className}` : ''}`} role="region" aria-label={label} aria-busy={model.refreshing || undefined}>
      <svg className="fx-defs" aria-hidden="true" focusable="false">
        <defs>
          <marker id={MARKER} viewBox="-6 -6 12 12" markerWidth="12" markerHeight="12" refX="0" refY="0" orient="auto" markerUnits="userSpaceOnUse">
            <path className="fx-marker" d="M0 -6 V6" />
          </marker>
        </defs>
      </svg>
      <ReactFlow<FocusFlowNode>
        nodes={nodes}
        edges={edges}
        nodeTypes={NODE_TYPES}
        onNodesChange={onNodesChange}
        onNodeDragStop={(_event, node) => commit(node.id, node.position.x, node.position.y)}
        onConnect={onConnect}
        onNodeClick={(_event, node) => {
          model.select(node.id);
          onActivate?.(node.data.focus);
        }}
        onPaneClick={() => model.select(null)}
        nodesConnectable={!frozen}
        ariaLabelConfig={ariaLabels}
        edgesFocusable={false}
        snapToGrid
        snapGrid={[GRID_W, GRID_H]}
        minZoom={0.3}
        maxZoom={1.6}
        fitView
        fitViewOptions={{ padding: 0.12 }}
        deleteKeyCode={null}
        selectionKeyCode={null}
        multiSelectionKeyCode={null}
        zoomOnDoubleClick={false}
      >
        <Background variant={BackgroundVariant.Dots} gap={[GRID_W, GRID_H]} size={2} color="var(--border-strong)" />
        <ViewControls />
        <Reveal tick={model.revealTick} id={model.selectedId} />
      </ReactFlow>
    </div>
  );
}

export function FocusCanvas(props: FocusCanvasProps) {
  return (
    <ReactFlowProvider>
      <Inner {...props} />
    </ReactFlowProvider>
  );
}

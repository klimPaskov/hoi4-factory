/* eslint-disable jsx-a11y/no-noninteractive-element-interactions, jsx-a11y/no-noninteractive-tabindex -- the pan and zoom surface takes pointer drags and the keys + - 0. Every action on it also has a button, and each focus is a real button inside it. */
// The live game preview (D-50): the focus tree as the game draws it, from the Agent Tools render
// path (`focus_render` with `view: "game"`). The picture is a real render, never a schematic. The
// overlay is positioned from the render's hover map, so a click on a drawn focus selects it, a
// drag moves it on the grid, and badges and markers sit exactly on the drawn focuses. In a real
// page the picture is swapped for a fresh render whenever the source changes. The lab fakes that
// wait with the Redrawing state.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '../../components/ui';
import type { Diagnostic, FocusNode } from '../../focus/types';
import baseMap from './render/game-base.json';
import baseUrl from './render/game-base.webp';
import proposedMap from './render/game-proposed.json';
import proposedUrl from './render/game-proposed.webp';
import { matchesQuery, type RenderName } from './useFocusModel';
import './gamepreview.css';

interface HoverMap {
  layout: { nodes: { id: string; x: number; y: number }[] };
  render: { width: number; height: number; focuses: { id: string; x: number; y: number; width: number; height: number }[] };
}

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

interface Geometry {
  url: string;
  width: number;
  height: number;
  boxes: Map<string, Box>;
  cells: Map<string, { x: number; y: number }>;
  /** The pixel of grid cell (0, 0): left edge and bottom edge of a focus, and one grid step. */
  grid: { ox: number; oy: number; sx: number; sy: number };
}

/** Least squares line through (cell, pixel) pairs, so one odd focus (a missing icon) cannot skew the grid. */
function fitLine(points: [number, number][]): { offset: number; step: number } {
  const n = points.length;
  const mx = points.reduce((a, p) => a + p[0], 0) / n;
  const my = points.reduce((a, p) => a + p[1], 0) / n;
  const num = points.reduce((a, p) => a + (p[0] - mx) * (p[1] - my), 0);
  const den = points.reduce((a, p) => a + (p[0] - mx) ** 2, 0) || 1;
  const step = num / den;
  return { offset: my - step * mx, step };
}

function geometry(map: HoverMap, url: string): Geometry {
  const boxes = new Map<string, Box>();
  const cells = new Map<string, { x: number; y: number }>();
  for (const f of map.render.focuses) boxes.set(f.id, { x: f.x, y: f.y, w: f.width, h: f.height });
  for (const n of map.layout.nodes) cells.set(n.id, { x: n.x, y: n.y });
  const xs: [number, number][] = [];
  const ys: [number, number][] = [];
  for (const [id, box] of boxes) {
    const cell = cells.get(id);
    if (!cell) continue;
    xs.push([cell.x, box.x]);
    ys.push([cell.y, box.y + box.h]);
  }
  const fx = fitLine(xs);
  const fy = fitLine(ys);
  return { url, width: map.render.width, height: map.render.height, boxes, cells, grid: { ox: fx.offset, oy: fy.offset, sx: fx.step, sy: fy.step } };
}

export const GEOMETRY: Record<RenderName, Geometry> = {
  base: geometry(baseMap as HoverMap, baseUrl),
  proposed: geometry(proposedMap as HoverMap, proposedUrl),
};

const DEFAULT_BOX = { w: 164, h: 108 };

function boxAt(geo: Geometry, id: string, x: number, y: number): Box {
  const drawn = geo.boxes.get(id);
  const size = drawn ?? { w: DEFAULT_BOX.w, h: DEFAULT_BOX.h };
  return { x: geo.grid.ox + geo.grid.sx * x, y: geo.grid.oy + geo.grid.sy * y - size.h, w: size.w, h: size.h };
}

interface View {
  z: number;
  x: number;
  y: number;
}

const MIN_Z = 0.2;
const MAX_Z = 3;
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

export interface GamePreviewProps {
  /** The render to show. */
  image: RenderName;
  /** Focuses to overlay: pending changes, selection, markers. Focuses the picture does not draw get a dashed placeholder. */
  nodes: FocusNode[];
  diagnostics?: Diagnostic[];
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  /** Dim every focus that does not match this search text. */
  query?: string;
  /** No moving and no per-focus apply and discard (the change card). */
  readOnly?: boolean;
  /** Dims the picture and says that a fresh render is on its way. */
  redrawing?: boolean;
  onMove?: (id: string, x: number, y: number) => void;
  onApply?: (id: string) => void;
  onDiscard?: (id: string) => void;
  /** Counts up to ask for the focus `revealId` to be brought into view. */
  revealTick?: number;
  revealId?: string | null;
  /** Only the picture and its markers: no grid numbers while dragging, no bar below and no note. The page says those things itself. */
  plain?: boolean;
  /** Pixels at the bottom of the frame that something else covers. The fitted picture stays above them. */
  fitInset?: number;
  label: string;
  className?: string;
}

interface Ghost {
  id: string;
  x: number;
  y: number;
}

export function GamePreview(props: GamePreviewProps) {
  const { image, nodes, diagnostics = [], selectedId, onSelect, query = '', readOnly = false, redrawing = false, onMove, onApply, onDiscard, revealTick = 0, revealId, plain = false, fitInset = 0, label, className } = props;
  const { t } = useTranslation();
  const geo = GEOMETRY[image];
  const frame = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [free, setFree] = useState<View>({ z: 1, x: 0, y: 0 });
  const [fitted, setFitted] = useState(true);
  const [seenTick, setSeenTick] = useState(revealTick);
  const [ghost, setGhost] = useState<Ghost | null>(null);
  const pan = useRef<{ px: number; py: number; vx: number; vy: number } | null>(null);
  const drag = useRef<{ id: string; sx: number; sy: number; box: Box; active: boolean } | null>(null);

  const fit = useCallback(
    (w: number, h: number): View => {
      const room = Math.max(h - fitInset, h / 2);
      const z = clamp(Math.min(w / geo.width, room / geo.height), MIN_Z, 1.2);
      return { z, x: (w - geo.width * z) / 2, y: (room - geo.height * z) / 2 };
    },
    [geo.width, geo.height, fitInset],
  );

  useLayoutEffect(() => {
    const el = frame.current;
    if (!el) return;
    const measure = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // While fitted, the view follows the frame size. A zoom, pan or reveal turns the fit off.
  const view = fitted && size.w > 0 && size.h > 0 ? fit(size.w, size.h) : free;
  const viewRef = useRef(view);
  useEffect(() => {
    viewRef.current = view;
  });

  const zoomAt = useCallback((factor: number, cx: number, cy: number) => {
    const v = viewRef.current;
    const z = clamp(v.z * factor, MIN_Z, MAX_Z);
    const k = z / v.z;
    setFitted(false);
    setFree({ z, x: cx - (cx - v.x) * k, y: cy - (cy - v.y) * k });
  }, []);

  // Ctrl or Cmd with the wheel zooms at the pointer. A plain wheel keeps scrolling the page.
  useEffect(() => {
    const el = frame.current;
    if (!el) return;
    const onWheel = (event: WheelEvent) => {
      if (!plain && !event.ctrlKey && !event.metaKey) return;
      event.preventDefault();
      const rect = el.getBoundingClientRect();
      zoomAt(event.deltaY < 0 ? 1.15 : 1 / 1.15, event.clientX - rect.left, event.clientY - rect.top);
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, [zoomAt, plain]);

  // Bring the requested focus into view, zoomed to at least a readable size.
  if (revealTick !== seenTick) {
    setSeenTick(revealTick);
    const node = nodes.find((n) => n.id === revealId);
    if (node && size.w > 0) {
      const box = geo.boxes.get(node.id) ?? boxAt(geo, node.id, node.x, node.y);
      const z = clamp(Math.max(view.z, 0.8), MIN_Z, 1.2);
      setFitted(false);
      setFree({ z, x: size.w / 2 - (box.x + box.w / 2) * z, y: size.h / 2 - (box.y + box.h / 2) * z });
    }
  }

  const worst = useMemo(() => {
    const rank = { error: 3, warning: 2, info: 1 } as const;
    const out = new Map<string, { severity: Diagnostic['severity']; count: number }>();
    for (const d of diagnostics) {
      if (!d.focusId) continue;
      const cur = out.get(d.focusId);
      out.set(d.focusId, { severity: cur && rank[cur.severity] >= rank[d.severity] ? cur.severity : d.severity, count: (cur?.count ?? 0) + 1 });
    }
    return out;
  }, [diagnostics]);

  const frozen = readOnly || redrawing;
  const onFramePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest('.gp-hit, .gp-controls')) return;
    pan.current = { px: event.clientX, py: event.clientY, vx: view.x, vy: view.y };
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const onFramePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const p = pan.current;
    if (!p) return;
    setFitted(false);
    setFree({ ...viewRef.current, x: p.vx + event.clientX - p.px, y: p.vy + event.clientY - p.py });
  };
  const endPan = () => {
    pan.current = null;
  };

  const startDrag = (event: PointerEvent<HTMLButtonElement>, node: FocusNode, box: Box) => {
    if (event.button !== 0) return;
    drag.current = { id: node.id, sx: event.clientX, sy: event.clientY, box, active: false };
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const moveDrag = (event: PointerEvent<HTMLButtonElement>, node: FocusNode) => {
    const d = drag.current;
    if (!d || d.id !== node.id || !node.editable || frozen) return;
    if (!d.active && Math.hypot(event.clientX - d.sx, event.clientY - d.sy) < 5) return;
    d.active = true;
    const z = viewRef.current.z;
    const left = d.box.x + (event.clientX - d.sx) / z;
    const bottom = d.box.y + d.box.h + (event.clientY - d.sy) / z;
    setGhost({ id: node.id, x: Math.max(0, Math.round((left - geo.grid.ox) / geo.grid.sx)), y: Math.max(0, Math.round((bottom - geo.grid.oy) / geo.grid.sy)) });
  };
  const endDrag = (node: FocusNode) => {
    const d = drag.current;
    drag.current = null;
    if (d?.active && ghost && ghost.id === node.id && (ghost.x !== node.x || ghost.y !== node.y)) onMove?.(node.id, ghost.x, ghost.y);
    setGhost(null);
  };
  const onNodeKey = (event: KeyboardEvent<HTMLButtonElement>, node: FocusNode) => {
    const step: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    const delta = step[event.key];
    if (!delta) return;
    event.preventDefault();
    if (!node.editable || frozen) return;
    onMove?.(node.id, Math.max(0, node.x + delta[0]), Math.max(0, node.y + delta[1]));
  };
  const onFrameKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    const cx = size.w / 2;
    const cy = size.h / 2;
    if (event.key === '+' || event.key === '=') zoomAt(1.25, cx, cy);
    else if (event.key === '-') zoomAt(1 / 1.25, cx, cy);
    else if (event.key === '0') setFitted(true);
  };

  const selectedNode = nodes.find((n) => n.id === selectedId);
  const dim = (node: FocusNode) => query !== '' && !matchesQuery(node, query);

  return (
    <div className={`gp${className ? ` ${className}` : ''}${redrawing ? ' is-redrawing' : ''}`} aria-busy={redrawing || undefined}>
      <div
        ref={frame}
        className="gp-frame"
        tabIndex={0}
        role="application"
        aria-label={label}
        onPointerDown={onFramePointerDown}
        onPointerMove={onFramePointerMove}
        onPointerUp={endPan}
        onPointerCancel={endPan}
        onKeyDown={onFrameKey}
      >
        <div className="gp-world" style={{ width: geo.width, height: geo.height, transform: `translate(${view.x}px, ${view.y}px) scale(${view.z})`, ['--gp-inv' as string]: String(1 / view.z ** 0.65) }}>
          <img className="gp-img" src={geo.url} width={geo.width} height={geo.height} alt={t('designLab.focus.render.alt')} draggable={false} />
          <div className="gp-overlay">
            {nodes.map((node) => {
              const drawn = geo.boxes.get(node.id);
              const cell = geo.cells.get(node.id);
              const inPlace = drawn && cell && cell.x === node.x && cell.y === node.y;
              const box = inPlace && drawn ? drawn : boxAt(geo, node.id, node.x, node.y);
              const state = !drawn ? 'undrawn' : inPlace ? 'drawn' : 'moved';
              const info = worst.get(node.id);
              const selected = node.id === selectedId;
              const classes = [
                'gp-hit',
                `is-${state}`,
                selected ? 'is-selected' : '',
                dim(node) ? 'is-dim' : '',
                node.pending ? `is-pending is-${node.pending.change}` : '',
                node.editable ? '' : 'is-locked',
                ghost?.id === node.id ? 'is-dragging' : '',
              ]
                .filter(Boolean)
                .join(' ');
              return (
                <div key={node.id} className="gp-slot" style={{ left: box.x, top: box.y, width: box.w, height: box.h }}>
                  <button
                    type="button"
                    className={classes}
                    aria-pressed={selected}
                    aria-label={t(node.pending ? `designLab.focus.node.label_${node.pending.change}` : 'designLab.focus.node.label', { name: node.label, cost: node.cost ?? 0 })}
                    onClick={() => onSelect(node.id)}
                    onPointerDown={(event) => startDrag(event, node, box)}
                    onPointerMove={(event) => moveDrag(event, node)}
                    onPointerUp={() => endDrag(node)}
                    onPointerCancel={() => endDrag(node)}
                    onKeyDown={(event) => onNodeKey(event, node)}
                  />
                  {node.pending ? (
                    <span className={`gp-pin gp-pin-${node.pending.change}`} aria-hidden="true">
                      {node.pending.change === 'added' ? '+' : plain ? '' : '~'}
                    </span>
                  ) : null}
                  {info ? (
                    <span className={`gp-mark gp-mark-${info.severity}`} aria-hidden="true">
                      {info.severity === 'info' ? 'i' : '!'}
                    </span>
                  ) : null}
                </div>
              );
            })}
            {ghost ? (
              <div
                className="gp-ghost"
                aria-hidden="true"
                style={(() => {
                  const b = boxAt(geo, ghost.id, ghost.x, ghost.y);
                  return { left: b.x, top: b.y, width: b.w, height: b.h };
                })()}
              >
                {plain ? null : <span className="gp-ghost-cell">{`${ghost.x}, ${ghost.y}`}</span>}
              </div>
            ) : null}
          </div>
        </div>
        <div className="gp-controls" role="group" aria-label={t('designLab.focus.view.label')}>
          {plain ? null : (
            <>
              <button type="button" onClick={() => zoomAt(1.25, size.w / 2, size.h / 2)} aria-label={t('designLab.focus.view.zoomIn')}>
                <span aria-hidden="true">+</span>
              </button>
              <button type="button" onClick={() => zoomAt(1 / 1.25, size.w / 2, size.h / 2)} aria-label={t('designLab.focus.view.zoomOut')}>
                <span aria-hidden="true">&minus;</span>
              </button>
            </>
          )}
          <button type="button" onClick={() => setFitted(true)}>
            {t('designLab.focus.view.fit')}
          </button>
        </div>
        {redrawing && !plain ? (
          <p className="gp-note" role="status">
            <span className="spinner" aria-hidden="true" /> {t('designLab.focus.render.redrawing')}
          </p>
        ) : null}
      </div>
      {selectedNode?.pending && !readOnly && !plain ? (
        <div className="gp-selbar" role="group" aria-label={t('designLab.focus.pending.region')}>
          <p>{t(`designLab.focus.pending.one_${selectedNode.pending.change}`, { name: selectedNode.label })}</p>
          <Button size="sm" variant="primary" disabled={redrawing} onClick={() => onApply?.(selectedNode.id)}>
            {t('designLab.focus.pending.apply')}
          </Button>
          <Button size="sm" disabled={redrawing} onClick={() => onDiscard?.(selectedNode.id)}>
            {t('designLab.focus.pending.discard')}
          </Button>
        </div>
      ) : null}
    </div>
  );
}

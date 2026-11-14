// The shapes the server produces for the focus editor (stage S04).

export type Verdict = 'pass' | 'incomplete' | 'fail';
export type LayoutVerdict = 'pass' | 'revise' | 'blocked' | 'unknown';
export interface SourceRef {
  path: string;
  line: number;
  column: number;
}
export interface FocusNode {
  id: string;
  label: string; // localised title, or the id when the key is missing
  titleKey: string;
  descriptionKey: string;
  /** The localised description, when the server sends it. */
  description?: string;
  cost: number | null;
  icon: { sprite: string; resolved: boolean } | null;
  x: number; // resolved grid position
  y: number;
  declared: { mode: 'absolute' | 'relative' | 'auto'; x?: number; y?: number; relativeTo?: string };
  prerequisiteGroups: string[][]; // AND of OR-groups
  mutuallyExclusive: string[];
  source: SourceRef;
  editable: boolean;
  notEditableReason?: string;
  pending?: { proposalId: string; actor: 'native_agent' | 'coding_client' | 'person'; change: 'added' | 'changed' };
}
export interface Diagnostic {
  code: string;
  severity: 'error' | 'warning' | 'info';
  message: string;
  focusId?: string;
  source?: SourceRef;
}
export interface FocusProjection {
  projectionHash: string;
  tool: { name: string; version: string };
  treeId: string;
  sourcePath: string;
  sourceHashes: Record<string, string>;
  layoutVerdict: LayoutVerdict;
  nodes: FocusNode[];
  diagnostics: Diagnostic[];
  supportedOperations: ('create' | 'link' | 'move' | 'set_cost' | 'set_text')[];
}

// The shapes the server produces for the focus editor and the change card (stage S04).
// The design lab builds its fixtures on these, so a chosen design needs no data changes.

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
  supportedOperations: ('create' | 'link' | 'move' | 'set_cost')[];
}
export interface CheckRow {
  name: string;
  result: 'passed' | 'failed' | 'unavailable' | 'skipped';
  detail: string;
}
export interface ChangeCard {
  proposalId: string;
  proposalHash: string;
  actor: { kind: 'person' | 'native_agent' | 'coding_client'; name: string };
  operation: string; // for example "hoi4.focus_rewrite"
  summary: {
    added: string[];
    changed: { id: string; fields: { name: string; before: string; after: string }[] }[];
    removed: string[];
  };
  files: { path: string; beforeSha256: string | null; afterSha256: string; unifiedDiff: string }[];
  sourceLinks: { focusId: string; source: SourceRef }[];
  findingDelta: { introduced: number; worsened: number; changed: number; improved: number; unchanged: number; resolved: number };
  closure: { verdict: Verdict; resolved: number; unresolved: number; unknown: number };
  layoutVerdict: LayoutVerdict;
  checks: CheckRow[];
  limits: string[];
  restorePoint: { ref: string; oid: string } | null;
  state: 'pending' | 'approved' | 'committed' | 'discarded' | 'stale';
  publication: { canPush: boolean; reason?: string };
}

// Small helpers the focus editor and the change card share: plain sentences about a change, and
// whether the bar floats over the picture.
import { useEffect, useState } from 'react';
import type { Proposal } from '../api/focus';
import type { FocusNode } from './types';

type Translate = (key: string, options?: Record<string, unknown>) => string;

export const labelOf = (nodes: FocusNode[], id: string): string => nodes.find((n) => n.id === id)?.label ?? id;

/** The names of what the changes touched, for changes that have no sentence from the agent. */
export function namesSentence(t: Translate, proposals: Proposal[], nodes: FocusNode[]): string {
  const added = new Set<string>();
  const changed = new Set<string>();
  for (const p of proposals) {
    p.summary.added.forEach((id) => added.add(id));
    p.summary.changed.forEach((c) => changed.add(c.id));
    p.changedIds.forEach((id) => changed.add(id));
  }
  added.forEach((id) => changed.delete(id));
  const names = (ids: Set<string>) => [...ids].map((id) => labelOf(nodes, id)).join(', ');
  if (added.size > 0 && changed.size > 0) return t('focus.own.both', { added: names(added), changed: names(changed) });
  if (added.size > 0) return t('focus.own.added', { names: names(added) });
  return t('focus.own.changed', { names: names(changed) });
}

/** One line for each cost change, which the picture cannot show. */
export function costLines(t: Translate, proposals: Proposal[], nodes: FocusNode[]): string[] {
  const lines: string[] = [];
  for (const p of proposals) {
    for (const change of p.summary.changed) {
      for (const field of change.fields) {
        if (field.name === 'cost') lines.push(t('focus.costChange', { name: labelOf(nodes, change.id), before: field.before, after: field.after }));
      }
    }
  }
  return lines;
}

/** True while the bar floats over the picture. On a narrow screen it sits below instead. */
export function useWide(): boolean {
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

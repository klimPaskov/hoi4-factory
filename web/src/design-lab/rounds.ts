// The open choices: one round per page, three or four variants each. A round leaves this list
// (and its variant files are deleted) once the maintainer has chosen and the page is built.
import type { ComponentType } from 'react';
import { ChangeA, ChangeB, ChangeC, ChangeD } from './focus/ChangeVariants';
import { FocusA, FocusB, FocusC, FocusD } from './focus/FocusVariants';

export interface VariantProps {
  /** One of the round's `states`, picked in the lab bar. */
  state: string;
}

export interface Round {
  id: string;
  states: string[];
  variants: { id: string; Component: ComponentType<VariantProps> }[];
}

export const ROUNDS: Round[] = [
  {
    id: 'focusEditor',
    states: ['pending', 'settled', 'refreshing', 'readonly'],
    variants: [
      { id: 'a', Component: FocusA },
      { id: 'b', Component: FocusB },
      { id: 'c', Component: FocusC },
      { id: 'd', Component: FocusD },
    ],
  },
  {
    id: 'changeCard',
    states: ['ready', 'incomplete', 'failed', 'stale', 'committed'],
    variants: [
      { id: 'a', Component: ChangeA },
      { id: 'b', Component: ChangeB },
      { id: 'c', Component: ChangeC },
      { id: 'd', Component: ChangeD },
    ],
  },
];

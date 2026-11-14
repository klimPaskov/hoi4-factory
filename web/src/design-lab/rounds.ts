// The open choices: one round per page, three or four variants each. A round leaves this list
// (and its variant files are deleted) once the maintainer has chosen and the page is built.
import type { ComponentType } from 'react';
import { ChangeCardPage } from './focus/ChangeVariants';
import { FocusEditor } from './focus/FocusVariants';

export interface VariantProps {
  /** One of the round's `states`, picked in the lab bar. */
  state: string;
}

export interface Round {
  id: string;
  states: string[];
  variants: { id: string; Component: ComponentType<VariantProps> }[];
}

// Both S04 rounds are decided. Each holds only the chosen design until the real page is built.
export const ROUNDS: Round[] = [
  {
    id: 'focusEditor',
    states: ['pending', 'settled', 'working', 'refreshing', 'readonly'],
    variants: [
      { id: 'a', Component: FocusEditor },
    ],
  },
  {
    id: 'changeCard',
    states: ['ready', 'incomplete', 'failed', 'stale', 'committed'],
    variants: [
      { id: 'a', Component: ChangeCardPage },
    ],
  },
];

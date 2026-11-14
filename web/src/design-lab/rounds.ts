// The open choices: one round per page, three or four variants each. A round leaves this list
// (and its variant files are deleted) once the maintainer has chosen and the page is built.
import type { ComponentType } from 'react';

export interface VariantProps {
  /** One of the round's `states`, picked in the lab bar. */
  state: string;
}

export interface Round {
  id: string;
  states: string[];
  variants: { id: string; Component: ComponentType<VariantProps> }[];
}

export const ROUNDS: Round[] = [];

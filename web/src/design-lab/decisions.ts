// What the maintainer is asked to look at. `OPEN` lists the open decisions, shown first on the
// lab's front page, one titled group each. `HISTORY` groups the images of every decision already made, newest
// first, for the archive page. Texts live in lab.en.json under designLab.review.
export interface Decision {
  id: string;
  /** Mockup file names (without .webp) in design-lab/mockups, one per option, in order. */
  options: string[];
  /** The id of the round in rounds.ts. Each option then links to its live variant (a, b, c ...). */
  round?: string;
}

export const OPEN: Decision[] = [
  {
    id: 'focusEditor',
    round: 'focusEditor',
    options: ['80-focus-a-canvas-inspector', '81-focus-b-three-panes', '82-focus-c-floating', '83-focus-d-table'],
  },
  {
    id: 'changeCard',
    round: 'changeCard',
    options: ['85-change-a-page', '86-change-b-split', '87-change-c-verdict', '88-change-d-tabs'],
  },
];

/**
 * Choices made overnight while the maintainer was away (they asked that work continue without
 * waiting for approval). Each is built and shown on the front page with its alternatives, so the
 * maintainer can confirm it or switch to another option. `chosen` indexes `options`.
 */
export const PROVISIONAL: (Decision & { chosen: number })[] = [];

export const HISTORY: Decision[] = [
  { id: 'account', options: ['70-account-a-cards', '71-account-b-settings', '72-account-c-usage'] },
  { id: 'taskPage', options: ['62-task-c-tabs', '61-task-b-story', '60-task-a-columns'] },
  { id: 'tasks', options: ['50-tasks-a-panel', '52-tasks-c-grouped', '51-tasks-b-composer'] },
  { id: 'setup', options: ['41-setup-b-vertical', '40-setup-a-horizontal', '42-setup-c-focused'] },
  { id: 'projectPage', options: ['32-project-c-guided', '30-project-a-overview', '31-project-b-tabs'] },
  { id: 'projectList', options: ['20-projects-a-table', '21-projects-b-cards', '22-projects-c-list', '23-projects-empty-mascot'] },
  {
    id: 'signIn',
    options: ['14-signin-c-cinematic', '10-signin-a-minimal', '11-signin-b-mascot', '12-signin-c-split', '13-signin-d-onestep-dark'],
  },
  { id: 'mascot', options: ['mascot-3', 'mascot-3a', 'mascot-3b', 'mascot-3c'] },
  { id: 'assets', options: ['icon-sheet', 'logo-factory-sheet'] },
  {
    id: 'targets',
    options: [
      '01-projects-desktop-light',
      '02-project-page-desktop-light',
      '03-setup-wizard-desktop-light',
      '04-tasks-desktop-light',
      '05-task-page-desktop-dark',
      '06-sign-in-desktop-light',
      '07-account-desktop-light',
      '08-phone-light',
      '09-projects-empty-desktop-dark',
    ],
  },
];

// What the maintainer is asked to look at. `OPEN` is the one open decision, shown first and alone
// on the lab's front page. `HISTORY` groups the images of every decision already made, newest
// first, for the archive page. Texts live in lab.en.json under designLab.review.
export interface Decision {
  id: string;
  /** Mockup file names (without .webp) in design-lab/mockups, one per option, in order. */
  options: string[];
}

export const OPEN: Decision | null = null;

/**
 * Choices made overnight while the maintainer was away (they asked that work continue without
 * waiting for approval). Each is built and shown on the front page with its alternatives, so the
 * maintainer can confirm it or switch to another option. `chosen` indexes `options`.
 */
export const PROVISIONAL: (Decision & { chosen: number })[] = [
  { id: 'setup', options: ['40-setup-a-horizontal', '41-setup-b-vertical', '42-setup-c-focused'], chosen: 0 },
];

export const HISTORY: Decision[] = [
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

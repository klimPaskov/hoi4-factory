// Names of the generated icons in src/assets/icons (one CSS class .icon-<name> each).
export const ICON_NAMES = [
  'account',
  'agent',
  'arrow-left',
  'branch',
  'check-circle',
  'chevron-right',
  'error',
  'folder',
  'info',
  'key',
  'moon',
  'pause',
  'pencil',
  'play',
  'plus',
  'revoke',
  'search',
  'sign-out',
  'stop',
  'sun',
  'tasks',
  'trash',
  'upload',
  'warning',
] as const;

export type IconName = (typeof ICON_NAMES)[number];

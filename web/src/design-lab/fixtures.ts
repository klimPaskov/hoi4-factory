// Sample data for the design lab. It reads like a real modder's account so variants are judged
// on realistic names, lengths and scripts (one right-to-left name), not on placeholder text.
import type { Project } from '../api/types';

export const SAMPLE_GAME = { version: '1.19.4.1', dlc: 34 };

export const SAMPLE_ACCOUNT = {
  displayName: 'Archduke Otto',
  email: 'otto.modder@example.com',
};

function project(
  id: string,
  name: string,
  created: string,
  extra: Partial<Project> = {},
): Project {
  return {
    id,
    name,
    visibility: 'private',
    created_at: created,
    deleted_at: null,
    delete_after: null,
    role: 'owner',
    ...extra,
  };
}

export const SAMPLE_PROJECTS: Project[] = [
  project('prj_fc36', 'Fractured Crowns: Alternate 1936', '2026-10-29T18:20:00Z'),
  project('prj_danube', 'Danubian Federation focus tree', '2026-10-21T09:05:00Z', {
    visibility: 'public',
  }),
  project('prj_nordic', 'Nordic Union Overhaul', '2026-10-02T14:40:00Z', {
    visibility: 'public',
    role: 'maintainer',
  }),
  project('prj_pacific', 'Pacific Theatre naval rework and the 1939 Imperial Japanese Navy refit events', '2026-09-14T07:12:00Z', {
    role: 'contributor',
  }),
  project('prj_arab', 'الاتحاد العربي', '2026-08-30T20:55:00Z'),
  project('prj_reddawn', 'Red Dawn: Russian Civil War events', '2026-07-11T11:00:00Z', {
    visibility: 'public',
    role: 'viewer',
  }),
  project('prj_old', 'Old test mod (1.12 saves)', '2025-12-03T16:30:00Z', {
    deleted_at: '2026-10-30T08:00:00Z',
    delete_after: '2026-11-29T08:00:00Z',
  }),
];

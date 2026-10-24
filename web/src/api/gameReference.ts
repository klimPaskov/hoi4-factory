// PENDING: GET /api/v1/game-reference is not served yet (the game-ref crate exists, the route
// does not). The response is read tolerantly: either the manifest itself or { game_reference }.
import { request } from './client';

export interface GameReferenceSummary {
  version: string | null;
  dlcCount: number | null;
}

interface Manifest {
  version?: string | null;
  raw_version?: string | null;
  numeric_version?: number[] | null;
  dlc_count?: number | null;
  dlc_ids?: string[] | null;
}

export function summarizeGameReference(data: unknown): GameReferenceSummary {
  const root = (data ?? {}) as { game_reference?: Manifest } & Manifest;
  const m: Manifest = root.game_reference ?? root;
  const version = m.numeric_version?.length
    ? m.numeric_version.join('.')
    : (m.raw_version ?? m.version ?? null);
  const dlcCount = typeof m.dlc_count === 'number' ? m.dlc_count : (m.dlc_ids?.length ?? null);
  return { version, dlcCount };
}

export async function getGameReference(signal?: AbortSignal): Promise<GameReferenceSummary> {
  return summarizeGameReference(await request<unknown>('/game-reference', { signal }));
}

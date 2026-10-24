// GET /api/v1/game-reference answers the manifest summary of the server's game reference:
// { available, version, raw_version, rev, dlc_ids, manifest_hash, read_only, reason? }.
// When the reference is not mounted, `available` is false and the version fields are absent.
import { request } from './client';

export interface GameReferenceSummary {
  version: string | null;
  dlcCount: number | null;
}

interface Manifest {
  available?: boolean;
  version?: string | null;
  raw_version?: string | null;
  dlc_ids?: string[] | null;
}

export function summarizeGameReference(data: unknown): GameReferenceSummary {
  const m = (data ?? {}) as Manifest;
  if (m.available === false) return { version: null, dlcCount: null };
  return { version: m.raw_version ?? m.version ?? null, dlcCount: m.dlc_ids?.length ?? null };
}

export async function getGameReference(signal?: AbortSignal): Promise<GameReferenceSummary> {
  return summarizeGameReference(await request<unknown>('/game-reference', { signal }));
}

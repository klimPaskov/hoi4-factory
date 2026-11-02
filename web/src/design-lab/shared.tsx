// Behavior shared by every variant, so variants differ only in layout and look. Everything runs
// on local sample data: no request leaves the page.
import { useId, useRef, useState, useSyncExternalStore, type FormEvent } from 'react';
import { useLocation, useSearchParams } from 'react-router';
import { Trans, useTranslation } from 'react-i18next';
import type { Project } from '../api/types';
import { applyTheme, readTheme, type ThemeChoice } from '../hooks';
import { SAMPLE_GAME, SAMPLE_PROJECTS } from './fixtures';

export type LabState = 'populated' | 'empty' | 'loading' | 'error';
export const LAB_STATES: LabState[] = ['populated', 'empty', 'loading', 'error'];

export interface DirectionProps {
  state: LabState;
  unverified: boolean;
}

const DAY = 86_400_000;

export interface SampleProjects {
  active: Project[];
  pending: Project[];
  create: (name: string) => void;
  rename: (id: string, name: string) => void;
  remove: (id: string) => void;
  restore: (id: string) => void;
}

export function useSampleProjects(state: LabState): SampleProjects {
  const [projects, setProjects] = useState<Project[]>(() =>
    state === 'populated' ? SAMPLE_PROJECTS : [],
  );
  const update = (id: string, change: Partial<Project>) =>
    setProjects((list) => list.map((p) => (p.id === id ? { ...p, ...change } : p)));
  return {
    active: projects.filter((p) => !p.deleted_at),
    pending: projects.filter((p) => p.deleted_at),
    create: (name) =>
      setProjects((list) => [
        {
          id: `prj_${Date.now()}`,
          name,
          visibility: 'private',
          created_at: new Date().toISOString(),
          deleted_at: null,
          delete_after: null,
          role: 'owner',
        },
        ...list,
      ]),
    rename: (id, name) => update(id, { name }),
    remove: (id) =>
      update(id, {
        deleted_at: new Date().toISOString(),
        delete_after: new Date(Date.now() + 30 * DAY).toISOString(),
      }),
    restore: (id) => update(id, { deleted_at: null, delete_after: null }),
  };
}

export type RowMode = 'idle' | 'rename' | 'delete';

/** Rename and delete for one project row, the way the real ProjectActions behaves. */
export function useRowActions(project: Project, model: SampleProjects) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<RowMode>('idle');
  const [name, setName] = useState(project.name);
  const [error, setError] = useState<string | undefined>();
  const canRename = project.role === 'owner' || project.role === 'maintainer';
  const canDelete = project.role === 'owner';
  return {
    mode,
    name,
    setName,
    error,
    canRename,
    canDelete,
    open(next: RowMode) {
      setName(project.name);
      setError(undefined);
      setMode(next);
    },
    close: () => setMode('idle'),
    submitRename(event: FormEvent) {
      event.preventDefault();
      if (!name.trim()) {
        setError(t('validation.nameRequired'));
        return;
      }
      model.rename(project.id, name.trim());
      setMode('idle');
    },
    confirmDelete() {
      model.remove(project.id);
      setMode('idle');
    },
  };
}

/** The new project form: validates like the real one, then adds to the sample list. */
export function useNewProject(model: SampleProjects, onCreated?: () => void) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [error, setError] = useState<string | undefined>();
  const inputRef = useRef<HTMLInputElement>(null);
  return {
    name,
    setName,
    error,
    inputRef,
    submit(event: FormEvent) {
      event.preventDefault();
      if (!name.trim()) {
        setError(t('validation.nameRequired'));
        inputRef.current?.focus();
        return;
      }
      setError(undefined);
      model.create(name.trim());
      setName('');
      onCreated?.();
    },
  };
}

export function useFilter(projects: Project[]) {
  const [query, setQuery] = useState('');
  const needle = query.trim().toLocaleLowerCase();
  const shown = needle
    ? projects.filter((p) => p.name.toLocaleLowerCase().includes(needle))
    : projects;
  return { query, setQuery, shown };
}

// ----- Theme, kept in step between the lab bar, the variant and the phone frame -----------

const themeListeners = new Set<() => void>();

function subscribeTheme(callback: () => void) {
  themeListeners.add(callback);
  // A phone or desktop frame is another document: follow its changes through storage events.
  const onStorage = (event: StorageEvent) => {
    if (event.key === 'hf-theme' || event.key === null) {
      applyTheme(readTheme());
      callback();
    }
  };
  window.addEventListener('storage', onStorage);
  return () => {
    themeListeners.delete(callback);
    window.removeEventListener('storage', onStorage);
  };
}

export function useLabTheme(): [ThemeChoice, (choice: ThemeChoice) => void] {
  const theme = useSyncExternalStore(subscribeTheme, readTheme);
  return [
    theme,
    (choice) => {
      applyTheme(choice);
      themeListeners.forEach((listener) => listener());
    },
  ];
}

/** The real header's theme control, restyled by each variant through `className`. */
export function ThemeControl({ className }: { className?: string }) {
  const { t } = useTranslation();
  const id = useId();
  const [theme, setTheme] = useLabTheme();
  return (
    <div className={className}>
      <label htmlFor={id}>{t('layout.theme')}</label>
      <select id={id} value={theme} onChange={(event) => setTheme(event.target.value as ThemeChoice)}>
        <option value="system">{t('layout.themeSystem')}</option>
        <option value="light">{t('layout.themeLight')}</option>
        <option value="dark">{t('layout.themeDark')}</option>
      </select>
    </div>
  );
}

export function useGameReferenceText(): string {
  const { t } = useTranslation();
  return t('gameReference.summary', { version: SAMPLE_GAME.version, dlc: SAMPLE_GAME.dlc });
}

export function ConfirmDeleteText({ name, nameClass }: { name: string; nameClass?: string }) {
  return (
    <Trans
      i18nKey="project.confirmDelete"
      values={{ name }}
      components={{ name: <bdi dir="auto" className={nameClass} /> }}
    />
  );
}

/** Links inside a variant stay on the variant: the lab has no real pages behind them. */
export function useHere() {
  const location = useLocation();
  return { pathname: location.pathname, search: location.search };
}

/** "Try again" in the error state reloads the variant with its projects. */
export function useRetry() {
  const [params, setParams] = useSearchParams();
  return () => {
    const next = new URLSearchParams(params);
    next.set('state', 'populated');
    setParams(next);
  };
}

/** Stable small number from a name, for decorative choices such as banner colors. */
export function nameHash(name: string): number {
  let hash = 0;
  for (const char of name) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) >>> 0;
  return hash;
}

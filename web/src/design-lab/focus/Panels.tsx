// The small shared parts of the focus editor variants: the source link and the problems list.
// Text is plain words. Both sit behind the one Details toggle.
import { useTranslation } from 'react-i18next';
import type { FocusModel } from './useFocusModel';

export function sourceHref(path: string, line: number): string {
  return `#source=${path}:${line}`;
}

export function SourceLink({ path, line }: { path: string; line: number }) {
  const { t } = useTranslation();
  return (
    <a href={sourceHref(path, line)} className="fx-source-link">
      {t('designLab.focus.viewSource')}
    </a>
  );
}

export const labelOf = (model: FocusModel, id: string): string => model.nodes.find((n) => n.id === id)?.label ?? id;

/** One plain sentence per problem. A problem about a focus selects that focus. */
export function DiagnosticsList({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  if (model.diagnostics.length === 0) return <p className="muted">{t('designLab.focus.noProblems')}</p>;
  return (
    <ul className="fx-diags">
      {model.diagnostics.map((d) => (
        <li key={`${d.code}:${d.focusId ?? ''}`}>
          <span className={`fx-dot is-${d.severity}`} aria-hidden="true" />
          {d.focusId ? (
            <button type="button" className="fx-link" onClick={() => model.reveal(d.focusId as string)}>
              {d.message}
            </button>
          ) : (
            <span>{d.message}</span>
          )}
        </li>
      ))}
    </ul>
  );
}

import { useTranslation } from 'react-i18next';
import type { WorkspaceSummary } from '../api/intake';
import { formatNumber } from '../format';
import { Badge, Facts } from './ui';

/** What is in the project's server workspace right now, as label and value pairs. */
export function WorkspaceFacts({ summary }: { summary: WorkspaceSummary }) {
  const { t } = useTranslation();
  const items = [
    summary.origin ? { key: 'origin', label: t('workspace.origin'), value: t(`workspace.origins.${summary.origin}`) } : null,
    typeof summary.file_count === 'number'
      ? { key: 'files', label: t('workspace.files'), value: formatNumber(summary.file_count) }
      : null,
    summary.ref ? { key: 'ref', label: t('workspace.ref'), value: <bdi dir="auto">{summary.ref}</bdi> } : null,
    summary.revision
      ? { key: 'revision', label: t('workspace.revision'), value: <span className="mono" dir="ltr">{summary.revision}</span> }
      : null,
    summary.setup_state
      ? {
          key: 'setup',
          label: t('workspace.setupState'),
          value: (
            <Badge tone={summary.setup_state === 'applied' ? 'success' : summary.setup_state === 'needs_rollback' ? 'warning' : 'neutral'}>
              {t(`workspace.states.${summary.setup_state}`, { defaultValue: summary.setup_state })}
            </Badge>
          ),
        }
      : null,
  ].filter((item) => item !== null);
  return <Facts items={items} />;
}

import { useTranslation } from 'react-i18next';
import type { SetupScan } from '../../api/intake';
import { Alert } from '../ui';
import { StepError } from './StepError';

/** Step 1: what the scan found. An incomplete scan is shown loudly. */
export function ScanStep({
  scan,
  busy,
  failure,
  onScan,
}: {
  scan: SetupScan | null;
  busy: boolean;
  failure: unknown;
  onScan: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div>
      {failure ? <StepError error={failure} what={t('setup.scan.what')} onRetry={onScan} /> : null}
      {scan ? (
        <>
          {scan.incomplete ? (
            <Alert kind="warning">
              <p>
                <strong>{t('setup.scan.incompleteTitle')}</strong>
              </p>
              <p>{t('setup.scan.incomplete')}</p>
            </Alert>
          ) : null}
          {scan.workspace_unchanged === false ? (
            <Alert kind="warning">{t('setup.scan.changed')}</Alert>
          ) : null}
          {scan.facts.length > 0 ? (
            <dl className="facts" aria-label={t('setup.scan.factsLabel')}>
              {scan.facts.map((fact) => (
                <div key={fact.label} className="fact">
                  <dt dir="auto">{fact.label}</dt>
                  <dd dir="auto">{fact.value}</dd>
                </div>
              ))}
            </dl>
          ) : (
            <p>{t('setup.scan.done')}</p>
          )}
          {scan.workspace_unchanged === true ? (
            <p className="muted small">{t('setup.scan.unchanged')}</p>
          ) : null}
        </>
      ) : (
        <p>{t('setup.scan.intro')}</p>
      )}
      <p className="button-row">
        <button type="button" className="button secondary" onClick={onScan} disabled={busy}>
          {busy ? t('setup.scan.working') : scan ? t('setup.scan.again') : t('setup.scan.button')}
        </button>
      </p>
      {scan ? <p className="muted small">{t('setup.scan.againHint')}</p> : null}
    </div>
  );
}

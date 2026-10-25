import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { approveEvidence, type SetupScan } from '../../api/intake';
import { formatNumber } from '../../format';
import { Alert } from '../ui';
import { StepError } from './StepError';

/**
 * Step 2: the person decides which excerpts of the mod may be sent to the model provider.
 * Everything starts checked, but nothing is sent until the explicit approve button is pressed.
 */
export function EvidenceStep({
  projectId,
  scan,
  state,
  approvedCount,
  onApproved,
  onApiError,
}: {
  projectId: string;
  scan: SetupScan;
  state: 'current' | 'done';
  approvedCount: number | null;
  onApproved: (count: number) => void;
  onApiError: (error: unknown) => boolean;
}) {
  const { t } = useTranslation();
  const candidates = scan.evidence.candidates;
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(candidates.map((c) => c.reference)),
  );
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown>(null);

  if (state === 'done') {
    return (
      <p>
        {approvedCount === null
          ? t('setup.evidence.doneUnknown')
          : approvedCount === 0
            ? t('setup.evidence.doneNone')
            : t('setup.evidence.done', { count: approvedCount, n: formatNumber(approvedCount) })}
      </p>
    );
  }

  const count = selected.size;

  function toggle(reference: string, on: boolean) {
    setSelected((previous) => {
      const next = new Set(previous);
      if (on) next.add(reference);
      else next.delete(reference);
      return next;
    });
  }

  async function approve() {
    setBusy(true);
    setFailure(null);
    try {
      const references = candidates.filter((c) => selected.has(c.reference)).map((c) => c.reference);
      const answer = await approveEvidence(projectId, { scan_id: scan.scan_id, references });
      onApproved(typeof answer.approved === 'number' ? answer.approved : references.length);
    } catch (error) {
      if (!onApiError(error)) setFailure(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <p>{t('setup.evidence.intro')}</p>
      <Alert kind="info">{t('setup.evidence.thirdParty')}</Alert>
      {scan.evidence.truncated ? <Alert kind="warning">{t('setup.evidence.truncated')}</Alert> : null}
      {candidates.length === 0 ? (
        <p>{t('setup.evidence.noCandidates')}</p>
      ) : (
        <>
          <p className="button-row">
            <button
              type="button"
              className="button secondary small-button"
              onClick={() => setSelected(new Set(candidates.map((c) => c.reference)))}
              disabled={busy}
            >
              {t('setup.evidence.selectAll')}
            </button>
            <button
              type="button"
              className="button secondary small-button"
              onClick={() => setSelected(new Set())}
              disabled={busy}
            >
              {t('setup.evidence.selectNone')}
            </button>
          </p>
          <ul className="evidence-list" aria-label={t('setup.evidence.listLabel')}>
            {candidates.map((candidate) => (
              <li key={candidate.reference} className="evidence-item">
                <label className="evidence-head">
                  <input
                    type="checkbox"
                    checked={selected.has(candidate.reference)}
                    onChange={(event) => toggle(candidate.reference, event.target.checked)}
                    disabled={busy}
                  />{' '}
                  <bdi dir="auto">{candidate.reference}</bdi>
                </label>
                <p className="small muted evidence-meta">
                  <span dir="ltr" className="path">
                    {candidate.path}
                  </span>{' '}
                  <bdi dir="auto">
                    {candidate.category} / {candidate.key}
                  </bdi>
                </p>
                <pre className="excerpt" dir="auto">
                  {candidate.excerpt}
                </pre>
              </li>
            ))}
          </ul>
        </>
      )}
      {count === 0 ? <Alert kind="warning">{t('setup.evidence.noneWarning')}</Alert> : null}
      {failure ? (
        <StepError error={failure} what={t('setup.evidence.what')} onRetry={() => void approve()} />
      ) : null}
      <p className="button-row">
        <button type="button" className="button primary" onClick={() => void approve()} disabled={busy}>
          {busy
            ? t('setup.evidence.approving')
            : count === 0
              ? t('setup.evidence.approveNone')
              : t('setup.evidence.approve', { count })}
        </button>
      </p>
    </div>
  );
}

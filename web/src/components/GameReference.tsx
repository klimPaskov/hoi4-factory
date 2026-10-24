import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import { getGameReference } from '../api/gameReference';
import { formatNumber } from '../format';
import { useAsync } from '../hooks';

/** Small header indicator: which Hearts of Iron IV build the server checks mods against. */
export function GameReferenceIndicator() {
  const { t } = useTranslation();
  const state = useAsync((signal) => getGameReference(signal));

  let text: string;
  let tone = '';
  if (state.status === 'loading') {
    text = t('gameReference.loading');
  } else if (state.status === 'error') {
    const unavailable = state.error instanceof ApiError && state.error.isUnavailable;
    text = unavailable ? t('gameReference.notAvailable') : t('gameReference.failed');
    tone = ' muted';
  } else if (state.data.version === null) {
    text = t('gameReference.unknown');
    tone = ' muted';
  } else {
    text = t('gameReference.summary', {
      version: state.data.version,
      dlc: formatNumber(state.data.dlcCount ?? 0),
    });
  }

  return (
    <p className={`game-reference small${tone}`} data-testid="game-reference">
      <span className="game-reference-label">{t('gameReference.label')}</span> {text}
      {state.status === 'error' && !(state.error instanceof ApiError && state.error.isUnavailable) ? (
        <>
          {' '}
          <button type="button" className="link-button" onClick={state.reload}>
            {t('common.tryAgain')}
          </button>
        </>
      ) : null}
    </p>
  );
}

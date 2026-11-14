// The change card (D-61, chosen 2026-11-14): the review of one change before it is saved. The game
// picture of the result fills the page, with one plain sentence, one main and one secondary
// action, and the one Details toggle. There is no before and after comparison.
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { VariantProps } from '../rounds';
import { Decision, Picture, ReviewDetails, Summary, useCard } from './ChangeParts';
import type { CardScenario } from './fixtures';

/** True while the bar floats over the picture. On a narrow screen it sits below instead. */
function useWide(): boolean {
  const query = '(min-width: 48.01rem)';
  const [wide, setWide] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const media = window.matchMedia(query);
    const on = () => setWide(media.matches);
    media.addEventListener('change', on);
    return () => media.removeEventListener('change', on);
  }, []);
  return wide;
}

/** Room kept free under the fitted picture for the bar that floats over it, in pixels. */
const BAR_ROOM = 150;

export function ChangeCardPage({ state }: VariantProps) {
  const { t } = useTranslation();
  const ctl = useCard(state as CardScenario);
  const wide = useWide();
  return (
    <div className="cx-page">
      <h1 className="cx-title">{t('designLab.change.title')}</h1>
      <div className="cx-stage">
        <Picture under={wide ? BAR_ROOM : 0} />
        <div className="cx-bar">
          <Summary card={ctl.card} />
          <Decision ctl={ctl} />
        </div>
      </div>
      <ReviewDetails card={ctl.card} />
    </div>
  );
}

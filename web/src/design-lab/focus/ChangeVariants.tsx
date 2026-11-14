// Four minimal layouts of the change card (D-61): the review of one change before it is saved.
// Each shows the game picture of the change, one plain sentence, one main and one secondary
// action, and the one Details toggle. They differ in how before and after are compared and in
// how much of the page the card takes.
//   A. One picture fills the page, with an After and Before switch.
//   B. One picture fills the page, with a divider to drag between before and after.
//   C. Before and after side by side.
//   D. A small card, as it would sit in a task's conversation.
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { VariantProps } from '../rounds';
import { Decision, Picture, ReviewDetails, Summary, useCard, type CardControl, type PictureMode } from './ChangeParts';
import type { CardScenario } from './fixtures';

function useScenario(state: string): CardControl {
  return useCard(state as CardScenario);
}

function Title() {
  const { t } = useTranslation();
  return <h1 className="cx-title">{t('designLab.change.title')}</h1>;
}

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

/** The picture fills the page. The sentence and the two buttons float over its lower edge. */
function FullPage({ state, mode, name }: { state: string; mode: PictureMode; name: string }) {
  const ctl = useScenario(state);
  const wide = useWide();
  return (
    <div className={`cx-page ${name}`}>
      <Title />
      <div className="cx-stage">
        <Picture mode={mode} under={wide ? BAR_ROOM : 0} />
        <div className="cx-bar">
          <Summary card={ctl.card} />
          <Decision ctl={ctl} />
        </div>
      </div>
      <ReviewDetails card={ctl.card} />
    </div>
  );
}

/** A. One picture with an After and Before switch. */
export function ChangeA({ state }: VariantProps) {
  return <FullPage state={state} mode="toggle" name="cxa" />;
}

/** B. One picture with a divider to drag between before and after. */
export function ChangeB({ state }: VariantProps) {
  return <FullPage state={state} mode="slider" name="cxb" />;
}

/** C. Before and after side by side, the sentence and the buttons in one row under them. */
export function ChangeC({ state }: VariantProps) {
  const ctl = useScenario(state);
  return (
    <div className="cx-page cxc">
      <Title />
      <div className="cx-stage">
        <Picture mode="side" />
      </div>
      <div className="cx-row">
        <Summary card={ctl.card} />
        <Decision ctl={ctl} />
      </div>
      <ReviewDetails card={ctl.card} />
    </div>
  );
}

/** D. A small card: the sentence, the picture with the switch, the buttons. It fits inside a task's conversation. */
export function ChangeD({ state }: VariantProps) {
  const ctl = useScenario(state);
  return (
    <div className="cx-page cxd">
      <article className="cx-card">
        <Title />
        <Summary card={ctl.card} />
        <div className="cx-card-picture">
          <Picture mode="toggle" />
        </div>
        <Decision ctl={ctl} />
        <ReviewDetails card={ctl.card} />
      </article>
    </div>
  );
}

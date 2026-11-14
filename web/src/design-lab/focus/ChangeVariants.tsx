// Four layouts of the change card: the review of one proposal before it is committed. Each shows the
// render, a few plain lines about what changed, Approve and Discard, and one Details toggle.
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Tabs } from '../../components/ui';
import type { VariantProps } from '../rounds';
import { Actions, CardHeader, DetailsBody, PreviewSection, ReviewDetails, SummarySection, useCard, type CardControl } from './ChangeParts';
import type { CardScenario } from './fixtures';

function useScenario(state: string): CardControl {
  return useCard(state as CardScenario);
}

/** A. One page in review order. The before and after renders sit side by side at the top, and the actions stay in a bar at the bottom. */
export function ChangeA({ state }: VariantProps) {
  const ctl = useScenario(state);
  return (
    <div className="fc-page fca">
      <CardHeader ctl={ctl} />
      <PreviewSection mode="side" />
      <SummarySection card={ctl.card} />
      <ReviewDetails card={ctl.card} />
      <div className="fca-bar">
        <Actions ctl={ctl} />
      </div>
    </div>
  );
}

/** B. Two columns: what changed and the actions on the left, one render with a before and after toggle on the right. */
export function ChangeB({ state }: VariantProps) {
  const ctl = useScenario(state);
  return (
    <div className="fc-page fcb">
      <div className="fcb-grid">
        <div className="fcb-left">
          <CardHeader ctl={ctl} />
          <SummarySection card={ctl.card} />
          <Actions ctl={ctl} />
          <ReviewDetails card={ctl.card} />
        </div>
        <PreviewSection mode="toggle" />
      </div>
    </div>
  );
}

/** C. The decision first: what changed and the actions in one line of text above one render with a before and after slider. */
export function ChangeC({ state }: VariantProps) {
  const ctl = useScenario(state);
  return (
    <div className="fc-page fcc">
      <CardHeader ctl={ctl} />
      <section className="fcc-verdict">
        <SummarySection card={ctl.card} />
        <Actions ctl={ctl} />
      </section>
      <PreviewSection mode="slider" />
      <ReviewDetails card={ctl.card} />
    </div>
  );
}

/** D. The render with a before and after toggle on top, always in view. What changed and Details are tabs under it, with the actions beside the tab row. */
export function ChangeD({ state }: VariantProps) {
  const { t } = useTranslation();
  const ctl = useScenario(state);
  const [tab, setTab] = useState('summary');
  return (
    <div className="fc-page fcd">
      <CardHeader ctl={ctl} />
      <PreviewSection mode="toggle" className="fcd-preview" />
      <Tabs
        label={t('designLab.change.sections.evidence')}
        value={tab}
        onChange={setTab}
        aside={<Actions ctl={ctl} />}
        tabs={[
          { id: 'summary', label: t('designLab.change.sections.summary'), panel: <SummarySection card={ctl.card} /> },
          { id: 'details', label: t('designLab.details'), panel: <DetailsBody card={ctl.card} /> },
        ]}
      />
    </div>
  );
}

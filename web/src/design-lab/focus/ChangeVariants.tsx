// Four layouts of the change card: the review of one proposal before it is committed.
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge, Card, Tabs } from '../../components/ui';
import type { SourceRef } from '../../focus/types';
import type { VariantProps } from '../rounds';
import {
  Actions,
  CardHeader,
  ChecksSection,
  ClosureDetail,
  DiffSection,
  FindingDelta,
  jump,
  LimitsSection,
  PreviewSection,
  RestorePoint,
  StaleBanner,
  SummarySection,
  VerdictStrip,
  useCard,
  type CardControl,
} from './ChangeParts';
import type { CardScenario } from './fixtures';
import { LayoutBadge } from './Panels';

function useScenario(state: string): CardControl {
  return useCard(state as CardScenario);
}

function useJump(ctl: CardControl) {
  return (source: SourceRef) => jump(source.path, source.line, ctl.card.files);
}

/** A. One scrolling page in review order, with the actions in a bar that stays at the bottom. */
export function ChangeA({ state }: VariantProps) {
  const { t } = useTranslation();
  const ctl = useScenario(state);
  const onJump = useJump(ctl);
  const { card } = ctl;
  return (
    <div className="fc-page fca">
      <CardHeader ctl={ctl} />
      <StaleBanner ctl={ctl} />
      <VerdictStrip ctl={ctl} />
      <Card title={t('designLab.change.sections.summary')}>
        <SummarySection card={card} onJump={onJump} />
      </Card>
      <Card title={t('designLab.change.sections.preview')}>
        <PreviewSection card={card} onJump={onJump} />
      </Card>
      <Card title={t('designLab.change.sections.diff')}>
        <DiffSection card={card} />
      </Card>
      <Card title={t('designLab.change.sections.findings')}>
        <FindingDelta delta={card.findingDelta} />
        <ClosureDetail closure={card.closure} />
        <p>
          {t('designLab.change.layoutLabel')} <LayoutBadge verdict={card.layoutVerdict} />
        </p>
      </Card>
      <Card title={t('designLab.change.sections.checks')}>
        <ChecksSection checks={card.checks} />
      </Card>
      <Card title={t('designLab.change.sections.limits')}>
        <LimitsSection limits={card.limits} />
        <RestorePoint card={card} />
      </Card>
      <div className="fca-bar">
        <Actions ctl={ctl} />
      </div>
    </div>
  );
}

/** B. Two columns: what changed and whether it is safe on the left, the evidence on the right. */
export function ChangeB({ state }: VariantProps) {
  const { t } = useTranslation();
  const ctl = useScenario(state);
  const onJump = useJump(ctl);
  const [tab, setTab] = useState('preview');
  const { card } = ctl;
  return (
    <div className="fc-page fcb">
      <CardHeader ctl={ctl} />
      <StaleBanner ctl={ctl} />
      <div className="fcb-grid">
        <div className="fcb-left">
          <Card title={t('designLab.change.sections.summary')}>
            <SummarySection card={card} onJump={onJump} />
          </Card>
          <Card title={t('designLab.change.sections.findings')}>
            <FindingDelta delta={card.findingDelta} />
            <ClosureDetail closure={card.closure} />
            <p>
              {t('designLab.change.layoutLabel')} <LayoutBadge verdict={card.layoutVerdict} />
            </p>
          </Card>
          <Card title={t('designLab.change.sections.checks')}>
            <ChecksSection checks={card.checks} />
          </Card>
          <Card title={t('designLab.change.sections.limits')}>
            <LimitsSection limits={card.limits} />
            <RestorePoint card={card} />
          </Card>
        </div>
        <div className="fcb-right">
          <div className="fcb-actions">
            <Actions ctl={ctl} />
          </div>
          <Tabs
            label={t('designLab.change.sections.evidence')}
            value={tab}
            onChange={setTab}
            tabs={[
              { id: 'preview', label: t('designLab.change.sections.preview'), panel: <PreviewSection card={card} onJump={onJump} /> },
              { id: 'diff', label: t('designLab.change.sections.diff'), panel: <DiffSection card={card} /> },
            ]}
          />
        </div>
      </div>
    </div>
  );
}

/** C. The decision first: can this be committed, and if not, why. The evidence sits below it. */
export function ChangeC({ state }: VariantProps) {
  const { t } = useTranslation();
  const ctl = useScenario(state);
  const onJump = useJump(ctl);
  const { card } = ctl;
  const failed = card.checks.filter((c) => c.result === 'failed').length;
  const unavailable = card.checks.filter((c) => c.result === 'unavailable').length;
  const ready = card.closure.verdict === 'pass' && failed === 0 && unavailable === 0 && card.state === 'pending';
  const headline = card.state === 'committed' ? 'committed' : card.state === 'stale' ? 'stale' : failed > 0 ? 'failed' : ready ? 'ready' : 'incomplete';
  return (
    <div className="fc-page fcc">
      <CardHeader ctl={ctl} />
      <section className={`fcc-verdict is-${headline}`} aria-labelledby="fcc-headline">
        <h2 id="fcc-headline">{t(`designLab.change.headline.${headline}`)}</h2>
        <p>{t(`designLab.change.headlineBody.${headline}`, { unknown: card.closure.unknown, failed, unavailable })}</p>
        <ul className="fcc-gates">
          <li>
            <details>
              <summary>
                <span>{t('designLab.change.gates.closure')}</span>
                <Badge tone={card.closure.verdict === 'pass' ? 'success' : card.closure.verdict === 'fail' ? 'danger' : 'warning'}>{t(`designLab.change.closure.${card.closure.verdict}`)}</Badge>
              </summary>
              <ClosureDetail closure={card.closure} />
            </details>
          </li>
          <li>
            <details>
              <summary>
                <span>{t('designLab.change.gates.layout')}</span>
                <LayoutBadge verdict={card.layoutVerdict} />
              </summary>
              <p className="small muted">{t('designLab.change.gates.layoutNote')}</p>
            </details>
          </li>
          <li>
            <details>
              <summary>
                <span>{t('designLab.change.gates.findings')}</span>
                <Badge tone={card.findingDelta.introduced + card.findingDelta.worsened > 0 ? 'danger' : 'success'}>
                  {t('designLab.change.delta.short', { introduced: card.findingDelta.introduced, worsened: card.findingDelta.worsened })}
                </Badge>
              </summary>
              <FindingDelta delta={card.findingDelta} />
            </details>
          </li>
          <li>
            <details open={failed + unavailable > 0}>
              <summary>
                <span>{t('designLab.change.gates.checks')}</span>
                <Badge tone={failed ? 'danger' : unavailable ? 'warning' : 'success'}>
                  {t('designLab.change.gates.checkCount', { passed: card.checks.filter((c) => c.result === 'passed').length, total: card.checks.length })}
                </Badge>
              </summary>
              <ChecksSection checks={card.checks} />
            </details>
          </li>
        </ul>
        <Actions ctl={ctl} />
      </section>
      <Card title={t('designLab.change.sections.summary')}>
        <SummarySection card={card} onJump={onJump} />
      </Card>
      <Card title={t('designLab.change.sections.preview')}>
        <PreviewSection card={card} onJump={onJump} />
      </Card>
      <Card title={t('designLab.change.sections.diff')}>
        <DiffSection card={card} open={false} />
      </Card>
      <Card title={t('designLab.change.sections.limits')}>
        <LimitsSection limits={card.limits} />
        <RestorePoint card={card} />
      </Card>
    </div>
  );
}

/** D. Tabs: Summary, Changes, Preview and Checks, with the actions beside the tab row. */
export function ChangeD({ state }: VariantProps) {
  const { t } = useTranslation();
  const ctl = useScenario(state);
  const [tab, setTab] = useState('summary');
  const { card } = ctl;
  const onJump = (source: SourceRef) => {
    setTab('changes');
    window.setTimeout(() => jump(source.path, source.line, card.files), 50);
  };
  return (
    <div className="fc-page fcd">
      <CardHeader ctl={ctl} />
      <StaleBanner ctl={ctl} />
      <VerdictStrip ctl={ctl} />
      <Tabs
        label={t('designLab.change.sections.evidence')}
        value={tab}
        onChange={setTab}
        aside={<Actions ctl={ctl} />}
        tabs={[
          {
            id: 'summary',
            label: t('designLab.change.sections.summary'),
            panel: (
              <>
                <SummarySection card={card} onJump={onJump} />
                <FindingDelta delta={card.findingDelta} />
                <ClosureDetail closure={card.closure} />
                <p>
                  {t('designLab.change.layoutLabel')} <LayoutBadge verdict={card.layoutVerdict} />
                </p>
              </>
            ),
          },
          { id: 'changes', label: t('designLab.change.sections.diff'), panel: <DiffSection card={card} /> },
          { id: 'preview', label: t('designLab.change.sections.preview'), panel: <PreviewSection card={card} onJump={onJump} /> },
          {
            id: 'checks',
            label: t('designLab.change.sections.checks'),
            panel: (
              <>
                <ChecksSection checks={card.checks} />
                <h3>{t('designLab.change.sections.limits')}</h3>
                <LimitsSection limits={card.limits} />
                <RestorePoint card={card} />
              </>
            ),
          },
        ]}
      />
    </div>
  );
}

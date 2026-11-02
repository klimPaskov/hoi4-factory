// The approved design system on one page: the real tokens, styles and ui.tsx components, in
// every variant and state, for review in both themes (ADR-025).
import { useState } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { ApiError } from '../api/client';
import {
  Alert,
  Badge,
  BrandMark,
  Button,
  ButtonRow,
  Card,
  CheckboxField,
  Chip,
  ChoiceGroup,
  Dialog,
  EmptyState,
  ErrorMessage,
  Facts,
  Field,
  FlagAvatar,
  Icon,
  Loading,
  Meter,
  NotAvailableYet,
  PageHeader,
  SegmentedControl,
  SelectField,
  Skeleton,
  StatCard,
  StepIndicator,
  Timeline,
} from '../components/ui';
import { ICON_NAMES } from '../components/iconNames';
import { Mascot, type MascotPose } from '../components/Mascot';
import { formatDate, formatMicros } from '../format';
import { usePageTitle } from '../hooks';
import { SAMPLE_PROJECTS } from './fixtures';
import { ConfirmDeleteText, ThemeControl } from './shared';

const SWATCHES = [
  'bg',
  'surface',
  'surface-2',
  'surface-3',
  'border',
  'border-strong',
  'input-border',
  'text',
  'muted',
  'primary',
  'primary-soft',
  'link',
  'danger',
  'danger-bg',
  'success',
  'success-bg',
  'warning',
  'warning-bg',
  'info',
  'info-bg',
];

const MASCOT_POSES: MascotPose[] = [
  'wave',
  'point-right',
  'point-left',
  'point-up',
  'point-down',
  'cheer',
  'thumbs-up',
  'think',
  'reading',
  'working',
  'waiting',
  'surprised',
  'sorry',
  'peek',
  'master',
];

export function ComponentsPage() {
  const { t } = useTranslation();
  const c = (key: string, options?: Record<string, unknown>) => t(`designLab.components.${key}`, options);
  usePageTitle(c('title'));
  const [name, setName] = useState('Danubian Federation');
  const [objective, setObjective] = useState('');
  const [mode, setMode] = useState('read_only');
  const [model, setModel] = useState('haiku');
  const [consent, setConsent] = useState(true);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [segment, setSegment] = useState('edit');
  const [pose, setPose] = useState<MascotPose>('wave');
  const [motion, setMotion] = useState<'enter' | 'idle' | 'bounce' | 'none'>('idle');
  const [paths, setPaths] = useState(['common/national_focus', 'common/ideas', 'localisation/english']);
  const retryable = new ApiError({ status: 503, code: 'unavailable', message: '', retryClass: 'safe_same_inputs' });

  return (
    <div className="lab-components">
      <div className="lab-bar" role="region" aria-label={t('designLab.toolbar.label')}>
        <Link to="/design-lab" className="lab-home">
          {t('designLab.toolbar.home')}
        </Link>
        <ThemeControl className="lab-theme" />
      </div>
      <div className="app-main lab-components-main">
        <PageHeader title={c('title')} subtitle={c('subtitle')} />

        <section className="section" aria-labelledby="dc-brand">
          <h2 id="dc-brand" className="section-title">
            {c('brand')}
          </h2>
          <p className="brand lab-brand">
            <BrandMark />
            <span>{t('app.name')}</span>
          </p>
        </section>

        <section className="section" aria-labelledby="dc-mascot">
          <h2 id="dc-mascot" className="section-title">
            {c('mascot')}
          </h2>
          <div className="card lab-stack">
            <Mascot pose={pose} motion={motion} size="lg">
              <p>{c('mascotSays')}</p>
            </Mascot>
            <div className="lab-fields">
              <SelectField label={c('mascotPose')} value={pose} onChange={(v) => setPose(v as MascotPose)} options={MASCOT_POSES.map((p) => ({ value: p, label: p }))} />
              <SegmentedControl
                legend={c('mascotMotion')}
                value={motion}
                onChange={(v) => setMotion(v as typeof motion)}
                options={(['enter', 'idle', 'bounce', 'none'] as const).map((m) => ({ value: m, label: c(`motions.${m}`) }))}
              />
            </div>
            <ul className="lab-poses">
              {MASCOT_POSES.map((p) => (
                <li key={p}>
                  <button type="button" className={p === pose ? 'is-selected' : undefined} onClick={() => setPose(p)} aria-pressed={p === pose}>
                    <Mascot pose={p} size="sm" motion="none" />
                    <code>{p}</code>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </section>

        <section className="section" aria-labelledby="dc-icons">
          <h2 id="dc-icons" className="section-title">
            {c('icons')}
          </h2>
          <ul className="lab-icons">
            {ICON_NAMES.map((name) => (
              <li key={name}>
                <Icon name={name} size="lg" />
                <code>{name}</code>
              </li>
            ))}
          </ul>
        </section>

        <section className="section" aria-labelledby="dc-new">
          <h2 id="dc-new" className="section-title">
            {c('newParts')}
          </h2>
          <div className="stats">
            <StatCard icon="folder" label={c('statProjects')} value={'6'} detail={c('statDetail')} />
            <StatCard icon="agent" label={c('statRunning')} value={'2'} />
            <StatCard icon="key" label={c('statSpent')} value={formatMicros(141_680)} />
          </div>
          <div className="card lab-stack">
            <Meter label={c('meterLabel')} value={141_680} max={1_000_000} text={`${formatMicros(141_680)} / ${formatMicros(1_000_000)}`} />
            <SegmentedControl
              legend={c('segLegend')}
              value={segment}
              onChange={setSegment}
              options={[
                { value: 'read_only', label: c('segRead') },
                { value: 'edit', label: c('segEdit') },
              ]}
            />
            <div className="chips">
              {paths.map((path) => (
                <Chip key={path} onRemove={() => setPaths(paths.filter((p) => p !== path))} removeLabel={c('chipRemove', { path })}>
                  {path}
                </Chip>
              ))}
            </div>
            <Timeline
              label={c('timelineLabel')}
              items={[
                { key: '1', icon: 'search', title: c('tl1'), meta: c('tlMeta') },
                { key: '2', icon: 'check-circle', tone: 'success', title: c('tl2'), meta: c('tlMeta') },
                {
                  key: '3',
                  icon: 'pencil',
                  tone: 'accent',
                  title: c('tl3'),
                  meta: (
                    <Badge tone="accent" live>
                      {c('running')}
                    </Badge>
                  ),
                },
              ]}
            />
            <div>
              <p className="section-title">{c('flags')}</p>
              <p className="lab-flags">
                {SAMPLE_PROJECTS.map((project) => (
                  <FlagAvatar key={project.id} name={project.name} />
                ))}
              </p>
            </div>
          </div>
        </section>

        <section className="section" aria-labelledby="dc-colors">
          <h2 id="dc-colors" className="section-title">
            {c('colors')}
          </h2>
          <ul className="lab-swatches">
            {SWATCHES.map((token) => (
              <li key={token}>
                <span className={`lab-swatch lab-swatch-${token}`} aria-hidden="true" />
                <code>--{token}</code>
              </li>
            ))}
          </ul>
        </section>

        <section className="section" aria-labelledby="dc-type">
          <h2 id="dc-type" className="section-title">
            {c('type')}
          </h2>
          <div className="card">
            <p className="lab-h1">{c('typeH1')}</p>
            <p className="lab-h2">{c('typeH2')}</p>
            <p>{c('typeBody')}</p>
            <p className="small muted">{c('typeSmall')}</p>
            <p>
              <code>common/national_focus/danube.txt</code>
            </p>
          </div>
        </section>

        <section className="section" aria-labelledby="dc-buttons">
          <h2 id="dc-buttons" className="section-title">
            {c('buttons')}
          </h2>
          <div className="card lab-stack">
            <ButtonRow>
              <Button variant="primary" icon="plus">
                {t('projects.newTitle')}
              </Button>
              <Button icon="pencil">{t('project.rename')}</Button>
              <Button variant="ghost">{t('common.cancel')}</Button>
              <Button variant="danger-outline" icon="trash">
                {t('project.delete')}
              </Button>
              <Button variant="danger">{t('project.confirmDeleteButton')}</Button>
            </ButtonRow>
            <ButtonRow>
              <Button variant="primary" size="sm">
                {t('common.save')}
              </Button>
              <Button size="sm">{t('common.tryAgain')}</Button>
              <Button variant="ghost" size="sm">
                {t('project.restore')}
              </Button>
              <Button variant="danger-outline" size="sm">
                {t('project.delete')}
              </Button>
              <Button variant="primary" disabled>
                {t('projects.creating')}
              </Button>
            </ButtonRow>
            <div className="lab-narrow">
              <Button variant="primary" block>
                {t('common.signIn')}
              </Button>
            </div>
          </div>
        </section>

        <section className="section" aria-labelledby="dc-fields">
          <h2 id="dc-fields" className="section-title">
            {c('fields')}
          </h2>
          <div className="card lab-fields">
            <div>
              <Field label={t('projects.nameLabel')} hint={t('projects.nameHint')} value={name} onChange={setName} />
              <Field label={t('auth.email')} type="email" value="" onChange={() => {}} error={t('validation.emailInvalid')} dir="ltr" />
              <Field label={c('objective')} value={objective} onChange={setObjective} multiline optional />
            </div>
            <div>
              <SelectField
                label={c('model')}
                value={model}
                onChange={setModel}
                options={[
                  { value: 'haiku', label: 'Claude Haiku 5.5' },
                  { value: 'sonnet', label: 'Claude Sonnet 5.5' },
                ]}
                hint={c('modelHint')}
              />
              <ChoiceGroup
                legend={c('mode')}
                value={mode}
                onChange={setMode}
                options={[
                  { value: 'read_only', label: c('modeRead') },
                  { value: 'edit', label: c('modeEdit') },
                ]}
              />
              <CheckboxField label={c('consent')} checked={consent} onChange={setConsent} />
            </div>
          </div>
        </section>

        <section className="section" aria-labelledby="dc-notices">
          <h2 id="dc-notices" className="section-title">
            {c('notices')}
          </h2>
          <Alert kind="info">
            <p>{t('tasks.eventsLive')}</p>
          </Alert>
          <Alert kind="success">
            <p>{t('verify.done')}</p>
          </Alert>
          <Alert kind="warning">
            <p className="alert-title">{c('warningTitle')}</p>
            <p>{t('project.pendingNotice')}</p>
            <ButtonRow>
              <Button size="sm">{t('project.restore')}</Button>
            </ButtonRow>
          </Alert>
          <ErrorMessage error={retryable} onRetry={() => {}} />
          <NotAvailableYet what={t('workspace.what')} />
        </section>

        <section className="section" aria-labelledby="dc-badges">
          <h2 id="dc-badges" className="section-title">
            {c('badges')}
          </h2>
          <p className="lab-badges">
            <Badge>{t('project.visibility.private')}</Badge>
            <Badge tone="success">{t('project.visibility.public')}</Badge>
            <Badge tone="accent" live>
              {c('running')}
            </Badge>
            <Badge tone="warning">{c('paused')}</Badge>
            <Badge tone="danger">{c('failed')}</Badge>
            <Badge tone="info">{c('inReview')}</Badge>
          </p>
        </section>

        <section className="section" aria-labelledby="dc-cards">
          <h2 id="dc-cards" className="section-title">
            {c('cards')}
          </h2>
          <Card title={t('workspace.title')} actions={<Button size="sm">{c('refresh')}</Button>}>
            <Facts
              items={[
                { label: t('workspace.origin'), value: t('workspace.origins.github') },
                { label: t('workspace.files'), value: '1,284' },
                { label: t('workspace.ref'), value: <span className="mono">{'main'}</span> },
                { label: t('workspace.setupState'), value: <Badge tone="success">{t('workspace.states.applied')}</Badge> },
              ]}
            />
          </Card>
        </section>

        <section className="section" aria-labelledby="dc-table">
          <h2 id="dc-table" className="section-title">
            {c('table')}
          </h2>
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th scope="col">{c('colName')}</th>
                  <th scope="col">{c('colVisibility')}</th>
                  <th scope="col">{c('colCreated')}</th>
                  <th scope="col">
                    <span className="sr-only">{c('colActions')}</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {SAMPLE_PROJECTS.slice(0, 4).map((project) => (
                  <tr key={project.id}>
                    <th scope="row">
                      <bdi dir="auto">{project.name}</bdi>
                    </th>
                    <td className="cell-nowrap">
                      <Badge tone={project.visibility === 'public' ? 'success' : 'neutral'}>
                        {t(`project.visibility.${project.visibility}`)}
                      </Badge>
                    </td>
                    <td className="cell-nowrap">{formatDate(project.created_at)}</td>
                    <td className="cell-actions">
                      <Button variant="ghost" size="sm">
                        {t('project.rename')}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <section className="section" aria-labelledby="dc-states">
          <h2 id="dc-states" className="section-title">
            {c('states')}
          </h2>
          <EmptyState
            icon
            title={t('projects.emptyTitle')}
            action={<Button variant="primary">{t('projects.create')}</Button>}
          >
            {t('projects.emptyBody')}
          </EmptyState>
          <div className="card lab-stack">
            <Loading label={t('projects.loading')} />
            <Skeleton label={t('projects.loading')} rows={4} />
          </div>
        </section>

        <section className="section" aria-labelledby="dc-steps">
          <h2 id="dc-steps" className="section-title">
            {c('steps')}
          </h2>
          <div className="card">
            <StepIndicator
              label={c('stepsLabel')}
              steps={[
                { label: c('stepScan'), state: 'done' },
                { label: c('stepExcerpts'), state: 'done' },
                { label: c('stepAnalyze'), state: 'done' },
                { label: c('stepConfirm'), state: 'current' },
                { label: c('stepPlan'), state: 'upcoming' },
                { label: c('stepApply'), state: 'upcoming' },
              ]}
            />
            <StepIndicator
              label={c('stepsLabel')}
              steps={[
                { label: c('stepScan'), state: 'done' },
                { label: c('stepExcerpts'), state: 'done' },
                { label: c('stepAnalyze'), state: 'done' },
                { label: c('stepConfirm'), state: 'done' },
                { label: c('stepPlan'), state: 'done' },
                { label: c('stepApply'), state: 'error' },
              ]}
            />
          </div>
        </section>

        <section className="section" aria-labelledby="dc-dialog">
          <h2 id="dc-dialog" className="section-title">
            {c('dialog')}
          </h2>
          <div className="card">
            <p className="muted">{c('dialogBody')}</p>
            <Button variant="danger-outline" onClick={() => setDialogOpen(true)}>
              {t('project.delete')}
            </Button>
          </div>
          <Dialog
            open={dialogOpen}
            title={t('project.confirmDeleteLabel')}
            onClose={() => setDialogOpen(false)}
            actions={
              <>
                <Button onClick={() => setDialogOpen(false)}>{t('common.cancel')}</Button>
                <Button variant="danger" onClick={() => setDialogOpen(false)}>
                  {t('project.confirmDeleteButton')}
                </Button>
              </>
            }
          >
            <p>
              <ConfirmDeleteText name={SAMPLE_PROJECTS[1]?.name ?? ''} nameClass="strong" />
            </p>
          </Dialog>
        </section>

        <section className="section" aria-labelledby="dc-money">
          <h2 id="dc-money" className="section-title">
            {c('numbers')}
          </h2>
          <div className="card">
            <Facts
              items={[
                { label: c('budget'), value: formatMicros(200_000) },
                { label: c('spent'), value: formatMicros(41_230) },
              ]}
            />
          </div>
        </section>

        <section className="section" aria-labelledby="dc-shell">
          <h2 id="dc-shell" className="section-title">
            {c('shell')}
          </h2>
          <p>{c('shellBody')}</p>
          <p>
            <a href="/design-lab/open?lab-scenario=projects-list">{c('shellLink')}</a>
          </p>
        </section>
      </div>
    </div>
  );
}

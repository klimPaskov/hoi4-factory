// Four layouts of the focus editor. They share the model, the live game preview and the panels, and differ
// in where the list, the inspector and the problems live.
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, Dialog, Tabs } from '../../components/ui';
import type { VariantProps } from '../rounds';
import { ModelPreview } from './ModelPreview';
import { PROJECTION } from './fixtures';
import { Banners, DiagnosticsList, Inspector, LayoutBadge, NodeList, NodeTable, PendingBar, SearchBox, Toolbar } from './Panels';
import { useFocusModel, type FocusModel, type FocusScenario } from './useFocusModel';
import './focus.css';

function useModel(state: string): FocusModel {
  return useFocusModel(state as FocusScenario);
}

function Title() {
  const { t } = useTranslation();
  return (
    <h1 className="fx-page-title">
      {t('designLab.focus.pageTitle')} <span className="mono muted">{PROJECTION.treeId}</span>
    </h1>
  );
}

/** A. The game render fills the page, the inspector sits on the right, the list is a drawer below. */
export function FocusA({ state }: VariantProps) {
  const { t } = useTranslation();
  const model = useModel(state);
  return (
    <div className="fx-page fxa">
      <Title />
      <Toolbar model={model} />
      <Banners model={model} />
      <PendingBar model={model} />
      <div className="fxa-main">
        <ModelPreview model={model} label={t('designLab.focus.previewLabel')} />
        <aside className="fxa-side" aria-label={t('designLab.focus.inspector.title')}>
          <Inspector model={model} />
        </aside>
      </div>
      <details className="fx-drawer">
        <summary>{t('designLab.focus.drawer', { count: model.nodes.length, problems: model.diagnostics.length })}</summary>
        <div className="fxa-drawer-body">
          <section aria-labelledby="fxa-list">
            <h2 id="fxa-list">{t('designLab.focus.list.title')}</h2>
            <NodeList model={model} />
          </section>
          <section aria-labelledby="fxa-diags">
            <h2 id="fxa-diags">{t('designLab.focus.problems')}</h2>
            <DiagnosticsList model={model} />
          </section>
        </div>
      </details>
    </div>
  );
}

/** B. Three panes at once: the list on the left, the render in the middle, the inspector right. */
export function FocusB({ state }: VariantProps) {
  const { t } = useTranslation();
  const model = useModel(state);
  return (
    <div className="fx-page fxb">
      <Title />
      <Banners model={model} />
      <div className="fxb-grid">
        <aside className="fxb-list" aria-label={t('designLab.focus.list.title')}>
          <SearchBox model={model} />
          <NodeList model={model} />
        </aside>
        <div className="fxb-center">
          <div className="fx-toolbar fxb-bar">
            <LayoutBadge verdict={model.layoutVerdict} />
            <Badge tone={model.diagnostics.length ? 'warning' : 'success'}>{t('designLab.focus.problemCount', { count: model.diagnostics.length })}</Badge>
            <Button size="sm" icon="plus" onClick={model.addFocus} disabled={model.refreshing}>
              {t('designLab.focus.add')}
            </Button>
          </div>
          <PendingBar model={model} />
          <ModelPreview model={model} label={t('designLab.focus.previewLabel')} />
        </div>
        <aside className="fxb-side" aria-label={t('designLab.focus.inspector.title')}>
          <Inspector model={model} />
          <section aria-labelledby="fxb-diags" className="fxb-diags">
            <h2 id="fxb-diags">{t('designLab.focus.problems')}</h2>
            <DiagnosticsList model={model} />
          </section>
        </aside>
      </div>
    </div>
  );
}

type Sheet = 'list' | 'problems' | null;

/** C. The render only, with a small bar. The inspector floats over the render when a focus is selected. */
export function FocusC({ state }: VariantProps) {
  const { t } = useTranslation();
  const model = useModel(state);
  const [sheet, setSheet] = useState<Sheet>(null);
  const problems = model.diagnostics.length;
  return (
    <div className="fx-page fxc">
      <div className="fxc-bar">
        <Title />
        <SearchBox model={model} />
        <LayoutBadge verdict={model.layoutVerdict} />
        <Button size="sm" onClick={() => setSheet('list')}>
          {t('designLab.focus.list.open', { count: model.nodes.length })}
        </Button>
        <Button size="sm" onClick={() => setSheet('problems')}>
          {t('designLab.focus.problemCount', { count: problems })}
        </Button>
        <Button size="sm" icon="plus" onClick={model.addFocus} disabled={model.refreshing}>
          {t('designLab.focus.add')}
        </Button>
      </div>
      <Banners model={model} />
      <PendingBar model={model} />
      <div className="fxc-stage">
        <ModelPreview model={model} label={t('designLab.focus.previewLabel')} className="fxc-preview" />
        {model.selected ? (
          <aside className="fxc-float" aria-label={t('designLab.focus.inspector.title')}>
            <Button size="sm" variant="ghost" className="fxc-close" onClick={() => model.select(null)}>
              {t('designLab.focus.inspector.close')}
            </Button>
            <Inspector model={model} />
          </aside>
        ) : null}
      </div>
      <Dialog open={sheet === 'list'} title={t('designLab.focus.list.title')} onClose={() => setSheet(null)} actions={<Button onClick={() => setSheet(null)}>{t('designLab.review.close')}</Button>}>
        <NodeList
          model={{
            ...model,
            reveal: (id) => {
              model.reveal(id);
              setSheet(null);
            },
          }}
        />
      </Dialog>
      <Dialog open={sheet === 'problems'} title={t('designLab.focus.problems')} onClose={() => setSheet(null)} actions={<Button onClick={() => setSheet(null)}>{t('designLab.review.close')}</Button>}>
        <DiagnosticsList
          model={{
            ...model,
            reveal: (id) => {
              model.reveal(id);
              setSheet(null);
            },
          }}
        />
      </Dialog>
    </div>
  );
}

/** D. A table with the cost editable in the row sits beside the render. */
export function FocusD({ state }: VariantProps) {
  const { t } = useTranslation();
  const model = useModel(state);
  const [tab, setTab] = useState('focuses');
  const [editing, setEditing] = useState(false);
  return (
    <div className="fx-page fxd">
      <Title />
      <Toolbar model={model}>
        <Button size="sm" disabled={!model.selected} onClick={() => setEditing(true)}>
          {t('designLab.focus.editSelected')}
        </Button>
      </Toolbar>
      <Banners model={model} />
      <PendingBar model={model} />
      <div className="fxd-grid">
      <ModelPreview model={model} label={t('designLab.focus.previewLabel')} className="fxd-preview" />
      <Tabs
        label={t('designLab.focus.tabs.label')}
        value={tab}
        onChange={setTab}
        tabs={[
          { id: 'focuses', label: t('designLab.focus.list.title'), panel: <NodeTable model={model} /> },
          { id: 'problems', label: t('designLab.focus.problemCount', { count: model.diagnostics.length }), panel: <DiagnosticsList model={model} /> },
        ]}
      />
      </div>
      <Dialog open={editing} title={t('designLab.focus.inspector.title')} onClose={() => setEditing(false)} actions={<Button onClick={() => setEditing(false)}>{t('designLab.review.close')}</Button>}>
        <Inspector model={model} />
      </Dialog>
    </div>
  );
}

// Four layouts of the focus editor. They share the model, the live game preview and the panels, and differ
// in where the list, the inspector and the problems live. Each shows only what a modder needs to edit.
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '../../components/ui';
import type { VariantProps } from '../rounds';
import { ModelPreview } from './ModelPreview';
import { AddButton, Banners, DiagnosticsList, Inspector, NodeList, NodeTable, PendingBar, SearchBox, Toolbar } from './Panels';
import { useFocusModel, type FocusModel, type FocusScenario } from './useFocusModel';
import './focus.css';

function useModel(state: string): FocusModel {
  return useFocusModel(state as FocusScenario);
}

function Title() {
  const { t } = useTranslation();
  return <h1 className="fx-page-title">{t('designLab.focus.pageTitle')}</h1>;
}

/** The problems, one plain sentence each. Nothing at all when there are none. */
function Problems({ model }: { model: FocusModel }) {
  const { t } = useTranslation();
  if (model.diagnostics.length === 0) return null;
  return (
    <section className="fx-problems" aria-labelledby="fx-problems-title">
      <h2 id="fx-problems-title">{t('designLab.focus.problems')}</h2>
      <DiagnosticsList model={model} />
    </section>
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
          {model.selected ? null : <Problems model={model} />}
        </aside>
      </div>
      <details className="fx-drawer">
        <summary>{t('designLab.focus.list.title')}</summary>
        <div className="fxa-drawer-body">
          <NodeList model={model} />
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
          <AddButton model={model} />
        </aside>
        <div className="fxb-center">
          <PendingBar model={model} />
          <ModelPreview model={model} label={t('designLab.focus.previewLabel')} />
        </div>
        <aside className="fxb-side" aria-label={t('designLab.focus.inspector.title')}>
          <Inspector model={model} />
          <Problems model={model} />
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
  const reveal = (id: string) => {
    model.reveal(id);
    setSheet(null);
  };
  return (
    <div className="fx-page fxc">
      <div className="fxc-bar">
        <Title />
        <SearchBox model={model} />
        <Button size="sm" variant="ghost" onClick={() => setSheet('list')}>
          {t('designLab.focus.list.title')}
        </Button>
        {problems > 0 ? (
          <Button size="sm" variant="ghost" onClick={() => setSheet('problems')}>
            {t('designLab.focus.problemCount', { count: problems })}
          </Button>
        ) : null}
        <AddButton model={model} />
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
        <NodeList model={{ ...model, reveal }} />
      </Dialog>
      <Dialog open={sheet === 'problems'} title={t('designLab.focus.problems')} onClose={() => setSheet(null)} actions={<Button onClick={() => setSheet(null)}>{t('designLab.review.close')}</Button>}>
        <DiagnosticsList model={{ ...model, reveal }} />
      </Dialog>
    </div>
  );
}

/** D. A table with the cost editable in the row sits beside the render. */
export function FocusD({ state }: VariantProps) {
  const { t } = useTranslation();
  const model = useModel(state);
  const [editing, setEditing] = useState(false);
  return (
    <div className="fx-page fxd">
      <Title />
      <Toolbar model={model}>
        {model.selected ? (
          <Button size="sm" onClick={() => setEditing(true)}>
            {t('designLab.focus.editSelected', { name: model.selected.label })}
          </Button>
        ) : null}
      </Toolbar>
      <Banners model={model} />
      <PendingBar model={model} />
      <div className="fxd-grid">
        <ModelPreview model={model} label={t('designLab.focus.previewLabel')} className="fxd-preview" />
        <div className="fxd-side">
          <NodeTable model={model} />
          <Problems model={model} />
        </div>
      </div>
      <Dialog open={editing} title={t('designLab.focus.inspector.title')} onClose={() => setEditing(false)} actions={<Button onClick={() => setEditing(false)}>{t('designLab.review.close')}</Button>}>
        <Inspector model={model} />
      </Dialog>
    </div>
  );
}

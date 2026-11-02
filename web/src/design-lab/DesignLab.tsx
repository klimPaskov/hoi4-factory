// The design lab: development-only previews the maintainer chooses from (ADR-025). App.tsx adds
// this route only when import.meta.env.DEV is true, so none of it reaches the production build.
import type { ComponentType } from 'react';
import { Link, Route, Routes, useParams, useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import i18n from '../i18n';
import { usePageTitle } from '../hooks';
import { DossierDirection } from './directions/Dossier';
import { StudioDirection } from './directions/Studio';
import { WarRoomDirection } from './directions/WarRoom';
import { WorkbenchDirection } from './directions/Workbench';
import labStrings from './lab.en.json';
import { LAB_STATES, useLabTheme, type DirectionProps, type LabState } from './shared';
import './lab.css';

// Lab text lives in its own file, merged into the translations only in development.
i18n.addResourceBundle('en', 'translation', labStrings, true, false);

interface Variant {
  id: string;
  Component: ComponentType<DirectionProps>;
}

const DIRECTIONS: Variant[] = [
  { id: 'workbench', Component: WorkbenchDirection },
  { id: 'dossier', Component: DossierDirection },
  { id: 'warroom', Component: WarRoomDirection },
  { id: 'studio', Component: StudioDirection },
];

const WIDTHS = ['full', 'phone', 'both'] as const;
type Width = (typeof WIDTHS)[number];

function LabIndex() {
  const { t } = useTranslation();
  usePageTitle(t('designLab.title'));
  return (
    <div className="lab-page">
      <h1>{t('designLab.title')}</h1>
      <p className="lab-note">{t('designLab.devOnly')}</p>
      <h2>{t('designLab.rounds')}</h2>
      <section className="lab-round" aria-labelledby="lab-round-1">
        <h3 id="lab-round-1">{t('designLab.round1.title')}</h3>
        <p>{t('designLab.round1.body')}</p>
        <ul className="lab-cards">
          {DIRECTIONS.map(({ id }) => (
            <li key={id} className="lab-card">
              <h4>{t(`designLab.directions.${id}.name`)}</h4>
              <p>{t(`designLab.directions.${id}.summary`)}</p>
              <Link to={`direction/${id}`} className="lab-button">
                {t('designLab.open', { name: t(`designLab.directions.${id}.name`) })}
              </Link>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

function Toolbar({ current, name }: { current: string; name: string }) {
  const { t } = useTranslation();
  const [params, setParams] = useSearchParams();
  const [theme, setTheme] = useLabTheme();
  const state = params.get('state') ?? 'populated';
  const width = params.get('width') ?? 'full';

  function set(key: string, value: string | null) {
    const next = new URLSearchParams(params);
    if (value === null) next.delete(key);
    else next.set(key, value);
    setParams(next, { replace: true });
  }

  return (
    <div className="lab-bar" role="region" aria-label={t('designLab.toolbar.label')}>
      <Link to="/design-lab" className="lab-home">
        {t('designLab.toolbar.home')}
      </Link>
      <fieldset className="lab-group">
        <legend>{t('designLab.toolbar.direction')}</legend>
        {DIRECTIONS.map(({ id }) => (
          <Link
            key={id}
            to={{ pathname: `/design-lab/direction/${id}`, search: params.toString() }}
            aria-current={id === current ? 'page' : undefined}
            className="lab-chip"
          >
            {t(`designLab.directions.${id}.name`)}
          </Link>
        ))}
      </fieldset>
      <fieldset className="lab-group">
        <legend>{t('designLab.toolbar.state')}</legend>
        {LAB_STATES.map((value) => (
          <label key={value} className="lab-chip">
            <input type="radio" name="lab-state" checked={state === value} onChange={() => set('state', value)} />
            {t(`designLab.toolbar.states.${value}`)}
          </label>
        ))}
        <label className="lab-chip">
          <input
            type="checkbox"
            checked={params.get('unverified') === '1'}
            onChange={(event) => set('unverified', event.target.checked ? '1' : null)}
          />
          {t('designLab.toolbar.unverified')}
        </label>
      </fieldset>
      <fieldset className="lab-group">
        <legend>{t('layout.theme')}</legend>
        {(['system', 'light', 'dark'] as const).map((value) => (
          <label key={value} className="lab-chip">
            <input type="radio" name="lab-theme" checked={theme === value} onChange={() => setTheme(value)} />
            {t(`layout.theme${value[0]?.toUpperCase()}${value.slice(1)}`)}
          </label>
        ))}
      </fieldset>
      <fieldset className="lab-group">
        <legend>{t('designLab.toolbar.width')}</legend>
        {WIDTHS.map((value) => (
          <label key={value} className="lab-chip">
            <input type="radio" name="lab-width" checked={width === value} onChange={() => set('width', value)} />
            {t(`designLab.toolbar.widths.${value}`)}
          </label>
        ))}
      </fieldset>
      <span className="sr-only">{name}</span>
    </div>
  );
}

function frameSource(params: URLSearchParams): string {
  const next = new URLSearchParams(params);
  next.delete('width');
  next.set('frame', '1');
  return `${window.location.pathname}?${next.toString()}`;
}

function DirectionPage() {
  const { t } = useTranslation();
  const { id = '' } = useParams();
  const [params] = useSearchParams();
  const variant = DIRECTIONS.find((d) => d.id === id);
  const name = variant ? t(`designLab.directions.${variant.id}.name`) : t('designLab.title');
  usePageTitle(`${name} · ${t('designLab.title')}`);
  if (!variant) {
    return (
      <div className="lab-page">
        <p>{t('designLab.notFound')}</p>
        <Link to="/design-lab">{t('designLab.toolbar.home')}</Link>
      </div>
    );
  }

  const stateParam = params.get('state');
  const state: LabState = LAB_STATES.includes(stateParam as LabState) ? (stateParam as LabState) : 'populated';
  const unverified = params.get('unverified') === '1';
  const width = (WIDTHS as readonly string[]).includes(params.get('width') ?? '') ? (params.get('width') as Width) : 'full';
  // Remount on every state change so each state starts from its own sample data.
  const view = <variant.Component key={`${variant.id}-${state}-${unverified}`} state={state} unverified={unverified} />;

  if (params.get('frame') === '1') return view;

  const source = frameSource(params);
  return (
    <>
      <Toolbar current={variant.id} name={name} />
      {width === 'full' ? view : null}
      {width !== 'full' ? (
        <div className="lab-frames">
          {width === 'both' ? (
            <div className="lab-desktop">
              <iframe src={source} title={t('designLab.toolbar.desktopFrame', { name })} width="1280" height="860" />
            </div>
          ) : null}
          <iframe className="lab-phone" src={source} title={t('designLab.toolbar.phoneFrame', { name })} width="375" height="812" />
        </div>
      ) : null}
    </>
  );
}

export default function DesignLab() {
  return (
    <Routes>
      <Route index element={<LabIndex />} />
      <Route path="direction/:id" element={<DirectionPage />} />
      <Route path="*" element={<LabIndex />} />
    </Routes>
  );
}

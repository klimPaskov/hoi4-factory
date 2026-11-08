// The design lab: development-only previews the maintainer chooses from (ADR-025). App.tsx adds
// this route only when import.meta.env.DEV is true, so none of it reaches the production build.
// It holds the approved design system, the real screens on sample data, and the open rounds:
// three or four working variants of one page each, removed once the maintainer has chosen.
import { Link, Route, Routes, useParams, useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import i18n from '../i18n';
import { usePageTitle } from '../hooks';
import { ComponentsPage } from './Components';
import labStrings from './lab.en.json';
import { HistoryPage, ReviewPage } from './Review';
import { ROUNDS } from './rounds';
import { SCENARIO_GROUPS, SCENARIOS } from './scenarios';
import { useLabTheme } from './shared';
import './lab.css';

// Lab text lives in its own file, merged into the translations only in development.
i18n.addResourceBundle('en', 'translation', labStrings, true, false);

const WIDTHS = ['full', 'phone', 'both'] as const;
type Width = (typeof WIDTHS)[number];

function Screens() {
  const { t } = useTranslation();
  usePageTitle(t('designLab.screens.title'));
  return (
    <div className="lab-page">
      <p>
        <Link to="/design-lab">{t('designLab.toolbar.home')}</Link>
      </p>
      <h1>{t('designLab.screens.title')}</h1>
      <p className="lab-note">{t('designLab.screens.body')}</p>
      {SCENARIO_GROUPS.map((group) => (
        <section key={group} aria-labelledby={`lab-group-${group}`}>
          <h2 id={`lab-group-${group}`}>{t(`designLab.screens.groups.${group}`)}</h2>
          <ul className="lab-scenarios">
            {Object.entries(SCENARIOS)
              .filter(([, scenario]) => scenario.group === group)
              .map(([name]) => (
                <li key={name}>
                  <a href={`/design-lab/open?lab-scenario=${name}`}>{name}</a>
                </li>
              ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

function Toolbar({ roundId, current }: { roundId: string; current: string }) {
  const { t } = useTranslation();
  const [params, setParams] = useSearchParams();
  const [theme, setTheme] = useLabTheme();
  const round = ROUNDS.find((r) => r.id === roundId);
  const state = params.get('state') ?? round?.states[0] ?? '';
  const width = params.get('width') ?? 'full';
  if (!round) return null;

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
        <legend>{t('designLab.toolbar.variant')}</legend>
        {round.variants.map(({ id }) => (
          <Link
            key={id}
            to={{ pathname: `/design-lab/round/${round.id}/${id}`, search: params.toString() }}
            aria-current={id === current ? 'page' : undefined}
            className="lab-chip"
          >
            {t(`designLab.round.${round.id}.variants.${id}.name`)}
          </Link>
        ))}
      </fieldset>
      <fieldset className="lab-group">
        <legend>{t('designLab.toolbar.state')}</legend>
        {round.states.map((value) => (
          <label key={value} className="lab-chip">
            <input type="radio" name="lab-state" checked={state === value} onChange={() => set('state', value)} />
            {t(`designLab.round.${round.id}.states.${value}`)}
          </label>
        ))}
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
    </div>
  );
}

function frameSource(params: URLSearchParams): string {
  const next = new URLSearchParams(params);
  next.delete('width');
  next.set('frame', '1');
  return `${window.location.pathname}?${next.toString()}`;
}

function VariantPage() {
  const { t } = useTranslation();
  const { round: roundId = '', variant: variantId = '' } = useParams();
  const [params] = useSearchParams();
  const round = ROUNDS.find((r) => r.id === roundId);
  const variant = round?.variants.find((v) => v.id === variantId);
  const name = variant ? t(`designLab.round.${roundId}.variants.${variantId}.name`) : t('designLab.title');
  usePageTitle(`${name} · ${t('designLab.title')}`);
  if (!round || !variant) {
    return (
      <div className="lab-page">
        <p>{t('designLab.gone')}</p>
        <Link to="/design-lab">{t('designLab.toolbar.home')}</Link>
      </div>
    );
  }

  const stateParam = params.get('state') ?? '';
  const state = round.states.includes(stateParam) ? stateParam : (round.states[0] ?? '');
  const width = (WIDTHS as readonly string[]).includes(params.get('width') ?? '') ? (params.get('width') as Width) : 'full';
  // Remount on every state change so each state starts from its own sample data.
  const view = <variant.Component key={`${variant.id}-${state}`} state={state} />;

  if (params.get('frame') === '1') return view;

  const source = frameSource(params);
  return (
    <>
      <Toolbar roundId={round.id} current={variant.id} />
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
      <Route index element={<ReviewPage />} />
      <Route path="components" element={<ComponentsPage />} />
      <Route path="history" element={<HistoryPage />} />
      <Route path="screens" element={<Screens />} />
      <Route path="round/:round/:variant" element={<VariantPage />} />
      <Route path="*" element={<ReviewPage />} />
    </Routes>
  );
}

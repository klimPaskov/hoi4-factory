// The generated reference mockups (docs/design/mockups): what each page should look like before
// it is built. Every page round starts from its mockup.
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { usePageTitle } from '../hooks';

const IMAGES = import.meta.glob('./mockups/*.webp', { eager: true, import: 'default', query: '?url' }) as Record<string, string>;

export function MockupsPage() {
  const { t } = useTranslation();
  usePageTitle(t('designLab.mockups.title'));
  const entries = Object.entries(IMAGES)
    .map(([path, url]) => ({ name: path.replace('./mockups/', '').replace('.webp', ''), url }))
    .sort((a, b) => a.name.localeCompare(b.name));
  return (
    <div className="lab-page lab-wide">
      <p>
        <Link to="/design-lab">{t('designLab.toolbar.home')}</Link>
      </p>
      <h1>{t('designLab.mockups.title')}</h1>
      <p className="lab-note">{t('designLab.mockups.body')}</p>
      <ul className="lab-mockups">
        {entries.map(({ name, url }) => (
          <li key={name}>
            <h2>{t(`designLab.mockups.items.${name}`)}</h2>
            <a href={url}>
              <img src={url} alt={t(`designLab.mockups.items.${name}`)} loading="lazy" />
            </a>
          </li>
        ))}
      </ul>
    </div>
  );
}

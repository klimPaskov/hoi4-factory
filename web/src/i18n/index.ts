import i18next from 'i18next';
import { initReactI18next } from 'react-i18next';
import en from './locales/en.json';

// English only at launch (ADR-021). Every user-visible string lives in a locale file, so adding
// a language means adding a file here and listing it in `resources`.
void i18next.use(initReactI18next).init({
  lng: 'en',
  fallbackLng: 'en',
  resources: { en: { translation: en } },
  // React escapes interpolated values itself.
  interpolation: { escapeValue: false },
  returnNull: false,
});

export default i18next;

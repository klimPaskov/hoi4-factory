import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router';
import { I18nextProvider } from 'react-i18next';
import { AppRoutes } from './App';
import { AuthProvider } from './auth';
import { applyTheme, readTheme } from './hooks';
import i18n from './i18n';
import './styles.css';

applyTheme(readTheme());

const container = document.getElementById('root');
if (!container) throw new Error('Missing #root element');

createRoot(container).render(
  <StrictMode>
    <I18nextProvider i18n={i18n}>
      <BrowserRouter>
        <AuthProvider>
          <AppRoutes />
        </AuthProvider>
      </BrowserRouter>
    </I18nextProvider>
  </StrictMode>,
);

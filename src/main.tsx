import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { HashRouter } from 'react-router';
import App from './App';
import { CheckRunProvider } from './features/check-run/CheckRunProvider';
import './index.css';

const queryClient = new QueryClient();

const rootElement = document.getElementById('root');
if (!rootElement) {
  throw new Error('Root element #root not found');
}

createRoot(rootElement).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <CheckRunProvider>
        <HashRouter>
          <App />
        </HashRouter>
      </CheckRunProvider>
    </QueryClientProvider>
  </StrictMode>,
);

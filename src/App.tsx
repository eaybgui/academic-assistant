import { Route, Routes } from 'react-router';
import Layout from './components/Layout';
import DashboardPage from './features/dashboard/DashboardPage';
import ResultsPage from './features/results/ResultsPage';
import RunsPage from './features/runs/RunsPage';
import AccountsPage from './features/accounts/AccountsPage';
import CatalogPage from './features/catalog/CatalogPage';
import SettingsPage from './features/settings/SettingsPage';

export default function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<DashboardPage />} />
        <Route path="/results" element={<ResultsPage />} />
        <Route path="/runs" element={<RunsPage />} />
        <Route path="/accounts" element={<AccountsPage />} />
        <Route path="/catalog" element={<CatalogPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Routes>
    </Layout>
  );
}

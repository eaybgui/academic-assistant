import { Route, Routes } from 'react-router';
import Layout from './components/Layout';
import TrackerPage from './features/tracker/TrackerPage';
import CalendarPage from './features/calendar/CalendarPage';
import AnnouncementsPage from './features/announcements/AnnouncementsPage';
import RunsPage from './features/runs/RunsPage';
import AccountsPage from './features/accounts/AccountsPage';
import UniversitiesPage from './features/universities/UniversitiesPage';
import SettingsPage from './features/settings/SettingsPage';

export default function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<TrackerPage />} />
        <Route path="/calendar" element={<CalendarPage />} />
        <Route path="/announcements" element={<AnnouncementsPage />} />
        <Route path="/runs" element={<RunsPage />} />
        <Route path="/accounts" element={<AccountsPage />} />
        <Route path="/universities" element={<UniversitiesPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Routes>
    </Layout>
  );
}

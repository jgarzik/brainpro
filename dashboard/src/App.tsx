import { BrowserRouter, Routes, Route } from "react-router-dom";
import { AppShell } from "@/components/layout";
import { ROUTES } from "@/constants/routes";
import { ErrorBoundary } from "@/components/ErrorBoundary";

// Lazy load pages
import { lazy, Suspense } from "react";
import { PageSpinner } from "@/components/ui/Spinner";

const ChatPage = lazy(() => import("@/pages/ChatPage"));
const OverviewPage = lazy(() => import("@/pages/OverviewPage"));
const SessionsPage = lazy(() => import("@/pages/SessionsPage"));
const SessionDetailPage = lazy(() => import("@/pages/SessionDetailPage"));
const ToolsPage = lazy(() => import("@/pages/ToolsPage"));
const SkillsPage = lazy(() => import("@/pages/SkillsPage"));
const AgentsPage = lazy(() => import("@/pages/AgentsPage"));
const ChannelsPage = lazy(() => import("@/pages/ChannelsPage"));
const HealthPage = lazy(() => import("@/pages/HealthPage"));
const EventsPage = lazy(() => import("@/pages/EventsPage"));
const CostsPage = lazy(() => import("@/pages/CostsPage"));
const ConfigPage = lazy(() => import("@/pages/ConfigPage"));
const DebugPage = lazy(() => import("@/pages/DebugPage"));

function App() {
  return (
    <ErrorBoundary>
      <BrowserRouter>
        <Routes>
          <Route element={<AppShell />}>
          <Route
            path={ROUTES.CHAT}
            element={
              <Suspense fallback={<PageSpinner />}>
                <ChatPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.OVERVIEW}
            element={
              <Suspense fallback={<PageSpinner />}>
                <OverviewPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.SESSIONS}
            element={
              <Suspense fallback={<PageSpinner />}>
                <SessionsPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.SESSION_DETAIL}
            element={
              <Suspense fallback={<PageSpinner />}>
                <SessionDetailPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.TOOLS}
            element={
              <Suspense fallback={<PageSpinner />}>
                <ToolsPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.SKILLS}
            element={
              <Suspense fallback={<PageSpinner />}>
                <SkillsPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.AGENTS}
            element={
              <Suspense fallback={<PageSpinner />}>
                <AgentsPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.CHANNELS}
            element={
              <Suspense fallback={<PageSpinner />}>
                <ChannelsPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.HEALTH}
            element={
              <Suspense fallback={<PageSpinner />}>
                <HealthPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.EVENTS}
            element={
              <Suspense fallback={<PageSpinner />}>
                <EventsPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.COSTS}
            element={
              <Suspense fallback={<PageSpinner />}>
                <CostsPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.CONFIG}
            element={
              <Suspense fallback={<PageSpinner />}>
                <ConfigPage />
              </Suspense>
            }
          />
          <Route
            path={ROUTES.DEBUG}
            element={
              <Suspense fallback={<PageSpinner />}>
                <DebugPage />
              </Suspense>
            }
          />
          </Route>
        </Routes>
      </BrowserRouter>
    </ErrorBoundary>
  );
}

export default App;

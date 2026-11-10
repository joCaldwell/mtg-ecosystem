import { lazy, Suspense } from "react";
import { Loading, ToastProvider } from "../ui/index.ts";
import { ErrorBoundary } from "./ErrorBoundary.tsx";
import { SessionProvider } from "./Session.tsx";
import { CardProviders } from "./CardProviders.tsx";
import { isPublicRoute, useRoute } from "./routes.ts";
const AppProviders = lazy(() => import("./Providers.tsx").then(m => ({ default: m.AppProviders })));
const SignedIn = lazy(() => import("./SignedIn.tsx").then(m => ({ default: m.SignedIn })));
const PublicDeckPage = lazy(() => import("../features/public/PublicDeckPage.tsx").then(m => ({ default: m.PublicDeckPage })));
const PublicProfilePage = lazy(() => import("../features/public/PublicProfilePage.tsx").then(m => ({ default: m.PublicProfilePage })));
export function App() {
  const route = useRoute();
  const open = isPublicRoute(route);
  return (
    <ErrorBoundary><ToastProvider><a className="skip-link" href="#main-content" onClick={(e) => { e.preventDefault(); document.getElementById("main-content")?.focus(); }}>Skip to main content</a><Suspense fallback={<Loading />}>
      <SessionProvider open={open}>
        {route.page === "shared" ? <CardProviders><PublicDeckPage slug={route.slug} key={route.slug} /></CardProviders>
          : route.page === "user" ? <CardProviders><PublicProfilePage handle={route.handle} key={route.handle} /></CardProviders>
          : <AppProviders><SignedIn route={route} /></AppProviders>}
      </SessionProvider>
    </Suspense></ToastProvider></ErrorBoundary>
  );
}

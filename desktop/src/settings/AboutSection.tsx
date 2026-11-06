import { useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button } from "@/components/ui/button";
import {
  checkForUpdate,
  UPDATE_CHECK_KEY,
  type UpdateCheckError,
  type UpdateStatus,
} from "./updateCheck";

// Fase 6.5 — versão do app e checagem manual de atualização. A mesma query
// (`UPDATE_CHECK_KEY`) alimenta o indicador discreto do `App.tsx`.
function AboutSection() {
  const query = useQuery<UpdateStatus, UpdateCheckError>({
    queryKey: UPDATE_CHECK_KEY,
    queryFn: checkForUpdate,
    staleTime: Infinity,
    retry: false,
  });

  const status = query.data;
  let message = "Not checked yet.";
  if (query.isFetching) {
    message = "Checking…";
  } else if (query.isError) {
    message = "Could not check for updates. Try again later.";
  } else if (status?.update_available) {
    message = `Version ${status.latest} is available.`;
  } else if (status && status.latest === null) {
    message = "No releases published yet.";
  } else if (status) {
    message = "You are on the latest version.";
  }

  return (
    <section className="flex flex-col gap-3" data-testid="about-section">
      <h2 className="text-lg font-semibold">About</h2>
      <p className="text-sm">
        Current version: <span data-testid="about-version">{status?.current ?? "…"}</span>
      </p>
      <p className="text-sm text-muted-foreground" data-testid="about-update-message">
        {message}
      </p>
      <div className="flex gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          data-testid="check-for-updates"
          disabled={query.isFetching}
          onClick={() => query.refetch()}
        >
          Check for updates
        </Button>
        {status?.update_available && status.release_url && (
          <Button
            type="button"
            size="sm"
            data-testid="open-release"
            onClick={() => openUrl(status.release_url as string)}
          >
            View release
          </Button>
        )}
      </div>
      <p className="text-xs text-muted-foreground">
        Checking contacts api.github.com once when the app opens and whenever you press the button.
        Nothing is downloaded or installed.
      </p>
    </section>
  );
}

export default AboutSection;

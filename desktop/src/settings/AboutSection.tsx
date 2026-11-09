import { useMutation, useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button } from "@/components/ui/button";
import type { AppError } from "../types";
import {
  checkForUpdate,
  downloadUpdate,
  installUpdate,
  UPDATE_CHECK_KEY,
  type DownloadedUpdate,
  type UpdateCheckError,
  type UpdateStatus,
} from "./updateCheck";

// Fase 6.5 / P9 — versão do app, checagem manual de atualização e instalação por
// botão. A mesma query (`UPDATE_CHECK_KEY`) alimenta o indicador discreto do
// `App.tsx`. Nada é baixado nem instalado sem o clique do usuário.
function AboutSection() {
  const query = useQuery<UpdateStatus, UpdateCheckError>({
    queryKey: UPDATE_CHECK_KEY,
    queryFn: checkForUpdate,
    staleTime: Infinity,
    retry: false,
  });
  const download = useMutation<DownloadedUpdate, AppError, void>({
    mutationFn: downloadUpdate,
  });
  const install = useMutation<void, AppError, void>({
    mutationFn: installUpdate,
  });

  const status = query.data;
  let message = "Not checked yet.";
  if (query.isFetching) {
    message = "Checking…";
  } else if (query.isError) {
    message = "Could not check for updates. Try again later.";
  } else if (download.isPending) {
    message = `Downloading version ${status?.latest}…`;
  } else if (download.isSuccess) {
    message = `Version ${download.data.version} downloaded and its signature verified.`;
  } else if (status?.update_available) {
    message = `Version ${status.latest} is available.`;
  } else if (status && status.latest === null) {
    message = "No releases published yet.";
  } else if (status) {
    message = "You are on the latest version.";
  }

  const failure = download.error ?? install.error;

  function checkAgain() {
    download.reset();
    install.reset();
    query.refetch();
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
      {failure && (
        <p className="text-sm text-red-600" data-testid="about-update-error">
          Could not install the update: {failure.message}
        </p>
      )}
      <div className="flex gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          data-testid="check-for-updates"
          disabled={query.isFetching || download.isPending || install.isPending}
          onClick={checkAgain}
        >
          Check for updates
        </Button>
        {status?.update_available && !download.isSuccess && (
          <Button
            type="button"
            size="sm"
            data-testid="download-update"
            disabled={query.isFetching || download.isPending}
            onClick={() => download.mutate()}
          >
            Download update
          </Button>
        )}
        {download.isSuccess && (
          <Button
            type="button"
            size="sm"
            data-testid="install-update"
            disabled={install.isPending}
            onClick={() => install.mutate()}
          >
            {install.isPending ? "Restarting…" : "Restart to install"}
          </Button>
        )}
        {status?.update_available && status.release_url && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            data-testid="open-release"
            onClick={() => openUrl(status.release_url as string)}
          >
            View release
          </Button>
        )}
      </div>
      <p className="text-xs text-muted-foreground">
        Checking reads latest.json from the project&apos;s GitHub Releases once when the app opens
        and whenever you press the button. An update is only downloaded when you ask, and it is
        installed only if its signature verifies against the key built into this app.
      </p>
    </section>
  );
}

export default AboutSection;

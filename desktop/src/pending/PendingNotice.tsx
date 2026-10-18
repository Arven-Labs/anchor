import { useOpenPending, type PendingFocus } from "./PendingNav";
import { Button } from "@/components/ui/button";

// Aviso que substitui o cartão de pendentes no Ledger e em Finances: a fila mora na
// aba Pending, aqui fica só a contagem e o link.
function PendingNotice({
  count,
  focus,
  testId,
}: {
  count: number;
  focus: PendingFocus;
  testId: string;
}) {
  const openPending = useOpenPending();
  if (count === 0) return null;
  return (
    <div
      className="flex items-center justify-between gap-3 rounded-lg border border-border/80 bg-muted/40 px-4 py-3 text-sm"
      data-testid={testId}
    >
      <span>{count} item(s) waiting for review in Pending.</span>
      <Button type="button" size="sm" data-testid={`${testId}-open`} onClick={() => openPending(focus)}>
        Open Pending
      </Button>
    </div>
  );
}

export default PendingNotice;

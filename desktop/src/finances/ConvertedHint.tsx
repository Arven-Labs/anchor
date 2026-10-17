// Fase 19 — valor de um item em moeda diferente da base do Workspace: mostra a
// moeda dele e o equivalente na base ao lado, ou o aviso de câmbio faltando. Não
// renderiza nada quando a moeda já é a base, então o texto das células em BRL
// (e os E2E que o leem) fica como era.
function ConvertedHint({
  currency,
  baseCurrency,
  inBase,
}: {
  currency: string;
  baseCurrency: string | undefined;
  inBase: number | null;
}) {
  if (!baseCurrency || currency === baseCurrency) return null;
  return (
    <span className="ml-1 text-xs text-muted-foreground" data-testid="converted-hint">
      {currency} · {inBase != null ? `≈ ${inBase.toFixed(2)} ${baseCurrency}` : `no ${currency}/${baseCurrency} rate`}
    </span>
  );
}

export default ConvertedHint;

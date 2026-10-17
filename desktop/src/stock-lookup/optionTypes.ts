// Tipos da cadeia de opções (comandos `list_option_chain`, Sessão 114), usados
// por `OptionsSection.tsx` e pelo simulador de payoff (Sessão 115).

export type OptionLeg = {
  series_ticker: string;
  last_price: number | null;
  last_trade_date: string | null;
  american: boolean;
};

export type StrikeRow = { strike: number; call: OptionLeg | null; put: OptionLeg | null };

export type Expiration = { expiration: string; days_to_expiry: number; rows: StrikeRow[] };

export type OptionChain = {
  root: string;
  stale: boolean;
  fetched_at: string | null;
  expirations: Expiration[];
};

export type Workspace = {
  id: number;
  name: string;
  created_at: string;
};

export type Portfolio = {
  id: number;
  workspace_id: number;
  name: string;
  description: string | null;
  created_at: string;
};

export type Custodia = {
  id: number;
  workspace_id: number;
  instituicao: string;
  titular: string;
  created_at: string;
};

// Fase 10.2, escopo Sessão 29 — REIT/imóvel/empresa não listada (Sessão 30)
// ficam pra uma fatia futura. `fii` entrou na Sessão 41, `etf_br`
// na Sessão 49 — só a fatia de cotação automática por ora, sem indicador de
// fundo específico (CVM não tem categoria de dados abertos pra fundo de
// índice, diferente de FII, achado pesquisando de verdade — ver PHASE.md
// item 8). `cripto` entrou na Sessão 51, unificando a antiga tela solta
// "Crypto Score" (indicadores de ciclo, ainda existe — ver
// CryptoLookupSection.tsx) dentro de Research/Ativos como classe de ativo de
// verdade, com cotação própria via CoinGecko. `bdr` entrou na Sessão 53 —
// recibo B3 de empresa estrangeira (ex.: AAPL34), mesmo endpoint Yahoo
// `.SA` de acao_br/fii/etf_br (confirmado ao vivo), zero coletor novo;
// exposição default "US" (editável), diferente das outras classes B3 que
// default "BR" — um BDR representa empresa estrangeira, quase nunca
// Brasil. `metal` entrou na Sessão 55 — só ouro (`XAU`) por ora, cotação
// via Yahoo sem `.SA` (COMEX, não é listada na B3), preço já convertido pra
// USD/grama na fonte; exposição default `categoria_especial`/`gold_metal`
// (nem país nem "BR"/"US" fazem sentido pra metal). `acao_internacional`
// ganhou cotação automática numa fatia 1 (Fatia 2, fundamentos via SEC
// EDGAR, fica pra depois) — mesmo endpoint Yahoo do resto, sem sufixo
// (ticker puro, ex.: AAPL); exposição default "US", mesmo padrão do BDR.
// `reit` (equivalente americano do FII) — mesma fonte Yahoo de cotação que
// `acao_internacional`, mas indicadores imobiliários próprios via SEC EDGAR
// (`reit_fundamentals`/`reit_manual_indicators`, não `stock_fundamentals`/
// `stock_dcf_fundamentals` — sem os 8 modelos de valuation, que não
// encaixam bem em imobiliário). `etf_us` (ETF listado nos EUA, ex.: SPY) —
// mesma fonte Yahoo sem `.SA` de `acao_internacional`/`reit`, sem
// fundamentos nenhum (nem os 8 modelos nem indicador de fundo dedicado,
// mesmo motivo do `etf_br`: ETF não tem demonstração financeira própria).
// `imovel` e `empresa_nao_listada` entraram na Sessão 64 — cadastro manual
// (mesmo caminho de `tesouro_direto`/`renda_fixa`, fora de
// `ASSET_CLASSES_WITH_AUTO_QUOTE`), compartilhando o esqueleto `AtivoManual`
// do rascunho original (histórico de avaliações + anexos, ver
// AssetValuation/AssetAttachment abaixo e `ManualAssetDetails.tsx`).
// `empresa_nao_listada` ganha ainda os 3 campos de participação societária
// em `Asset` abaixo (`equity_*`) — sem coluna própria de percentual, sempre
// calculado a partir de `equity_shares_owned`/`equity_total_shares`.
// Ver commands/asset.rs::ASSET_CLASSES.
export type AssetClass =
  | "acao_br"
  | "fii"
  | "etf_br"
  | "cripto"
  | "bdr"
  | "metal"
  | "acao_internacional"
  | "reit"
  | "etf_us"
  | "tesouro_direto"
  | "renda_fixa"
  | "imovel"
  | "empresa_nao_listada";

export const ASSET_CLASSES: AssetClass[] = [
  "acao_br",
  "fii",
  "etf_br",
  "cripto",
  "bdr",
  "metal",
  "acao_internacional",
  "reit",
  "etf_us",
  "tesouro_direto",
  "renda_fixa",
  "imovel",
  "empresa_nao_listada",
];

export const ASSET_CLASS_LABELS: Record<AssetClass, string> = {
  acao_br: "Stock (B3)",
  fii: "FII (B3)",
  etf_br: "ETF (B3)",
  cripto: "Crypto",
  bdr: "BDR (B3)",
  metal: "Metal",
  acao_internacional: "International stock",
  reit: "REIT",
  etf_us: "ETF (US)",
  tesouro_direto: "Tesouro Direto",
  renda_fixa: "Fixed income",
  imovel: "Real estate",
  empresa_nao_listada: "Private company",
};

// Classes que usam os campos fi_* (renda fixa detalhada) na transação de compra.
export const FIXED_INCOME_CLASSES: AssetClass[] = ["tesouro_direto", "renda_fixa"];

// Classes com busca automática de cotação por ticker — usada por
// AssetSection.tsx (form vira busca) e Research (StockLookupPanel.tsx,
// dispatch por classe). Ação BR/FII/ETF/BDR compartilham o mesmo endpoint
// Yahoo `{ticker}.SA`; cripto usa CoinGecko e metal usa Yahoo sem `.SA`
// (fontes diferentes, ver commands/collector.rs::run_stock_collector) —
// daqui pra baixo o front não distingue a fonte, só o backend sabe rotear.
export const ASSET_CLASSES_WITH_AUTO_QUOTE: AssetClass[] = [
  "acao_br",
  "fii",
  "etf_br",
  "cripto",
  "bdr",
  "metal",
  "acao_internacional",
  "reit",
  "etf_us",
];

export type ExposureType = "pais" | "categoria_especial";

// Fase 10, item 8, Sessão 86 — subconjunto de AssetClass que faz sentido
// como "o que um BDR realmente representa por trás" (pedido registrado
// desde a Sessão 53: um BDR pode ser ação, ETF, REIT, metal ou cripto, não
// só empresa operacional). Reaproveita ASSET_CLASS_LABELS pros rótulos do
// Select em vez de um enum/label próprios — ver
// commands::asset::BDR_UNDERLYING_CLASSES (mesma lista, backend valida).
export const BDR_UNDERLYING_CLASSES: AssetClass[] = [
  "acao_internacional",
  "etf_us",
  "reit",
  "metal",
  "cripto",
];

export type Asset = {
  id: number;
  ticker: string;
  name: string;
  asset_class: string;
  currency: string;
  exchange: string | null;
  exposure_type: string;
  exposure_value: string;
  created_at: string;
  cnpj: string | null;
  equity_shares_owned: number | null;
  equity_total_shares: number | null;
  equity_company_valuation: number | null;
  underlying_asset_class: string | null;
};

// Fase 10, item 8, Sessão 41 — indicadores de FII direto da CVM (dados
// abertos), priorizados sobre a bolsai por pedido explícito do dono do
// projeto. `cnpj` é a chave de junção com `assets.cnpj` (não um ID interno
// dessas tabelas). Ver commands/fii.rs.
export type FiiCnpjSuggestion = {
  cnpj: string;
  fund_name: string;
};

export type FiiCvmMonthly = {
  id: number;
  cnpj: string;
  reference_date: string;
  patrimonio_liquido: number;
  valor_patrimonial_cota: number;
  numero_cotistas: number | null;
  dividend_yield_mes: number | null;
  rentabilidade_efetiva_mes: number | null;
  source: string;
  fetched_at: string;
};

export type FiiCvmProperty = {
  id: number;
  cnpj: string;
  reference_date: string;
  nome_imovel: string;
  endereco: string | null;
  area_m2: number | null;
  percentual_vacancia: number | null;
  percentual_inadimplencia: number | null;
  percentual_receitas_fii: number | null;
  percentual_locado: number | null;
  source: string;
  fetched_at: string;
};

// Fase 10, item 8 — indicadores imobiliários de REIT via SEC EDGAR, tabela
// própria (não stock_fundamentals/stock_dcf_fundamentals — REIT não usa os
// 8 modelos de valuation). Time series (mesmo padrão de StockFundamentals),
// leitura recente-primeiro filtrada por ticker no client. Campos nullable
// refletem taxonomia XBRL inconsistente entre REITs (ex.: Simon Property
// não reporta NetIncomeLoss). Ver commands/reit.rs.
export type ReitFundamentals = {
  id: number;
  ticker: string;
  reference_year: number;
  revenue: number;
  real_estate_property_net: number | null;
  real_estate_property_at_cost: number | null;
  stockholders_equity: number;
  net_income: number | null;
  eps_diluted: number;
  source: string;
  fetched_at: string;
};

// FFO/AFFO por ação e taxa de ocupação — confirmado ao vivo que não
// existem como tag XBRL (métricas non-GAAP só em texto/tabela do 10-K) —
// campo manual editável, mesmo espírito do landbank do RNAV. Uma linha por
// ticker, upsert.
export type ReitManualIndicators = {
  id: number;
  ticker: string;
  ffo_per_share: number | null;
  affo_per_share: number | null;
  occupancy_pct: number | null;
  updated_at: string;
};

export type TransactionType =
  | "compra"
  | "venda"
  | "aporte"
  | "retirada"
  | "provento"
  | "transferencia"
  | "desdobramento"
  | "grupamento"
  | "bonificacao";

export const TRANSACTION_TYPES: TransactionType[] = [
  "compra",
  "venda",
  "aporte",
  "retirada",
  "provento",
  "transferencia",
  "desdobramento",
  "grupamento",
  "bonificacao",
];

export const TRANSACTION_TYPE_LABELS: Record<TransactionType, string> = {
  compra: "Buy",
  venda: "Sell",
  aporte: "Contribution",
  retirada: "Withdrawal",
  provento: "Dividend",
  transferencia: "Transfer between custodies",
  desdobramento: "Stock split",
  grupamento: "Reverse split",
  bonificacao: "Bonus shares",
};

// Fase 13.4 — Dividendo vs. JSCP (juros sobre capital próprio, retido 15% de
// IR na fonte, ver `domain::proventos::net_total` no Rust). Yahoo não
// distingue os dois, então sugestões automáticas sempre nascem "dividendo" —
// editável nos diálogos de Confirmar/Registrar provento futuro
// (DividendSuggestionsSection.tsx) e no lançamento manual de "provento"
// (TransactionSection.tsx).
export type PaymentType = "dividendo" | "jscp";
export const PAYMENT_TYPES: PaymentType[] = ["dividendo", "jscp"];
export const PAYMENT_TYPE_LABELS: Record<PaymentType, string> = {
  dividendo: "Dividend",
  jscp: "JSCP",
};

export const FI_INDEXADORES = ["CDI", "IPCA", "SELIC", "PREFIXADO", "OUTRO"] as const;
export const FI_LIQUIDEZ_OPTIONS = ["diaria", "no_vencimento", "outro"] as const;

export type TransactionView = {
  id: number;
  asset_id: number | null;
  ticker: string | null;
  asset_class: string | null;
  custodia_id: number | null;
  custodia_label: string | null;
  transfer_to_custodia_id: number | null;
  transfer_to_custodia_label: string | null;
  // Fase 19 — moeda do `total_value`, o valor na moeda base pela taxa da data
  // do lançamento (`null` sem cotação do par) e a moeda base.
  currency: string;
  total_value_in_base: number | null;
  base_currency: string;
  transaction_type: string;
  quantity: number | null;
  unit_price: number | null;
  total_value: number;
  fee: number | null;
  transaction_date: string;
  notes: string | null;
  fi_emissor: string | null;
  fi_indexador: string | null;
  fi_taxa_percentual: number | null;
  fi_data_vencimento: string | null;
  fi_liquidez: string | null;
  created_at: string;
  // Fase 13.2 — quantidade líquida do ativo logo após este lançamento
  // (`domain::transaction_ledger::running_quantities`, Rust). `null` pra
  // lançamentos sem asset_id (aporte/retirada).
  running_quantity: number | null;
};

export type CustodiaBreakdown = {
  custodia_id: number | null;
  custodia_label: string | null;
  quantity: number;
};

// Fase 10.3, fatia das classes com preço histórico automatizado (Ação
// BR/FII) — aporte/retirada e as demais classes ainda não entram nesse
// cálculo (ver ProfitabilitySection.tsx).
export type MonthlyReturn = {
  year_month: string;
  bmv: number;
  emv: number;
  cf_total: number;
  r_month_pct: number;
  r_cumulative_pct: number;
};

// Fase 13.5 — comparação com benchmarks de fonte gratuita
// (`commands/profitability.rs::get_profitability_comparison`). `code` é
// "cdi"/"ipca"/"^BVSP"/"IVVB11"; `benchmarks` só traz os que têm dado na
// janela de meses da carteira (IFIX/SMLL/IDIV ficam de fora desta fase,
// sem fonte histórica gratuita — ver PHASE.md item 13.5).
export type BenchmarkPoint = { year_month: string; r_month_pct: number; r_cumulative_pct: number };
export type BenchmarkSeries = { code: string; label: string; monthly: BenchmarkPoint[] };
export type ProfitabilityComparison = {
  portfolio: MonthlyReturn[];
  benchmarks: BenchmarkSeries[];
  // Fase 19 — moeda do retorno; fora do BRL os benchmarks vêm vazios.
  base_currency: string;
};

// Fase 13.1 — `current_price`/`market_value`/`unrealized_pl*` vêm de
// `domain::position_pricing::price_position` (Rust). `price_source`:
// "market" (cotação automática), "manual_valuation" (imóvel/empresa não
// listada, `asset_valuations`), "avg_buy_price" (sem cotação ao vivo,
// fallback pro preço médio de compra) ou "none" (nenhuma fonte disponível).
export type PositionView = {
  asset_id: number;
  ticker: string;
  name: string;
  asset_class: string;
  // Moeda dos valores abaixo (a base do Workspace) e a do ativo; `fx_missing`
  // = sem cotação do par, valores vêm null e ficam fora dos totais.
  currency: string;
  native_currency: string;
  fx_missing: boolean;
  quantity: number;
  average_buy_price: number | null;
  // Fase 34 — ganho/perda já realizado nas vendas (posição encerrada inclusive).
  realized_pl: number;
  by_custodia: CustodiaBreakdown[];
  current_price: number | null;
  market_value: number | null;
  unrealized_pl: number | null;
  unrealized_pl_pct: number | null;
  // Fase 19 — só quando a moeda do ativo difere da base: quanto do P/L veio do
  // preço e quanto do câmbio desde a compra (somam `unrealized_pl`).
  unrealized_pl_price: number | null;
  unrealized_pl_fx: number | null;
  price_source: "market" | "manual_valuation" | "avg_buy_price" | "none";
};

// Fase 13.1 — agregados pra tela Resumo (`commands/portfolio_summary.rs`).
export type ClassAllocation = {
  asset_class: string;
  market_value: number;
  pct: number;
};

export type PortfolioSummary = {
  total_market_value: number;
  total_cost_basis: number;
  unrealized_pl: number;
  unrealized_pl_pct: number | null;
  // Fase 34 — ganho/perda já realizado nas vendas.
  realized_pl: number;
  dividends_received_12m: number;
  allocation_by_class: ClassAllocation[];
  positions_missing_price: number;
  // Fase 19 — moeda dos totais e posições sem cotação do par pra ela.
  base_currency: string;
  positions_missing_fx: number;
};

// Fase 13.6 — sugestão de lançamento de provento gerada a partir de fonte
// externa (primeira versão: `stock_dividend_payments`, histórico do Yahoo).
// `status`: `pending` | `confirmed` | `discarded`. `com_date` (Data Com) é
// campo manual preenchido na confirmação (nenhuma fonte gratuita validada
// devolve isso com confiança, decisão da Sessão 76).
export type DividendSuggestionView = {
  id: number;
  asset_id: number;
  ticker: string;
  name: string;
  asset_class: string;
  currency: string;
  payment_date: string;
  amount: number;
  quantity: number;
  total: number;
  status: string;
  com_date: string | null;
  source: string;
  payment_type: string;
  created_at: string;
};

// Fase 13.4 — agregados pra tela dedicada "Dividends" (`commands/proventos.rs`).
// "paid" vem de `transactions` (`transaction_type = "provento"`); "receivable"
// vem de `suggested_dividends` ainda não confirmado/descartado
// (`pending`/`matched`/`divergent`). `net_total` já desconta 15% de IR
// quando `payment_type === "jscp"`.
export type ProventoRow = {
  asset_id: number;
  ticker: string;
  name: string;
  asset_class: string;
  status: "paid" | "receivable";
  payment_type: string;
  com_date: string | null;
  payment_date: string;
  quantity: number;
  amount_per_share: number;
  // Fase 19 — `total_value`/`net_total` na moeda base (taxa da data do
  // pagamento); `amount_per_share` e `native_total_value` na moeda do ativo.
  total_value: number;
  net_total: number;
  native_currency: string;
  native_total_value: number;
  fx_missing: boolean;
};

export type ProventosMonthlyBucket = {
  year_month: string;
  received: number;
  expected: number;
};

export type ProventosAssetTotal = {
  asset_id: number;
  ticker: string;
  total: number;
};

export type ProventosSummary = {
  avg_monthly_12m: number;
  total_12m: number;
  total_all_time: number;
  by_asset_12m: ProventosAssetTotal[];
  monthly: ProventosMonthlyBucket[];
  rows: ProventoRow[];
  base_currency: string;
};

// Fase 10.4 — listas nomeadas de ativos (preço-alvo/notas) e favoritos
// rápidos (estrela), dois mecanismos separados por pedido explícito do dono
// do projeto.
export type Watchlist = {
  id: number;
  workspace_id: number;
  name: string;
  created_at: string;
};

export type WatchlistItem = {
  id: number;
  watchlist_id: number;
  asset_id: number;
  target_price: number | null;
  notes: string | null;
  created_at: string;
};

export type AssetFavorite = {
  id: number;
  workspace_id: number;
  asset_id: number;
  created_at: string;
};

// Fase 10.5 — tese de investimento, vinculável a um ativo específico
// (asset_id preenchido) ou global/macro (asset_id null). Anexos vivem em
// disco (app_data_dir), só metadados ficam no banco.
export type Thesis = {
  id: number;
  workspace_id: number;
  asset_id: number | null;
  title: string;
  content_markdown: string;
  created_at: string;
  updated_at: string;
};

export type ThesisAttachment = {
  id: number;
  thesis_id: number;
  original_file_name: string;
  stored_relative_path: string;
  file_size_bytes: number;
  content_type: string | null;
  created_at: string;
};

// Fase 10, item 8 — classe `imovel` (cadastro manual). Histórico de
// avaliações do imóvel; `origin` só grava "manual" por ora (reajuste
// automático por % ainda não decidido, ver PHASE.md item 8). Ver
// `commands/property.rs`.
export type AssetValuation = {
  id: number;
  asset_id: number;
  valuation_date: string;
  value: number;
  origin: string;
  notes: string | null;
  created_at: string;
};

// Anexo de um ativo de cadastro manual (escritura, ITBI, IPTU pago, foto),
// mesmo molde de `ThesisAttachment` — arquivo em disco (app_data_dir),
// `document_type` é texto livre pra rotular sem travar numa lista fixa.
export type AssetAttachment = {
  id: number;
  asset_id: number;
  original_file_name: string;
  stored_relative_path: string;
  file_size_bytes: number;
  content_type: string | null;
  document_type: string | null;
  created_at: string;
};

// Sessão 110 — aviso de provento da CVM (`list_cvm_dividend_notices`). `error`
// é a falha só daquele ativo (ex.: sem chave do bolsai).
export type CvmNotice = {
  delivered_on: string;
  pdf_url: string;
};

export type CvmAssetNotices = {
  asset_id: number;
  ticker: string;
  notices: CvmNotice[];
  error: string | null;
};

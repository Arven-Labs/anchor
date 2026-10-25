export type BankAccountView = {
  id: number;
  workspace_id: number;
  nome: string;
  titular: string;
  created_at: string;
  // Fase 19 — moeda da conta; `balance` está nela e `balance_in_base` na moeda
  // base do Workspace (`null` sem cotação do par).
  currency: string;
  balance: number;
  balance_in_base: number | null;
  // Fase 19 — com lançamento, dívida, empréstimo ou baixa a moeda não muda mais.
  in_use: boolean;
  // Fase 35, entrega 8 — o saldo já está na carteira (renda fixa importada): o patrimônio não o conta de novo no caixa.
  mirrored_in_portfolio: boolean;
  // Fase 35, entrega 11 — fração do CDI da estimativa automática do rendimento (1 = 100%): `null` = ainda não decidido
  // (liga sozinho na primeira transferência de porquinho aprovada do extrato), `0` = desligada de propósito.
  auto_yield_pct_of_cdi: number | null;
};

export type GeneralTransactionCategory = {
  id: number;
  workspace_id: number;
  nome: string;
  limite_mensal: number | null;
  created_at: string;
  // Fase 19 — gasto do mês e limite estão na moeda base do Workspace; despesas
  // sem cotação do par ficam fora do gasto e são contadas em `spent_missing_fx`.
  spent_this_month: number;
  base_currency: string;
  spent_missing_fx: number;
};

export type GeneralTransaction = {
  id: number;
  workspace_id: number;
  bank_account_id: number;
  transaction_type: string;
  category_id: number | null;
  valor: number;
  transaction_date: string;
  notes: string | null;
  created_at: string;
  // Fase 19 — as duas pernas de uma transferência compartilham este id.
  transfer_id: number | null;
  // Moeda da conta e o valor na moeda base pela taxa do dia (`null` sem câmbio).
  currency: string;
  valor_in_base: number | null;
};

// Fase 12, núcleo de Finanças Gerais — só `receita`/`despesa` são
// selecionáveis no formulário manual. `parcela_divida` existe como tipo de
// transação (gerado só por `create_liability`, nunca digitado à mão) —
// `compra_ativo`/`venda_ativo` do rascunho original seguem fora de escopo
// (ver PHASE.md, Fase 12).
export const GENERAL_TRANSACTION_TYPES = ["receita", "despesa"] as const;
export type GeneralTransactionType = (typeof GENERAL_TRANSACTION_TYPES)[number];

// Registro solto (não `Record<GeneralTransactionType, string>`) porque
// precisa rotular também `parcela_divida`, que não é selecionável no form.
export const GENERAL_TRANSACTION_TYPE_LABELS: Record<string, string> = {
  receita: "Income",
  despesa: "Expense",
  parcela_divida: "Loan installment",
  // Fase 29 — gerados só por `create_loan`/`register_loan_repayment`.
  emprestimo_concedido: "Loan granted",
  recebimento_emprestimo: "Loan repayment received",
  // Fase 19 — as duas pernas aparecem como uma linha "Transfer" na lista.
  transferencia_saida: "Transfer out",
  transferencia_entrada: "Transfer in",
};

// Fase 29 — empréstimos concedidos a terceiros.
export type Borrower = {
  id: number;
  workspace_id: number;
  nome: string;
  notes: string | null;
  created_at: string;
};

export type LoanRepayment = {
  id: number;
  loan_id: number;
  bank_account_id: number | null;
  general_transaction_id: number | null;
  data: string;
  valor_recebido: number;
  valor_baixado: number;
  notes: string | null;
  created_at: string;
};

export type LoanView = {
  id: number;
  workspace_id: number;
  borrower_id: number;
  borrower_nome: string;
  bank_account_id: number | null;
  principal: number;
  taxa_mensal_percentual: number | null;
  data_inicio: string;
  include_in_net_worth: boolean;
  notes: string | null;
  created_at: string;
  currency: string;
  accrued_interest: number;
  total_written_off: number;
  extra_received: number;
  saldo_a_receber: number;
  saldo_a_receber_in_base: number | null;
  repayments: LoanRepayment[];
};

export type Liability = {
  id: number;
  workspace_id: number;
  bank_account_id: number;
  linked_asset_id: number | null;
  nome: string;
  titular: string;
  principal: number;
  taxa_percentual: number;
  indexador: string;
  prazo_meses: number;
  sistema_amortizacao: string;
  data_inicio: string;
  created_at: string;
};

export type LiabilityView = Liability & {
  // Fase 19 — moeda da conta da dívida e o saldo na moeda base (`null` sem cotação).
  currency: string;
  saldo_devedor_atual: number;
  saldo_devedor_in_base: number | null;
  // Fase 12 — retorno alavancado. Os 3 campos abaixo aparecem juntos (todos
  // presentes ou todos `null`): sem `linked_asset_id`, "custo da dívida"
  // sozinho não tem com o que ser comparado (ver `commands/liability.rs`).
  annualized_debt_cost_pct: number | null;
  linked_asset_return_12m_pct: number | null;
  spread_12m_pct: number | null;
};

// Fase 35, entrega 9 — conciliação do saldo pelo saldo real do banco
// (`commands::balance_reconciliation::preview_balance_reconciliation`).
export type ReconcilePreview = {
  date: string;
  bank_balance: number;
  /** Saldo do ledger no fim do dia, sem o rendimento que falta. */
  ledger_balance: number;
  /** `bank_balance − ledger_balance`; só vira receita se for positiva. */
  difference: number;
};

// Fase 35, entrega 10 — estimativa do rendimento do porquinho por resgate
// (`commands::porquinho_estimate::stage_porquinho_yield_estimate`).
export type EstimateResult = {
  redemptions: number;
  staged: number;
  skipped_covered_day: number;
  skipped_no_yield: number;
  duplicates: number;
};

// Fase 12 — Patrimônio Líquido/Alavancagem cruzando Portfolio + Finanças
// Gerais (`commands::net_worth::get_net_worth_summary`).
export type NetWorthSummary = {
  /** Fase 19 — moeda de todos os totais abaixo. */
  base_currency: string;
  /** Itens deixados de fora por falta de cotação do par pra moeda base. */
  items_missing_fx: number;
  total_market_value: number;
  total_cash: number;
  /** Parte do saldo das contas marcadas "já na carteira" que saiu do caixa por já estar nas posições de renda fixa. */
  cash_in_portfolio: number;
  total_debt: number;
  total_loans_receivable: number;
  /** Posições em opções abertas com a flag ligada (Fase 18): comprada soma, lançada subtrai. */
  total_options_value: number;
  /** Quantas dessas valeram o prêmio, por falta de preço atualizado. */
  options_at_cost_count: number;
  net_worth: number;
  leverage_ratio: number | null;
};

// Fase 12 — dívida/passivo. Vocabulário duplicado de
// `portfolio/types.ts::FI_INDEXADORES` de propósito: Finanças Gerais é
// módulo irmão de Portfolio, não depende dele (ver PHASE.md, Fase 12).
export const LIABILITY_INDEXADORES = ["CDI", "IPCA", "SELIC", "PREFIXADO", "OUTRO"] as const;

export const LIABILITY_AMORTIZATION_SYSTEMS = ["price", "sac"] as const;
export type LiabilityAmortizationSystem = (typeof LIABILITY_AMORTIZATION_SYSTEMS)[number];

export const LIABILITY_AMORTIZATION_SYSTEM_LABELS: Record<LiabilityAmortizationSystem, string> = {
  price: "Price (fixed installment)",
  sac: "SAC (fixed amortization)",
};

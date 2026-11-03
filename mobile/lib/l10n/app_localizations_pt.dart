// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Portuguese (`pt`).
class AppLocalizationsPt extends AppLocalizations {
  AppLocalizationsPt([String locale = 'pt']) : super(locale);

  @override
  String get cryptoScoreTitle => 'Score cripto';

  @override
  String get cryptoThresholdsTooltip => 'Limites';

  @override
  String get cryptoRefresh => 'Atualizar';

  @override
  String get cryptoRefreshing => 'Atualizando…';

  @override
  String get cryptoNoReadings =>
      'Nenhuma leitura ainda. Toque em Atualizar para coletar.';

  @override
  String cryptoUnexpectedError(String detail) {
    return 'Erro inesperado: $detail';
  }

  @override
  String get cryptoFailuresTitle => 'Falhas da última coleta';

  @override
  String cryptoValidReadings(int count) {
    return '$count com leitura válida';
  }

  @override
  String get cryptoStatusNoReading => 'Sem leitura';

  @override
  String get cryptoStatusExpired => 'Vencida';

  @override
  String greenCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count verdes',
      one: '1 verde',
      zero: '0 verdes',
    );
    return '$_temp0';
  }

  @override
  String loggedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count registrados',
      one: '1 registrado',
      zero: '0 registrados',
    );
    return '$_temp0';
  }

  @override
  String greensOfTotal(String greens, int total) {
    return '$greens de $total';
  }

  @override
  String get historyTitle => 'Histórico';

  @override
  String get historyEmpty => 'Sem histórico ainda.';

  @override
  String get historyExplainer =>
      'Parte dos indicadores verdes em cada data. Uma leitura deixa de contar quando passa do prazo.';

  @override
  String historyDateCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count datas',
      one: '1 data',
      zero: '0 datas',
    );
    return '$_temp0';
  }

  @override
  String historySummary(String dates, String greensOfTotal, String date) {
    return '$dates · último: $greensOfTotal em $date';
  }

  @override
  String historyDetail(String date, String text) {
    return '$date — $text';
  }

  @override
  String historyPoint(String greensOfTotal, String logged, String verdict) {
    return '$greensOfTotal · $logged · $verdict';
  }

  @override
  String get periodAll => 'Tudo';

  @override
  String get verdictIntact => 'Tese intacta — manter / seguir aportando';

  @override
  String get verdictNeutral => 'Neutro — acompanhar de perto';

  @override
  String get verdictReduceRisk => 'Reduzir risco';

  @override
  String get verdictShortIntact => 'Tese intacta';

  @override
  String get verdictShortNeutral => 'Neutro';

  @override
  String get verdictShortReduceRisk => 'Reduzir risco';

  @override
  String get signalGreen => 'Verde';

  @override
  String get signalNeutral => 'Neutro';

  @override
  String get signalRed => 'Vermelho';

  @override
  String get groupOnChain => 'On-chain';

  @override
  String get groupFundamentals => 'Fundamentos da rede';

  @override
  String get groupDerivatives => 'Derivativos e opções';

  @override
  String get groupLiquidity => 'Liquidez de mercado';

  @override
  String get groupSentiment => 'Sentimento de mercado';

  @override
  String get indicatorMvrvZScore => 'MVRV Z-Score';

  @override
  String get indicatorNvtRatio => 'NVT (vs. média de 90 dias)';

  @override
  String get indicatorPuellMultiple => 'Múltiplo de Puell';

  @override
  String get indicatorNetIssuance => 'Emissão líquida (emissão − queima)';

  @override
  String get indicatorStakingYield => 'Staking yield real';

  @override
  String get indicatorTvlTrend => 'Tendência do TVL (30d)';

  @override
  String get indicatorActiveAddressesTrend => 'Endereços ativos (30d)';

  @override
  String get indicatorExchangeNetflow => 'Netflow das exchanges';

  @override
  String get indicatorFeesVsEmission => 'Taxas vs. emissão';

  @override
  String get indicatorFundingRate7d => 'Funding dos perpétuos (7d, anualizado)';

  @override
  String get indicatorOpenInterestChange30d => 'Open interest (variação 30d)';

  @override
  String get indicatorLongShortRatio => 'Razão long/short (7d)';

  @override
  String get indicatorDvol => 'DVOL (volatilidade implícita)';

  @override
  String get indicatorPutCallRatio => 'Put/call de opções';

  @override
  String get indicatorStablecoinSupplyChange30d =>
      'Oferta de stablecoins (30d)';

  @override
  String get indicatorExchangeSupplyChange30d => 'Oferta nas exchanges (30d)';

  @override
  String get indicatorEthBtcTrend30d => 'ETH/BTC (30d)';

  @override
  String get indicatorFearGreedIndex => 'Fear & Greed (0–100)';

  @override
  String get indicatorMayerMultiple => 'Múltiplo de Mayer';

  @override
  String get indicatorBtcDominance => 'Dominância do BTC';

  @override
  String get indicatorSopr => 'SOPR';

  @override
  String get indicatorNupl => 'NUPL';

  @override
  String thresholdsTitle(String coin) {
    return 'Limites · $coin';
  }

  @override
  String get thresholdDefault => 'Padrão';

  @override
  String get thresholdEdited => 'Editado';

  @override
  String get thresholdGreen => 'Verde';

  @override
  String get thresholdRed => 'Vermelho';

  @override
  String bandsHigherIsBetter(String green, String red) {
    return 'Verde ≥ $green · Vermelho ≤ $red';
  }

  @override
  String bandsLowerIsBetter(String green, String red) {
    return 'Verde ≤ $green · Vermelho ≥ $red';
  }

  @override
  String get thresholdHigherIsBetter => 'Maior é melhor';

  @override
  String get thresholdLowerIsBetter => 'Menor é melhor';

  @override
  String thresholdDefaultValues(String green, String red) {
    return 'Padrão: verde $green, vermelho $red';
  }

  @override
  String get thresholdSaveWarning =>
      'Salvar reclassifica as leituras já gravadas e muda o histórico: o passado passa a ser visto com estes limites.';

  @override
  String get thresholdNeedTwoNumbers => 'Digite dois números.';

  @override
  String get thresholdNeedDifferentNumbers => 'Use dois números diferentes.';

  @override
  String get actionReset => 'Redefinir';

  @override
  String get actionCancel => 'Cancelar';

  @override
  String get actionSave => 'Salvar';

  @override
  String get actionSaving => 'Salvando…';

  @override
  String get navHome => 'Início';

  @override
  String get navQuote => 'Cotação';

  @override
  String get navPortfolio => 'Portfolio';

  @override
  String get navFinance => 'Finanças';

  @override
  String get navCrypto => 'Cripto';

  @override
  String get homeTitle => 'Início';

  @override
  String get homeCustomizeTitle => 'Personalizar Início';

  @override
  String get homeCustomizeTooltip => 'Personalizar';

  @override
  String get homeRestoreDefault => 'Restaurar padrão';

  @override
  String get homeWidthFull => 'Cheia';

  @override
  String get homeWidthHalf => 'Meia';

  @override
  String homeSaveFailed(String error) {
    return 'Não consegui salvar o layout: $error';
  }

  @override
  String homeLoadFailed(String error) {
    return 'Não consegui carregar as cotações agora. Puxe para atualizar. ($error)';
  }

  @override
  String get homeAllHidden =>
      'Todos os cards estão escondidos. Toque em Personalizar para mostrá-los.';

  @override
  String homeTotalMissing(int count) {
    return '$count fora da soma (sem cotação ou câmbio)';
  }

  @override
  String homeProfitExcluded(int count) {
    return '$count fora do cálculo';
  }

  @override
  String get homeNoPositions => 'Sem posições abertas.';

  @override
  String get homeCash => 'Caixa';

  @override
  String get homePortfolio => 'Carteira';

  @override
  String get homeLoansReceivable => 'Empréstimos';

  @override
  String get homeDebts => 'Dívidas';

  @override
  String homeNetWorthMissing(int count) {
    return '$count item(ns) fora da soma por falta de câmbio ou cotação.';
  }

  @override
  String homeFxUnavailable(String currencies) {
    return 'Câmbio indisponível agora para $currencies.';
  }

  @override
  String get homeWidgetTotalValue => 'Valor total';

  @override
  String get homeWidgetProfit => 'Lucro não realizado';

  @override
  String get homeWidgetAllocation => 'Alocação por classe';

  @override
  String get homeWidgetNetWorth => 'Patrimônio líquido';

  @override
  String get assetClassAcaoBr => 'Ação BR';

  @override
  String get assetClassFii => 'FII';

  @override
  String get assetClassEtfBr => 'ETF BR';

  @override
  String get assetClassEtfRendaFixa => 'ETF Renda Fixa BR';

  @override
  String get assetClassBdr => 'BDR';

  @override
  String get assetClassAcaoInternacional => 'Ação internacional';

  @override
  String get assetClassReit => 'REIT';

  @override
  String get assetClassEtfUs => 'ETF US';

  @override
  String get assetClassCripto => 'Cripto';

  @override
  String get assetClassMetal => 'Metal';

  @override
  String get assetClassTesouroDireto => 'Tesouro Direto';

  @override
  String get assetClassRendaFixa => 'Renda Fixa';

  @override
  String get assetClassImovel => 'Imóvel';

  @override
  String get assetClassEmpresaNaoListada => 'Empresa não listada';

  @override
  String get assetClassFundo => 'Fundo';

  @override
  String get portfolioTitle => 'Portfolio';

  @override
  String get portfolioCryptoTax => 'IR de cripto';

  @override
  String get portfolioForeignTax => 'IR de exterior';

  @override
  String get portfolioFundTax => 'IR de fundos';

  @override
  String get portfolioSwapCrypto => 'Trocar cripto';

  @override
  String get portfolioCustodies => 'Custódias';

  @override
  String get portfolioHistory => 'Histórico';

  @override
  String get portfolioAddAsset => 'Adicionar ativo';

  @override
  String get portfolioEmpty => 'Nenhum ativo cadastrado ainda.';

  @override
  String portfolioUnits(String quantity) {
    return '$quantity un.';
  }

  @override
  String portfolioAveragePrice(String value) {
    return 'Preço médio: $value';
  }

  @override
  String portfolioCurrentPrice(String value) {
    return 'Preço atual: $value';
  }

  @override
  String portfolioValue(String value) {
    return 'Valor: $value';
  }

  @override
  String get portfolioPriceUnavailable => 'indisponível agora';

  @override
  String portfolioShare(String percent, String value) {
    return 'Participação: $percent% — $value';
  }

  @override
  String get actionEdit => 'Editar';

  @override
  String get actionDelete => 'Excluir';

  @override
  String get finTitle => 'Finanças';

  @override
  String get finTransfer => 'Transferir';

  @override
  String get finLoans => 'Empréstimos';

  @override
  String get finDebts => 'Dívidas';

  @override
  String get finCategories => 'Categorias';

  @override
  String get finRefresh => 'Atualizar';

  @override
  String get finFabAccount => 'Conta';

  @override
  String get finEmptyAccounts => 'Nenhuma conta cadastrada ainda.';

  @override
  String get finNetWorth => 'Patrimônio';

  @override
  String get finCalculating => 'Calculando…';

  @override
  String finCash(String value) {
    return 'Caixa $value';
  }

  @override
  String finPortfolio(String value) {
    return 'Carteira $value';
  }

  @override
  String finLoansReceivable(String value) {
    return 'Empréstimos a receber $value';
  }

  @override
  String finDebtsLine(String value) {
    return 'Dívidas −$value';
  }

  @override
  String finMissingItems(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count itens fora da soma por falta de câmbio ou cotação.',
      one: '1 item fora da soma por falta de câmbio ou cotação.',
    );
    return '$_temp0';
  }

  @override
  String finFxBanner(String currencies) {
    return 'Câmbio indisponível agora para $currencies. Os itens dessas moedas ficam fora do patrimônio.';
  }

  @override
  String finNoRate(String pair) {
    return 'sem câmbio $pair';
  }

  @override
  String get finNewAccount => 'Nova conta';

  @override
  String get finEditAccount => 'Editar conta';

  @override
  String get finFieldName => 'Nome';

  @override
  String get finFieldHolder => 'Titular';

  @override
  String get finFieldCurrency => 'Moeda';

  @override
  String get finAccountNameHint => 'Nubank';

  @override
  String get finCurrencyLocked => 'Não muda numa conta que já tem lançamentos.';

  @override
  String finDeleteTitle(String name) {
    return 'Excluir $name?';
  }

  @override
  String get finDeleteWithTx =>
      'Os lançamentos desta conta também serão apagados.';

  @override
  String get finDeleteNoTx => 'A conta não tem lançamentos.';

  @override
  String get finTransferNeedTwo => 'Cadastre duas contas para transferir.';

  @override
  String get finFrom => 'De';

  @override
  String get finTo => 'Para';

  @override
  String finAmountSent(String currency) {
    return 'Saiu ($currency)';
  }

  @override
  String finAmount(String currency) {
    return 'Valor ($currency)';
  }

  @override
  String finAmountReceived(String currency) {
    return 'Chegou ($currency)';
  }

  @override
  String finEffectiveRate(String from, String rate, String to) {
    return 'Taxa efetiva: 1 $from = $rate $to';
  }

  @override
  String get finSpreadIncluded => 'Spread e tarifa já embutidos nos valores.';

  @override
  String get finNotes => 'Observação';

  @override
  String get finSameAccount => 'Escolha duas contas diferentes.';

  @override
  String get finAmountsRequired => 'Informe os valores, maiores que zero.';

  @override
  String get txNewTitle => 'Novo lançamento';

  @override
  String get txCategory => 'Categoria';

  @override
  String get txNoCategory => 'Sem categoria';

  @override
  String get txAmountRequired => 'Informe um valor maior que zero.';

  @override
  String get txDeleteTitle => 'Excluir lançamento?';

  @override
  String txDeleteBody(String type, String value, String date) {
    return '$type de $value em $date.';
  }

  @override
  String get txDeleteTransferLeg =>
      'A outra perna da transferência também será apagada.';

  @override
  String get txFab => 'Lançamento';

  @override
  String get txBalance => 'Saldo';

  @override
  String get txEmpty => 'Nenhum lançamento nesta conta ainda.';

  @override
  String get txFuture => 'ainda não entra no saldo';

  @override
  String get txTypeReceita => 'Receita';

  @override
  String get txTypeDespesa => 'Despesa';

  @override
  String get txTypeTransferenciaSaida => 'Transferência (saída)';

  @override
  String get txTypeTransferenciaEntrada => 'Transferência (entrada)';

  @override
  String get txTypeParcelaDivida => 'Parcela de dívida';

  @override
  String get txTypeEmprestimoConcedido => 'Empréstimo concedido';

  @override
  String get txTypeRecebimentoEmprestimo => 'Recebimento de empréstimo';

  @override
  String genTxInstallment(String name, int number, int total) {
    return '$name — parcela $number/$total';
  }

  @override
  String genTxLoanGranted(String name) {
    return 'Empréstimo para $name';
  }

  @override
  String genTxLoanRepayment(String name) {
    return 'Recebimento de $name';
  }

  @override
  String get amortizationPrice => 'Price (parcela fixa)';

  @override
  String get amortizationSac => 'SAC (amortização fixa)';

  @override
  String get actionRegister => 'Cadastrar';

  @override
  String get liabTitle => 'Dívidas';

  @override
  String get liabFab => 'Dívida';

  @override
  String get liabEmpty => 'Nenhuma dívida cadastrada ainda.';

  @override
  String get liabNeedAccount => 'Cadastre uma conta antes da dívida.';

  @override
  String get liabNew => 'Nova dívida';

  @override
  String get liabNameHint => 'Financiamento do carro';

  @override
  String get liabFieldAccount => 'Conta';

  @override
  String liabFieldPrincipal(String currency) {
    return 'Principal ($currency)';
  }

  @override
  String get liabFieldRate => 'Taxa (% a.m.)';

  @override
  String get liabRateHelp => '0 para dívida sem juros.';

  @override
  String get liabFieldTerm => 'Prazo (meses)';

  @override
  String get liabFieldSystem => 'Sistema';

  @override
  String liabStart(String date) {
    return 'Início $date';
  }

  @override
  String get liabFirstDue => 'A parcela 1 vence um mês depois do início.';

  @override
  String get liabInvalid =>
      'Informe nome, principal e prazo maiores que zero e a taxa.';

  @override
  String liabDeleteTitle(String name) {
    return 'Excluir $name?';
  }

  @override
  String get liabDeleteBody =>
      'As parcelas desta dívida também saem dos lançamentos da conta.';

  @override
  String get liabDeleteAction => 'Excluir dívida';

  @override
  String liabBalance(String value) {
    return 'Saldo devedor $value';
  }

  @override
  String liabPaid(int paid, int total) {
    return '$paid de $total parcelas pagas';
  }

  @override
  String liabNext(String value, String date) {
    return 'Próxima: $value em $date';
  }

  @override
  String get liabSettled => 'Quitada';

  @override
  String liabSummary(String system, String rate, String value) {
    return '$system · $rate% a.m. · principal $value';
  }

  @override
  String liabInstallmentLine(
    String number,
    String date,
    String value,
    String interest,
  ) {
    return '$number · $date · $value (juros $interest)';
  }

  @override
  String get loanTitle => 'Empréstimos';

  @override
  String get loanFab => 'Empréstimo';

  @override
  String get loanEmpty => 'Nenhum empréstimo cadastrado ainda.';

  @override
  String get loanNew => 'Novo empréstimo';

  @override
  String get loanEdit => 'Editar empréstimo';

  @override
  String get loanBorrower => 'Devedor';

  @override
  String get loanNewBorrowerOption => 'Novo devedor…';

  @override
  String get loanNewBorrowerName => 'Nome do devedor';

  @override
  String get loanFromAccount => 'Conta de onde saiu';

  @override
  String get loanNoAccountHelp =>
      'Sem conta é só registro: nada muda em saldo.';

  @override
  String get loanNoAccount => 'Sem conta';

  @override
  String get loanCurrency => 'Moeda';

  @override
  String loanPrincipal(String currency) {
    return 'Principal ($currency)';
  }

  @override
  String get loanRateOptional => 'Taxa (% a.m., opcional)';

  @override
  String get loanRateHelp =>
      'Juros simples, só informativos. Vazio = sem juros.';

  @override
  String loanDateOn(String date) {
    return 'Em $date';
  }

  @override
  String get loanInclude => 'Somar ao patrimônio';

  @override
  String get loanNotes => 'Observação';

  @override
  String get loanInvalid =>
      'Informe o devedor e um principal maior que zero; a taxa, se houver, não pode ser negativa.';

  @override
  String get loanRateNegative => 'A taxa não pode ser negativa.';

  @override
  String get loanRepayTitle => 'Registrar baixa';

  @override
  String get loanRepayConfirm => 'Registrar';

  @override
  String loanReceived(String currency) {
    return 'Recebido ($currency)';
  }

  @override
  String get loanReceivedHelp =>
      '0 ou vazio para perdão, sem dinheiro entrando.';

  @override
  String get loanWrittenOff => 'Baixado do saldo';

  @override
  String get loanWrittenOffHelp => 'Vazio = o mesmo valor recebido.';

  @override
  String get loanReceivingAccount => 'Conta que recebeu';

  @override
  String get loanRepayInvalid =>
      'Informe o valor baixado (maior que zero); o recebido não pode ser negativo.';

  @override
  String loanDeleteTitle(String name) {
    return 'Excluir o empréstimo de $name?';
  }

  @override
  String get loanDeleteBody =>
      'As baixas e os lançamentos que ele gerou na conta também saem.';

  @override
  String get loanDeleteRepayTitle => 'Excluir a baixa?';

  @override
  String loanDeleteRepayBody(String value, String date) {
    return 'Baixa de $value em $date. O recebimento lançado na conta também sai.';
  }

  @override
  String loanToReceive(String value) {
    return 'A receber $value';
  }

  @override
  String get loanOutsideNetWorth => ' (fora do patrimônio)';

  @override
  String loanPrincipalOn(String value, String date) {
    return 'Principal $value em $date';
  }

  @override
  String loanInterest(String value) {
    return 'Juros acumulados $value';
  }

  @override
  String loanExtraMore(String value) {
    return 'Recebido a mais: $value';
  }

  @override
  String loanExtraLess(String value) {
    return 'Recebido a menos: $value';
  }

  @override
  String loanRate(String rate) {
    return 'Taxa $rate% a.m. (juros simples)';
  }

  @override
  String get loanNoRepayments => 'Nenhuma baixa ainda.';

  @override
  String loanRepaymentLine(String date, String received, String writtenOff) {
    return '$date · recebido $received · baixou $writtenOff';
  }

  @override
  String get catTitle => 'Categorias';

  @override
  String get catFab => 'Categoria';

  @override
  String get catEmpty => 'Nenhuma categoria cadastrada ainda.';

  @override
  String get catNew => 'Nova categoria';

  @override
  String get catEdit => 'Editar categoria';

  @override
  String get catNameHint => 'Mercado';

  @override
  String catMonthlyLimit(String base) {
    return 'Limite mensal ($base)';
  }

  @override
  String get catNoLimitHelp => 'Vazio = sem limite.';

  @override
  String get catInvalid =>
      'Informe o nome e, se houver, um limite maior que zero.';

  @override
  String get catDeleteBody =>
      'Os lançamentos dela ficam sem categoria; nenhum é apagado.';

  @override
  String catSpentMonth(String value) {
    return 'Gasto no mês: $value';
  }

  @override
  String catSpentOfLimit(String spent, String limit) {
    return '$spent de $limit';
  }

  @override
  String catOver(String value) {
    return 'Estourou em $value';
  }

  @override
  String catMissingFx(int count) {
    return '$count despesa(s) fora da soma por falta de câmbio.';
  }

  @override
  String get currencyUsd => 'Dólar americano';

  @override
  String get currencyEur => 'Euro';

  @override
  String get currencyGbp => 'Libra esterlina';

  @override
  String get currencyJpy => 'Iene';

  @override
  String get currencyChf => 'Franco suíço';

  @override
  String get currencyCad => 'Dólar canadense';

  @override
  String get currencyAud => 'Dólar australiano';

  @override
  String get currencyNzd => 'Dólar neozelandês';

  @override
  String get currencyCny => 'Yuan';

  @override
  String get currencyBrl => 'Real';

  @override
  String get currencyArs => 'Peso argentino';

  @override
  String get currencyUyu => 'Peso uruguaio';

  @override
  String get currencyPyg => 'Guarani';

  @override
  String get currencyClp => 'Peso chileno';

  @override
  String get currencyCop => 'Peso colombiano';

  @override
  String get currencyPen => 'Sol peruano';

  @override
  String get currencyMxn => 'Peso mexicano';

  @override
  String finErrUnknownCurrency(String currency) {
    return 'Moeda fora do catálogo: $currency';
  }

  @override
  String get finErrAmountMustBePositive => 'O valor deve ser maior que zero';

  @override
  String finErrInvalidDate(String date) {
    return 'Data inválida: $date';
  }

  @override
  String get finErrManualTypeOnly =>
      'Só receita e despesa são lançadas à mão; transferência nasce em Transferir, parcela de dívida ao criar a dívida e o empréstimo ao conceder o empréstimo';

  @override
  String get finErrLiabilityNameRequired => 'Informe o nome da dívida';

  @override
  String get finErrBorrowerNameRequired => 'Informe o nome do devedor';

  @override
  String get finErrPrincipalMustBePositive =>
      'O principal deve ser maior que zero';

  @override
  String get finErrTermMustBePositive =>
      'O prazo (em meses) deve ser maior que zero';

  @override
  String get finErrNegativeMonthlyRate => 'A taxa mensal não pode ser negativa';

  @override
  String get finErrRepaidAmountMustBePositive =>
      'O valor baixado deve ser maior que zero';

  @override
  String get finErrReceivedAmountNegative =>
      'O valor recebido não pode ser negativo';

  @override
  String finErrLoanCurrencyMismatch(
    String loanCurrency,
    String accountCurrency,
  ) {
    return 'O empréstimo é em $loanCurrency e a conta em $accountCurrency';
  }

  @override
  String get finErrCategoryNameRequired => 'Informe o nome da categoria';

  @override
  String get finErrCategoryLimitMustBePositive =>
      'O limite mensal deve ser maior que zero';

  @override
  String get finErrTransferAmountsMustBePositive =>
      'Os valores devem ser maiores que zero';

  @override
  String get finErrTransferSameAccount => 'Escolha duas contas diferentes';

  @override
  String get finErrTransferSameCurrencyAmountsDiffer =>
      'Contas na mesma moeda recebem exatamente o que saiu';

  @override
  String get finErrAccountNotFound => 'Conta não encontrada';

  @override
  String get finErrAccountCurrencyLocked =>
      'A moeda não muda numa conta que já tem lançamentos';

  @override
  String get finErrBorrowerHasLoans =>
      'Este devedor ainda tem empréstimos: apague-os antes';

  @override
  String get finErrBorrowerNotFound => 'Devedor não encontrado';

  @override
  String get finErrLoanNotFound => 'Empréstimo não encontrado';

  @override
  String get finErrLinkedTransactionLocked =>
      'Parcela de dívida e lançamento de empréstimo só se apagam junto com a dívida ou o empréstimo';
}

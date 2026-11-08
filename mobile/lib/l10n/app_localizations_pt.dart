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
  String get homeSettingsTooltip => 'Configurações';

  @override
  String get settingsTitle => 'Configurações';

  @override
  String get settingsLanguage => 'Idioma';

  @override
  String get settingsLanguageSystem => 'Seguir o aparelho';

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
  String get portfolioB3Tax => 'IR de B3';

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

  @override
  String get finErrBalanceMustBeNonNegative =>
      'O saldo do banco deve ser zero ou mais';

  @override
  String finErrDateInFuture(String date) {
    return 'A data do saldo não pode ser futura: $date';
  }

  @override
  String get finErrNothingToReconcile =>
      'Os lançamentos já batem com o saldo do banco: nada a conciliar';

  @override
  String get finErrBankBalanceBelowLedger =>
      'O saldo do banco é menor que o dos lançamentos. Pode faltar uma retirada, ou use o ajuste de saldo';

  @override
  String get cryptoTaxTitle => 'IR de cripto';

  @override
  String get cryptoTaxDisclaimer =>
      'Calculado só com as transações lançadas neste app, a custo médio e com a taxa informada em cada operação. As regras vêm de fontes secundárias: confira na Receita (GCAP) antes de pagar.';

  @override
  String get cryptoTaxUsdNote =>
      'Operações em USD convertidas pelo dólar da data (Yahoo, USDBRL=X).';

  @override
  String get cryptoTaxFxUnavailable =>
      'Câmbio indisponível agora: as operações em USD ficaram de fora. Tente de novo em instantes.';

  @override
  String get cryptoTaxEmpty => 'Nenhuma venda de cripto em reais ainda.';

  @override
  String cryptoTaxMonthSales(String value) {
    return 'Vendas do mês: $value';
  }

  @override
  String cryptoTaxMonthGain(String value) {
    return 'Ganho do mês: $value';
  }

  @override
  String cryptoTaxTax(String value, String rate) {
    return 'Imposto: $value (alíquota $rate%)';
  }

  @override
  String cryptoTaxCarriedIn(String value) {
    return 'Acumulado de meses anteriores: $value';
  }

  @override
  String cryptoTaxSalesCount(int count) {
    return 'Vendas ($count)';
  }

  @override
  String cryptoTaxSaleTitle(String ticker, String date) {
    return '$ticker — $date';
  }

  @override
  String cryptoTaxSaleDetail(
    String sale,
    String fee,
    String cost,
    String gain,
  ) {
    return 'Venda $sale$fee · custo $cost · ganho $gain';
  }

  @override
  String cryptoTaxSaleFee(String value) {
    return ' (taxa $value)';
  }

  @override
  String cryptoTaxStatusExempt(String limit) {
    return 'Isento: vendas até $limit no mês';
  }

  @override
  String cryptoTaxStatusDarf(int code, String value, String date) {
    return 'DARF $code de $value até $date';
  }

  @override
  String cryptoTaxStatusBelowMinimum(String value, String minimum) {
    return 'Imposto de $value abaixo do mínimo de $minimum: soma ao mês seguinte';
  }

  @override
  String get cryptoTaxStatusNoTax => 'Sem imposto: o mês fechou sem ganho';

  @override
  String cryptoTaxWarnCostUnknown(String ticker, String date) {
    return 'Venda de $ticker em $date maior que as compras lançadas: o ganho considera só a parte com custo conhecido. Lance as compras que faltam.';
  }

  @override
  String cryptoTaxWarnSalesNonBrl(String period) {
    return 'Vendas de cripto fora de BRL em $period não são apuradas aqui (regra de exterior).';
  }

  @override
  String cryptoTaxWarnFxMissing(String period) {
    return 'Venda de cripto em USD em $period sem câmbio para converter: ficou de fora da apuração.';
  }

  @override
  String get b3TaxTitle => 'IR de B3';

  @override
  String get b3TaxDisclaimer =>
      'Estimativa feita só com as transações lançadas neste app, a custo médio e com a taxa informada em cada operação. Day trade é compra e venda do mesmo ativo, no mesmo dia e na mesma custódia. Não cobre opções, desdobramentos nem bonificações. Alíquotas e limite de isenção vêm de fontes secundárias: confira na Receita e nas notas de corretagem antes de pagar.';

  @override
  String get b3TaxEmpty => 'Nenhuma venda na B3 ainda.';

  @override
  String b3TaxMonthStockSales(String value) {
    return 'Vendas de ações (swing): $value';
  }

  @override
  String b3TaxMonthExemptGain(String value) {
    return 'Ganho isento: $value';
  }

  @override
  String b3TaxBucketLine(
    String name,
    String result,
    String used,
    String tax,
    String rate,
  ) {
    return '$name: resultado $result, prejuízo compensado $used, imposto $tax ($rate%)';
  }

  @override
  String b3TaxIrrf(String value) {
    return 'IRRF retido (estimado): $value';
  }

  @override
  String b3TaxCarriedIn(String value) {
    return 'Saldo de meses anteriores: $value';
  }

  @override
  String get b3TaxBucketCommon => 'Comum';

  @override
  String get b3TaxBucketDayTrade => 'Day trade';

  @override
  String get b3TaxBucketFii => 'FII';

  @override
  String b3TaxSalesCount(int count) {
    return 'Vendas ($count)';
  }

  @override
  String b3TaxSaleTitle(String ticker, String date) {
    return '$ticker — $date';
  }

  @override
  String b3TaxSaleDetail(String bucket, String sale, String cost, String gain) {
    return '$bucket · venda $sale · custo $cost · ganho $gain';
  }

  @override
  String b3TaxStatusExempt(String limit) {
    return 'Isento: vendas de ações até $limit no mês';
  }

  @override
  String b3TaxStatusDarf(int code, String value, String date) {
    return 'DARF $code de $value até $date';
  }

  @override
  String b3TaxStatusBelowMinimum(String value, String minimum) {
    return 'Imposto de $value abaixo do mínimo de $minimum: soma ao mês seguinte';
  }

  @override
  String get b3TaxStatusNoTax => 'Sem imposto: nada a pagar no mês';

  @override
  String b3TaxWarnCostUnknown(String ticker, String date) {
    return 'Venda de $ticker em $date maior que as compras lançadas: o ganho considera só a parte com custo conhecido. Lance as compras que faltam.';
  }

  @override
  String get valuationButton => 'Valuation';

  @override
  String valuationTitle(String ticker) {
    return 'Valuation — $ticker';
  }

  @override
  String valuationCurrentPrice(String value) {
    return 'Cotação atual: $value';
  }

  @override
  String get valuationModelLabel => 'Modelo';

  @override
  String get valuationDisclaimer =>
      'Só a conta das premissas que você digita, não é recomendação. As taxas são digitadas em porcentagem. Valores e ações precisam estar na mesma escala: se os valores estão em R\$ milhões, digite as ações em milhões.';

  @override
  String get valuationCalculate => 'Calcular';

  @override
  String get valuationSave => 'Salvar';

  @override
  String get valuationSaved => 'Cálculo salvo';

  @override
  String valuationFairPrice(String value) {
    return 'Preço justo: $value';
  }

  @override
  String valuationSafetyMargin(String value) {
    return 'Margem de segurança: $value';
  }

  @override
  String get valuationVerdictCheap =>
      'Barata: a cotação está abaixo do preço justo';

  @override
  String get valuationVerdictExpensive =>
      'Cara: a cotação está no preço justo ou acima';

  @override
  String get valuationVerdictCheapShort => 'Barata';

  @override
  String get valuationVerdictExpensiveShort => 'Cara';

  @override
  String get valuationSavedList => 'Valuations salvos';

  @override
  String valuationSavedEmpty(String ticker) {
    return 'Nenhum valuation salvo para $ticker ainda.';
  }

  @override
  String valuationSavedDetail(
    String price,
    String fair,
    String margin,
    String verdict,
    String date,
  ) {
    return 'Cotação $price · justo $fair · margem $margin · $verdict · $date';
  }

  @override
  String get valuationDelete => 'Excluir';

  @override
  String get valuationModelBazin => 'Bazin (preço-teto por dividendos)';

  @override
  String get valuationModelGraham => 'Graham (valor intrínseco)';

  @override
  String get valuationModelGordon => 'Gordon (crescimento de dividendos)';

  @override
  String get valuationModelDcf => 'DCF (fluxo de caixa descontado)';

  @override
  String get valuationModelBanks => 'Bancos (P/VP via ROE)';

  @override
  String get valuationModelRim => 'RIM (lucro residual)';

  @override
  String get valuationModelRnav => 'RNAV (valor líquido dos ativos)';

  @override
  String get valuationModelProjectedCeiling => 'Teto projetado';

  @override
  String get valuationFieldAverageDividend =>
      'Dividendo médio anual por ação (R\$)';

  @override
  String get valuationFieldDesiredYield => 'Yield desejado (%)';

  @override
  String get valuationFieldEps => 'Lucro por ação, LPA (R\$)';

  @override
  String get valuationFieldBookValuePerShare =>
      'Valor patrimonial por ação, VPA (R\$)';

  @override
  String get valuationFieldCurrentDividend =>
      'Dividendo anual atual por ação (R\$)';

  @override
  String get valuationFieldExpectedGrowth => 'Crescimento esperado (%)';

  @override
  String get valuationFieldKe => 'Custo do capital próprio, Ke (%)';

  @override
  String get valuationFieldEbit => 'EBIT (R\$)';

  @override
  String get valuationFieldTaxRate => 'Alíquota de IR (%)';

  @override
  String get valuationFieldDepreciationAmortization =>
      'Depreciação e amortização (R\$)';

  @override
  String get valuationFieldCapex => 'Capex (R\$)';

  @override
  String get valuationFieldNwcChange => 'Variação do capital de giro (R\$)';

  @override
  String get valuationFieldTotalDebt => 'Dívida total (R\$)';

  @override
  String get valuationFieldCash => 'Caixa (R\$)';

  @override
  String get valuationFieldSharesOutstanding => 'Ações em circulação (milhões)';

  @override
  String get valuationFieldBeta => 'Beta';

  @override
  String get valuationFieldRiskFreeRate => 'Taxa livre de risco (%)';

  @override
  String get valuationFieldMarketRiskPremium =>
      'Prêmio de risco de mercado (%)';

  @override
  String get valuationFieldKd => 'Custo da dívida, Kd (%)';

  @override
  String get valuationFieldPerpetuityGrowth =>
      'Crescimento na perpetuidade (%)';

  @override
  String get valuationFieldRoe => 'ROE (%)';

  @override
  String get valuationFieldPayout => 'Payout (%)';

  @override
  String get valuationFieldRoeCurrent => 'ROE atual (%)';

  @override
  String get valuationFieldFadeYears => 'Anos para o ROE convergir ao Ke';

  @override
  String get valuationFieldLandbank => 'Landbank (R\$)';

  @override
  String get valuationFieldInventoryAtMarketValue =>
      'Estoque a valor de mercado (R\$)';

  @override
  String get valuationFieldNetCash => 'Caixa líquido (R\$)';

  @override
  String get valuationFieldProjectionYears => 'Anos de projeção';

  @override
  String valuationErrMissingInput(String field) {
    return 'Preencha o campo: $field';
  }

  @override
  String valuationErrInputNotFinite(String field) {
    return 'Número inválido em: $field';
  }

  @override
  String get valuationErrPriceMustBePositive =>
      'A cotação precisa ser maior que zero';

  @override
  String get valuationErrDesiredYieldMustBePositive =>
      'O yield desejado precisa ser maior que zero';

  @override
  String get valuationErrEpsAndBookValueMustBePositive =>
      'LPA e VPA precisam ser maiores que zero';

  @override
  String get valuationErrKeMustExceedGrowth =>
      'O custo do capital próprio (Ke) precisa ser maior que o crescimento';

  @override
  String get valuationErrWaccMustExceedGrowth =>
      'A taxa de desconto (WACC) precisa ser maior que o crescimento na perpetuidade';

  @override
  String get valuationErrSharesMustBePositive =>
      'As ações em circulação precisam ser maiores que zero';

  @override
  String get valuationErrFadeYearsMustBePositive =>
      'Os anos de convergência precisam ser pelo menos 1';

  @override
  String valuationErrWholeNumberRequired(String field) {
    return '$field precisa ser um número inteiro';
  }

  @override
  String get valuationErrProjectionYearsNegative =>
      'Os anos de projeção não podem ser negativos';

  @override
  String get valuationErrFairPriceMustBePositive =>
      'Essas premissas dão um preço justo zero ou negativo, então não há valuation. Revise os números.';

  @override
  String get swapIntro =>
      'Trocar uma moeda por outra conta como venda da moeda entregue para o IR: entra nas vendas do mês (limite de R\$ 35 mil) e realiza ganho sobre o custo médio. Grava uma venda e uma compra pelo mesmo valor.';

  @override
  String get swapNeedTwoCoins =>
      'É preciso ter saldo em uma cripto e outra cadastrada.';

  @override
  String get swapCoinOut => 'Moeda que sai';

  @override
  String swapCoinOutItem(String ticker, String balance) {
    return '$ticker (saldo $balance)';
  }

  @override
  String get swapQuantityOut => 'Quantidade que sai';

  @override
  String get swapCoinIn => 'Moeda que entra';

  @override
  String get swapQuantityIn => 'Quantidade que entra';

  @override
  String get swapMarketValue => 'Valor de mercado da troca (BRL)';

  @override
  String get swapUseMarketPrice => 'Usar preço de mercado';

  @override
  String get swapSearching => 'Buscando...';

  @override
  String get swapFee => 'Taxa (opcional, BRL)';

  @override
  String get swapCustody => 'Custódia (opcional)';

  @override
  String get swapNoCustody => 'Nenhuma';

  @override
  String swapDate(String date) {
    return 'Data: $date';
  }

  @override
  String get swapRegister => 'Registrar troca';

  @override
  String swapHintNoBrlQuote(String ticker) {
    return 'Sem cotação em reais para $ticker.';
  }

  @override
  String swapHintTodayPrice(String date) {
    return 'Preço de hoje ($date); se a troca foi em outra data, ajuste o valor.';
  }

  @override
  String get swapHintQuoteUnavailable =>
      'Cotação indisponível agora; digite o valor.';

  @override
  String swapHintNoQuote(String ticker) {
    return 'Sem cotação para $ticker; digite o valor.';
  }

  @override
  String get swapFeeNotNumber => 'A taxa precisa ser um número.';

  @override
  String get swapErrNeedTwoDifferentCoins =>
      'A permuta precisa de duas moedas diferentes.';

  @override
  String swapErrNotCrypto(String ticker) {
    return '$ticker não é cripto; a permuta só vale entre criptomoedas.';
  }

  @override
  String swapErrNotBrl(String ticker, String currency) {
    return '$ticker está em $currency; a permuta é apurada em reais.';
  }

  @override
  String get swapErrQuantityOutMustBePositive =>
      'O campo quantidade que sai deve ser maior que zero.';

  @override
  String get swapErrQuantityInMustBePositive =>
      'O campo quantidade que entra deve ser maior que zero.';

  @override
  String get swapErrValueMustBePositive =>
      'O campo valor deve ser maior que zero.';

  @override
  String get swapErrNegativeFee => 'A taxa não pode ser negativa.';

  @override
  String swapErrInsufficientBalance(
    String ticker,
    String available,
    String requested,
  ) {
    return 'Saldo insuficiente de $ticker: tem $available e a troca entrega $requested.';
  }

  @override
  String get fundTaxTitle => 'IR de fundos';

  @override
  String get fundTaxDisclaimer =>
      'Estimativa com as transações e as cotas lançadas neste app. O come-cotas e o IR do resgate saem do fundo na fonte: não há DARF. As regras vêm de fontes secundárias; confira na Receita e no extrato do fundo.';

  @override
  String get fundRefreshCvm => 'Atualizar da CVM';

  @override
  String get fundCvmDownloadWarning =>
      'Baixa da CVM um arquivo de até ~12 MB por mês lido (em geral 4 meses, uns 40 MB). Continuar?';

  @override
  String get fundDownload => 'Baixar';

  @override
  String get fundProgressPreparing => 'Preparando…';

  @override
  String fundProgressReading(int done, int total) {
    return 'Lendo mês $done de $total…';
  }

  @override
  String fundCvmReadFailed(String error) {
    return 'Não foi possível ler a CVM: $error';
  }

  @override
  String get fundLossCreditsHeader =>
      'Prejuízo a compensar em ganhos e come-cotas posteriores (mesma administradora e mesmo tipo)';

  @override
  String fundLossCreditLine(
    String amount,
    String tickers,
    String administrator,
    String term,
  ) {
    return '$amount — $tickers ($administrator, $term)';
  }

  @override
  String get fundNoAdministrator => 'sem administradora';

  @override
  String get fundNone => 'Nenhum fundo cadastrado ainda.';

  @override
  String get fundStatusNoPosition => 'Sem cotas na carteira.';

  @override
  String get fundStatusNoTerm =>
      'Escolha o tipo do fundo para estimar o come-cotas e o IR do resgate.';

  @override
  String get fundStatusNoQuota =>
      'Lance o valor da cota para estimar o come-cotas.';

  @override
  String get fundStatusExempt =>
      'Fundo de ações ou entidade de investimento: sem come-cotas.';

  @override
  String fundStatusEstimated(String date, String tax, String quotas) {
    return 'Próximo come-cotas em $date: $tax ($quotas cotas)';
  }

  @override
  String fundBasis(String basis, String origin, String gain, String rate) {
    return 'Base por cota $basis ($origin) · ganho $gain × $rate%';
  }

  @override
  String get fundBasisLastComeCotas => 'valor da cota no último come-cotas';

  @override
  String get fundBasisEstoque =>
      'custo médio mais o estoque de 2023 já tributado';

  @override
  String get fundBasisAverageCost => 'custo médio';

  @override
  String get fundEstoqueNotTaxed =>
      'Diga como o estoque de 2023 foi tributado para estimar o imposto e o custo do come-cotas.';

  @override
  String get fundEstoqueNeedsQuota =>
      'Lance a cota de 31/12/2023 (ou de até 10 dias antes) para estimar o estoque.';

  @override
  String fundEstoqueEstimated(
    String base,
    String date,
    String quota,
    String rate,
    String tax,
  ) {
    return 'Estoque de 2023: ganho de $base até $date (cota $quota) a $rate% = $tax';
  }

  @override
  String fundRefundable(String total) {
    return 'Come-cotas pago acima do IR devido: $total. A lei não prevê restituição desse excesso (IN RFB 1.585/2015, art. 15; Lei 14.754/2023, art. 17, §6). É imposto pago sobre rendimento que depois sumiu, e ele volta como perda a compensar em ganhos posteriores. O app já soma essa perda ao crédito de prejuízo; o valor do excesso em si não está incluído nos líquidos.';
  }

  @override
  String fundWarnStaleQuota(int days) {
    return 'O último valor de cota tem mais de $days dias: a estimativa pode estar velha.';
  }

  @override
  String fundWarnUnrecordedDeduction(String date) {
    return 'O come-cotas de $date já passou sem desconto registrado: lance a transação Come-cotas para a base ficar certa.';
  }

  @override
  String fundWarnCreditNotInformed(String date, String balance) {
    return 'O come-cotas de $date foi registrado com $balance de crédito de prejuízo no grupo: a administradora pode ter usado parte dele, então o saldo de crédito mostrado pode estar acima do real. Preencha o crédito de prejuízo usado nessa transação para resolver.';
  }

  @override
  String fundWarnCreditExceedsBalance(
    String date,
    String informed,
    String balance,
  ) {
    return 'O come-cotas de $date diz que $informed de crédito de prejuízo foram usados, mas o grupo só tinha $balance: só esse saldo foi abatido. Confira o valor.';
  }

  @override
  String fundWarnRedemptionUncovered(String date) {
    return 'Resgate de $date maior que as aplicações lançadas: o custo considera só a parte conhecida.';
  }

  @override
  String fundWarnRedemptionExcess(String date, String amount) {
    return 'No resgate de $date o come-cotas pago passou do IR devido em $amount: fica fora do líquido (veja a nota sobre o excesso).';
  }

  @override
  String fundCreditReduced(String used, String before) {
    return 'Crédito de prejuízo de $used já reduziu o ganho: o imposto seria $before sem ele.';
  }

  @override
  String fundRedemptionCreditLine(String used, String ir, String before) {
    return 'Prejuízo de resgates anteriores do grupo abateu $used do ganho: IR $ir em vez de $before.';
  }

  @override
  String fundRedemptionLossLine(String loss) {
    return 'Este resgate perdeu $loss: compensa o ganho de resgates e come-cotas posteriores do mesmo grupo (mesma administradora e mesmo tipo).';
  }

  @override
  String fundRecordHead(String administrator, String date) {
    return 'Todos os fundos que você lançou da administradora $administrator foram resgatados até $date.';
  }

  @override
  String fundRecordExpired(String head, String balance, String until) {
    return '$head A obrigação da administradora de guardar o registro das perdas ($balance) terminou em $until (IN RFB 1.585, art. 15 §2): confira se ela ainda tem o registro antes de contar com o saldo.';
  }

  @override
  String fundRecordActive(String head, String balance, String until) {
    return '$head A administradora só é obrigada a guardar o registro das perdas ($balance) até $until (IN RFB 1.585, art. 15 §2). A lei não limita o seu direito de compensar, mas confira com a administradora.';
  }

  @override
  String fundCvmLatest(String date) {
    return ' Última cota da CVM: $date.';
  }

  @override
  String fundCvmRead(String latest) {
    return 'Cotas lidas da CVM.$latest';
  }

  @override
  String get fundCvmNoCnpj => 'Informe o CNPJ do fundo para ler a cota da CVM.';

  @override
  String fundCvmPending(String latest) {
    return 'Ainda não lida.$latest';
  }

  @override
  String get fundCvmNotFound =>
      'A CVM não trouxe cota para este CNPJ (ou subclasse) nos meses lidos. Confira o CNPJ.';

  @override
  String get fundCvmNeedsSubclass =>
      'Este CNPJ só tem subclasses no informe: escolha uma.';

  @override
  String fundCvmError(String message, String latest) {
    return 'Não foi possível ler a CVM agora ($message). Tente de novo em instantes.$latest';
  }

  @override
  String get fundCvmErrorDefault => 'erro';

  @override
  String get fundTermLabel => 'Tipo do fundo';

  @override
  String get fundStructureLabel => 'Estrutura do fundo (Lei 14.754)';

  @override
  String get fundEstoqueLabel => 'Estoque de 2023 (arts. 27 e 28)';

  @override
  String get fundEstoqueNotInformed => 'Não tributado / não informado';

  @override
  String get fundAdministratorLabel =>
      'Administradora / intermediário (compensa prejuízo)';

  @override
  String get fundAdministratorHint => 'ex.: XP, BTG';

  @override
  String get fundAdministratorHelpNoTerm => 'Escolha o tipo do fundo primeiro.';

  @override
  String get fundAdministratorHelpNone =>
      'Sem isso, o fundo só compensa prejuízo com ele mesmo.';

  @override
  String get fundAdministratorHelpSet =>
      'Fundos do mesmo tipo e da mesma administradora compensam entre si.';

  @override
  String get fundSaveAdministrator => 'Salvar administradora';

  @override
  String get fundCnpjLabel => 'CNPJ do fundo (cota pela CVM)';

  @override
  String get fundSaveCnpj => 'Salvar CNPJ';

  @override
  String get fundSubclassLabel => 'Subclasse da CVM';

  @override
  String get fundSubclassNone => 'Classe (sem subclasse)';

  @override
  String fundQuotasTitle(int count) {
    return 'Valor da cota ($count)';
  }

  @override
  String get fundDeleteQuota => 'Apagar cota';

  @override
  String get fundAddQuota => 'Lançar cota';

  @override
  String fundDeductionsTitle(int count) {
    return 'Come-cotas registrados ($count)';
  }

  @override
  String fundDeductionLine(
    String quantity,
    String price,
    String tax,
    String credit,
  ) {
    return '$quantity cotas a $price · imposto $tax$credit';
  }

  @override
  String fundDeductionCredit(String value) {
    return ' · crédito usado $value';
  }

  @override
  String fundRedemptionsTitle(int count) {
    return 'Resgates e amortizações estimados ($count)';
  }

  @override
  String fundRedemptionAmortization(String date, String quota) {
    return '$date — amortização (cota a $quota; as cotas ficam)';
  }

  @override
  String fundRedemptionTitle(String date, String quotas, String quota) {
    return '$date — $quotas cotas a $quota';
  }

  @override
  String fundRedemptionDetail(String gross, String iof, String ir, String net) {
    return 'Bruto $gross · IOF $iof · IR $ir · líquido $net';
  }

  @override
  String fundRedemptionComeCotasCredit(String value) {
    return 'Come-cotas já pago abateu $value do IR';
  }

  @override
  String fundQuotaDialogTitle(String ticker) {
    return 'Cota de $ticker';
  }

  @override
  String get fundQuotaValueLabel => 'Valor da cota (R\$)';

  @override
  String fundDate(String date) {
    return 'Data: $date';
  }

  @override
  String get fundTermLongTerm => 'Longo prazo';

  @override
  String get fundTermShortTerm => 'Curto prazo';

  @override
  String get fundTermEquity => 'Ações';

  @override
  String get fundTermInvestmentEntity => 'Entidade de investimento';

  @override
  String get fundStructureOpen => 'Aberto';

  @override
  String get fundStructureClosed => 'Fechado';

  @override
  String get fundStructureExclusive => 'Exclusivo';

  @override
  String get fundEstoqueGeneral => 'Regra geral (15%)';

  @override
  String get fundEstoqueOption8 => 'Opção (8%)';

  @override
  String get fundErrNeedTermBeforeStructure =>
      'Escolha o tipo do fundo antes da estrutura';

  @override
  String get fundErrNeedTermBeforeEstoque =>
      'Escolha o tipo do fundo antes do estoque';

  @override
  String get fundErrNeedTermBeforeAdministrator =>
      'Escolha o tipo do fundo antes da administradora';

  @override
  String get fundErrNeedTermBeforeSubclass =>
      'Escolha o tipo do fundo antes da subclasse da CVM';

  @override
  String get fundErrCnpjMustHave14Digits => 'O CNPJ precisa ter 14 dígitos';

  @override
  String get foreignTitle => 'IR de exterior';

  @override
  String get foreignIntroNote =>
      'Calculado só com as transações lançadas neste app, a custo médio, com a taxa de cada operação e em reais pela PTAX do Banco Central (fechamento, venda; compra no imposto retido no exterior), e no dia sem cotação pela última anterior. Onde a PTAX falta, vale o câmbio de mercado (Yahoo) e o ano avisa que é aproximado. Vai na declaração anual, sem DARF mensal. As regras vêm de fontes secundárias: confira na Receita antes de pagar.';

  @override
  String get foreignFxUnavailableNote =>
      'Câmbio indisponível agora para alguma moeda (nem PTAX nem de mercado): as operações nela ficaram de fora. Tente de novo em instantes.';

  @override
  String get foreignEmpty =>
      'Nenhuma venda ou provento de ativo do exterior a partir de 2024 ainda.';

  @override
  String foreignStatusTaxDue(String value) {
    return 'Imposto estimado de $value na declaração';
  }

  @override
  String foreignStatusCovered(String value) {
    return 'Sem imposto a pagar: o retido no exterior cobre os $value devidos';
  }

  @override
  String foreignStatusLoss(String value) {
    return 'Sem imposto: o ano fechou com prejuízo de $value, que abate os anos seguintes';
  }

  @override
  String get foreignStatusFitInLoss =>
      'Sem imposto: o resultado do ano coube no prejuízo de anos anteriores';

  @override
  String foreignRatePtax(String rate) {
    return 'Câmbio $rate (PTAX)';
  }

  @override
  String foreignRateMarket(String rate) {
    return 'Câmbio $rate (mercado)';
  }

  @override
  String foreignWarnCostUnknown(String ticker, String date) {
    return 'Venda de $ticker em $date maior que as compras lançadas: o ganho considera só a parte com custo conhecido. Lance as compras que faltam.';
  }

  @override
  String foreignWarnFxMissing(int year) {
    return 'Operações ou proventos de $year sem câmbio (nem PTAX nem de mercado) para a data: ficaram de fora da apuração.';
  }

  @override
  String foreignWarnFxApproximate(int year) {
    return 'Operações ou proventos de $year foram convertidos pelo câmbio de mercado porque a PTAX do Banco Central não estava disponível para eles (sem rede, moeda que a PTAX não cota ou buraco longo): os valores do ano são aproximados.';
  }

  @override
  String foreignCapitalGain(String value) {
    return 'Ganho de capital: $value';
  }

  @override
  String foreignDividends(String value) {
    return 'Proventos: $value';
  }

  @override
  String foreignLossUsed(String value) {
    return 'Prejuízo de anos anteriores usado: $value';
  }

  @override
  String foreignTaxableBase(String value) {
    return 'Base de cálculo: $value';
  }

  @override
  String foreignTax(String value, String rate) {
    return 'Imposto: $value (alíquota $rate%)';
  }

  @override
  String foreignWithheld(String value, String credit) {
    return 'Retido no exterior: $value (crédito de $credit)';
  }

  @override
  String foreignLossCarriedOut(String value) {
    return 'Prejuízo que abate os anos seguintes: $value';
  }

  @override
  String foreignSalesCount(int count) {
    return 'Vendas ($count)';
  }

  @override
  String foreignDividendsCount(int count) {
    return 'Proventos ($count)';
  }

  @override
  String foreignItemTitle(String ticker, String date) {
    return '$ticker — $date';
  }

  @override
  String foreignSaleDetail(String rate, String sale, String cost, String gain) {
    return '$rate · venda $sale · custo $cost · ganho $gain';
  }

  @override
  String foreignDividendDetail(String rate, String gross, String withheld) {
    return '$rate · $gross$withheld';
  }

  @override
  String foreignDividendWithheld(String value) {
    return ' · retido $value';
  }

  @override
  String get foreignHoldingsTitle => 'Bens e Direitos';

  @override
  String get foreignHoldingsNote =>
      'Custo de aquisição em reais, não valor de mercado, só do que está lançado neste app. Uma linha por custódia: o custo de cada uma é a quantidade dela vezes o custo médio do ativo, então as custódias somam a posição. Grupo, código e país da ficha são seus: preencha pelo programa da Receita (as fontes divergem e o app não confere; só completa zeros, então 3 vira 03). Entram na descrição copiada e no CSV. Estimativa de fontes secundárias: confira no programa da Receita.';

  @override
  String get foreignHoldingsLegend =>
      '\"aproximado\": alguma compra sem PTAX usou o câmbio de mercado. \"incompleto\": uma operação ficou sem câmbio, ou uma venda foi maior que as compras lançadas, e o custo está subestimado.';

  @override
  String foreignReturnOf(int year) {
    return 'Declaração de $year';
  }

  @override
  String get foreignExportCsv => 'Exportar CSV';

  @override
  String foreignNoPosition(String previous, String current) {
    return 'Nenhuma posição no exterior em $previous nem em $current.';
  }

  @override
  String get foreignNoCustody => 'Sem custódia';

  @override
  String foreignHoldingNone(String date) {
    return '$date: —';
  }

  @override
  String foreignHoldingValue(
    String date,
    String cost,
    String quantity,
    String approximate,
    String incomplete,
  ) {
    return '$date: $cost ($quantity un.)$approximate$incomplete';
  }

  @override
  String get foreignApproximate => ' · aproximado';

  @override
  String get foreignIncomplete => ' · incompleto';

  @override
  String foreignFichaLine(String ficha) {
    return 'Ficha: $ficha';
  }

  @override
  String get foreignFichaNotFilled => 'não preenchida';

  @override
  String foreignFichaGroup(String value) {
    return 'Grupo $value';
  }

  @override
  String foreignFichaCode(String value) {
    return 'Código $value';
  }

  @override
  String foreignFichaCountry(String value) {
    return 'País $value';
  }

  @override
  String get foreignEditFichaTooltip => 'Editar grupo, código e país da ficha';

  @override
  String get foreignCopyTooltip => 'Copiar descrição';

  @override
  String get foreignDescriptionCopied => 'Descrição copiada.';

  @override
  String foreignFichaDialogTitle(String ticker) {
    return 'Ficha de $ticker';
  }

  @override
  String get foreignFieldGroup => 'Grupo';

  @override
  String get foreignFieldCode => 'Código';

  @override
  String get foreignFieldCountry => 'País';

  @override
  String get foreignFichaHelp =>
      'Copie do programa da Receita; o app não confere. Vazio apaga.';

  @override
  String get foreignFichaInvalid =>
      'Ficha inválida. Use só dígitos: grupo e código até 2, país até 3.';

  @override
  String get foreignCsvDialogTitle => 'Salvar informe de Bens e Direitos';

  @override
  String foreignCsvSaved(int count) {
    return '$count linhas salvas.';
  }

  @override
  String foreignCsvFailed(String error) {
    return 'Não consegui salvar o CSV: $error';
  }

  @override
  String foreignDescHead(String ticker, String name, String custody) {
    return '$ticker — $name.$custody';
  }

  @override
  String foreignDescCustody(String label) {
    return ' Custódia: $label.';
  }

  @override
  String foreignDescFicha(String ficha) {
    return 'Ficha: $ficha.';
  }

  @override
  String foreignDescPosition(String date, String units, String cost) {
    return 'Situação em $date: $units, custo de aquisição $cost.';
  }

  @override
  String get foreignDescNoPosition => 'sem posição';

  @override
  String foreignDescUnits(String quantity) {
    return '$quantity un.';
  }

  @override
  String get foreignDescBasis =>
      'Custo de aquisição convertido em reais pela cotação de fechamento (venda) do Banco Central na data de cada compra.';

  @override
  String tickerLabel(String assetClass) {
    return 'Ticker ($assetClass)';
  }

  @override
  String get classLabel => 'Classe';

  @override
  String get searchButton => 'Buscar';

  @override
  String get quoteSourceUnavailable =>
      'Fonte de cotação indisponível agora (limite ou rede) — tente de novo em instantes.';

  @override
  String quoteSearchFailed(String ticker, String assetClass) {
    return 'Não foi possível buscar a cotação de $ticker ($assetClass)';
  }

  @override
  String get hintRealEstate => 'Apto Centro';

  @override
  String get hintUnlistedCompany => 'Empresa XYZ';

  @override
  String get hintFund => 'Fundo XYZ FIC FIM';

  @override
  String get hintFixedIncome => 'CDB Banco X';

  @override
  String get addAssetNameLabel => 'Nome';

  @override
  String addAssetLookupFailed(String ticker, String assetClass) {
    return 'Não foi possível buscar $ticker ($assetClass) — confira o ticker.';
  }

  @override
  String get addAssetFundTerm => 'Tipo do fundo (para o come-cotas)';

  @override
  String get addAssetEquity => 'Participação societária';

  @override
  String addAssetEquityPercent(String title, String percent) {
    return '$title ($percent%)';
  }

  @override
  String addAssetEquityPercentValue(
    String title,
    String percent,
    String value,
  ) {
    return '$title ($percent% — $value)';
  }

  @override
  String get addAssetSharesOwned => 'Cotas/ações possuídas';

  @override
  String get addAssetTotalShares => 'Total de cotas/ações da empresa';

  @override
  String addAssetValuation(String currency) {
    return 'Valuation da empresa ($currency)';
  }

  @override
  String get custodianInstitution => 'Instituição';

  @override
  String get custodianHolder => 'Titular';

  @override
  String get custodianAdd => 'Adicionar';

  @override
  String get custodianNone => 'Nenhuma custódia cadastrada ainda.';

  @override
  String get custodianEditTitle => 'Editar custódia';

  @override
  String get ptxTitleNew => 'Lançar transação';

  @override
  String get ptxTitleEdit => 'Editar transação';

  @override
  String get ptxType => 'Tipo';

  @override
  String get ptxNeedFund => 'Cadastre um ativo da classe Fundo primeiro.';

  @override
  String get ptxNeedAsset => 'Cadastre um ativo primeiro, na aba Portfolio.';

  @override
  String get ptxAsset => 'Ativo';

  @override
  String get ptxIssuer => 'Emissor (opcional)';

  @override
  String get ptxIndex => 'Indexador';

  @override
  String get ptxRate => 'Taxa contratada (%)';

  @override
  String ptxMaturity(String date) {
    return 'Vencimento: $date';
  }

  @override
  String get ptxMaturityOptional => 'Vencimento (opcional)';

  @override
  String get ptxLiquidity => 'Liquidez';

  @override
  String get ptxQuantity => 'Quantidade';

  @override
  String get ptxQuotaValue => 'Valor da cota';

  @override
  String get ptxUnitPrice => 'Preço unitário';

  @override
  String get ptxFee => 'Taxa (opcional)';

  @override
  String ptxTotal(String value) {
    return 'Total: $value';
  }

  @override
  String get ptxAmount => 'Valor';

  @override
  String ptxForeignTax(String currency) {
    return 'Imposto retido no exterior ($currency, opcional)';
  }

  @override
  String ptxTaxPaidOn(String date) {
    return 'Imposto pago em: $date';
  }

  @override
  String get ptxTaxPaidOnHint =>
      'Data do pagamento do imposto (opcional; padrão: a do provento)';

  @override
  String get ptxLossCredit =>
      'Crédito de prejuízo usado (rendimento, R\$, opcional)';

  @override
  String get ptxLossCreditHelp =>
      'Vazio = não sei; 0 = nenhum crédito foi usado.';

  @override
  String get ptxCustodianFrom => 'Custódia (origem)';

  @override
  String get ptxCustodianOptional => 'Custódia (opcional)';

  @override
  String get ptxCustodianNone => 'Nenhuma';

  @override
  String get ptxCustodianTo => 'Custódia (destino)';

  @override
  String ptxDate(String date) {
    return 'Data: $date';
  }

  @override
  String get ptxSave => 'Salvar';

  @override
  String get ptxTypeCompra => 'Compra';

  @override
  String get ptxTypeVenda => 'Venda';

  @override
  String get ptxTypeAporte => 'Aporte';

  @override
  String get ptxTypeRetirada => 'Retirada';

  @override
  String get ptxTypeProvento => 'Provento';

  @override
  String get ptxTypeTransferencia => 'Transferência';

  @override
  String get ptxTypeComeCotas => 'Come-cotas';

  @override
  String get ptxTypeAmortizacao => 'Amortização';

  @override
  String get ptxLiqDaily => 'Diária';

  @override
  String get ptxLiqAtMaturity => 'No vencimento';

  @override
  String get ptxLiqOther => 'Outro';

  @override
  String get ptxIdxFixed => 'Prefixado';

  @override
  String get ptxIdxOther => 'Outro';

  @override
  String get ptxAmortNoShares =>
      'A amortização precisa de cotas em carteira nessa data.';

  @override
  String ptxAmortExceeds(String amount, String position) {
    return 'O valor amortizado ($amount) passa do valor da posição nessa data ($position).';
  }

  @override
  String get madTitleCompany => 'Detalhes da empresa';

  @override
  String get madTitleProperty => 'Detalhes do imóvel';

  @override
  String get madEquityTitle => 'Participação societária';

  @override
  String madEquityPct(String percent) {
    return 'Participação societária ($percent%)';
  }

  @override
  String madEquityPctValue(String percent, String value, String currency) {
    return 'Participação societária ($percent% — $value $currency)';
  }

  @override
  String get madEquityNote =>
      'O percentual é sempre calculado a partir de cotas possuídas / total de cotas da empresa — nunca digitado direto.';

  @override
  String get madSharesOwned => 'Cotas/ações possuídas';

  @override
  String get madTotalShares => 'Total de cotas/ações';

  @override
  String madCompanyValuation(String currency) {
    return 'Valuation ($currency)';
  }

  @override
  String get madSaveEquity => 'Salvar participação';

  @override
  String get madHistory => 'Histórico de avaliação';

  @override
  String madHistoryLatest(String value, String currency) {
    return 'Histórico de avaliação (última: $value $currency)';
  }

  @override
  String get madNoValuations => 'Nenhuma avaliação registrada ainda.';

  @override
  String get madPickDate => 'Escolher data';

  @override
  String madDate(String date) {
    return 'Data: $date';
  }

  @override
  String madValue(String currency) {
    return 'Valor ($currency)';
  }

  @override
  String get madNotes => 'Notas (opcional)';

  @override
  String get madAddValuation => 'Adicionar avaliação';

  @override
  String get madAttachments => 'Anexos';

  @override
  String get madNoAttachments => 'Nenhum anexo ainda.';

  @override
  String get madDocType => 'Tipo de documento (opcional)';

  @override
  String get madDocTypeHint => 'Escritura, ITBI, cap table...';

  @override
  String get madAddAttachment => 'Adicionar anexo';

  @override
  String get txhEmpty => 'Nenhuma transação lançada ainda.';

  @override
  String get txhImportTooltip => 'Importar CSV do desktop';

  @override
  String get txhExportTooltip => 'Exportar CSV para o desktop';

  @override
  String get txhExportDialogTitle => 'Salvar transações para o desktop';

  @override
  String txhExported(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count linhas salvas.',
      one: '1 linha salva.',
      zero: '0 linhas salvas.',
    );
    return '$_temp0';
  }

  @override
  String txhExportedSkipped(int count) {
    return '$count ficaram de fora (transferência, amortização ou ativo removido).';
  }

  @override
  String txhExportFailed(String error) {
    return 'Não consegui salvar o CSV: $error';
  }

  @override
  String txhImported(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count transações importadas.',
      one: '1 transação importada.',
      zero: '0 transações importadas.',
    );
    return '$_temp0';
  }

  @override
  String txhImportFailed(String error) {
    return 'Não consegui importar o CSV: $error';
  }

  @override
  String txhFee(String value) {
    return 'taxa $value';
  }

  @override
  String txhWithheld(String value) {
    return 'retido $value';
  }

  @override
  String txhPaidOn(String date) {
    return 'pago em $date';
  }

  @override
  String txhLossCreditUsed(String value) {
    return 'crédito usado $value';
  }

  @override
  String get txhPreviewTitle => 'Importar do desktop';

  @override
  String txhPreviewNewTransactions(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count transações novas',
      one: '1 transação nova',
      zero: '0 transações novas',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewNewAssets(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ativos novos',
      one: '1 ativo novo',
      zero: '0 ativos novos',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewNewCustodians(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count custódias novas',
      one: '1 custódia nova',
      zero: '0 custódias novas',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewDuplicates(int count) {
    return '$count já existem (ignoradas)';
  }

  @override
  String txhPreviewSkipped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count transferências ignoradas (o CSV não traz o destino)',
      one: '1 transferência ignorada (o CSV não traz o destino)',
      zero: '0 transferências ignoradas (o CSV não traz o destino)',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count linhas com problema (não entram):',
      one: '1 linha com problema (não entra):',
      zero: '0 linhas com problema (não entram):',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewIssueLine(int line, String message) {
    return 'linha $line: $message';
  }

  @override
  String txhPreviewMore(int count) {
    return '… e mais $count';
  }

  @override
  String get txhCancel => 'Cancelar';

  @override
  String get txhImport => 'Importar';

  @override
  String get importIssueEmptyFile => 'arquivo vazio';

  @override
  String importIssueMissingColumn(String name) {
    return 'coluna \'$name\' ausente no cabeçalho';
  }

  @override
  String importIssueInvalidDate(String raw) {
    return 'data inválida: \'$raw\' (esperado AAAA-MM-DD ou DD/MM/AAAA)';
  }

  @override
  String importIssueUnknownType(String raw) {
    return 'tipo desconhecido: \'$raw\'';
  }

  @override
  String importIssueInvalidNumber(String name, String raw) {
    return 'número inválido em \'$name\': \'$raw\'';
  }

  @override
  String get importIssueTotalEmpty => 'total_value vazio';

  @override
  String importIssueUnknownClass(String raw) {
    return 'classe desconhecida: \'$raw\'';
  }

  @override
  String importIssueTickerRequired(String type) {
    return 'ticker obrigatório para $type';
  }

  @override
  String importIssueNewAssetNeedsClass(String ticker) {
    return 'ativo novo \'$ticker\': informe a asset_class';
  }

  @override
  String importIssueFundOnly(String type) {
    return '$type só vale para ativo da classe fundo';
  }

  @override
  String importIssueQuantityRequired(String type) {
    return 'quantity obrigatória para $type';
  }

  @override
  String importIssueUnitPriceRequired(String type) {
    return 'unit_price obrigatório para $type';
  }
}

import '../data/db.dart';
import '../domain/fx_history.dart';
import '../domain/profitability.dart';
import '../domain/profitability_error.dart';
import '../domain/yahoo_monthly_history.dart';
import '../models/asset.dart';
import '../models/portfolio_transaction.dart';
import 'portfolio_repository.dart';
import 'quote_exceptions.dart';
import 'yahoo_quote_service.dart';

/// O que o botão "atualizar histórico de preços" fez, por ticker.
class PriceRefreshSummary {
  final List<String> updated;
  final List<String> failed;

  const PriceRefreshSummary({required this.updated, required this.failed});
}

/// Guarda o fechamento mensal dos ativos e monta a entrada da rentabilidade (TWR). O Yahoo e
/// o câmbio entram por parâmetro para o teste rodar sem rede.
class ProfitabilityRepository {
  final PortfolioRepository _portfolio;
  /// Fontes de dados; o padrão é o Yahoo. Injetáveis para teste, sem rede.
  final Future<MonthlyPriceHistory> Function(String symbol)? fetchHistory;
  final Future<FxHistory> Function(String currency, DateTime from)? fetchFx;
  final DateTime Function() _now;
  late final YahooQuoteService _yahoo = YahooQuoteService();

  ProfitabilityRepository({
    PortfolioRepository? portfolio,
    this.fetchHistory,
    this.fetchFx,
    DateTime Function()? now,
  }) : _portfolio = portfolio ?? PortfolioRepository(),
       _now = now ?? DateTime.now;

  static const _relevantTypes = {
    TransactionType.compra,
    TransactionType.venda,
    TransactionType.provento,
  };

  /// Ativos de classe medida que têm ao menos uma compra, venda ou provento.
  List<Asset> _measured(List<Asset> assets, List<PortfolioTransaction> transactions) {
    final withTransactions = {
      for (final tx in transactions)
        if (tx.assetId != null && _relevantTypes.contains(tx.type)) tx.assetId!,
    };
    return [
      for (final asset in assets)
        if (profitabilityAssetClasses.contains(asset.assetClass) && withTransactions.contains(asset.id)) asset,
    ];
  }

  /// Busca o histórico mensal de cada ativo medido, um de cada vez (para não bater no limite
  /// do Yahoo), e troca as linhas do ticker. Quem falha entra em `failed` e mantém os dados
  /// antigos; os outros seguem.
  Future<PriceRefreshSummary> refreshPriceHistory() async {
    final assets = await _portfolio.listAssets();
    final transactions = await _portfolio.listAllTransactions();
    final fetch = fetchHistory ?? _yahoo.fetchMonthlyHistory;
    final updated = <String>[];
    final failed = <String>[];
    for (final asset in _measured(assets, transactions)) {
      final symbol = YahooQuoteService.symbolFor(asset);
      if (symbol == null) continue;
      try {
        await _replace(asset.ticker, await fetch(symbol));
        updated.add(asset.ticker);
      } on QuoteNotFoundException {
        failed.add(asset.ticker);
      } on QuoteUnavailableException {
        failed.add(asset.ticker);
      }
    }
    return PriceRefreshSummary(updated: updated, failed: failed);
  }

  Future<void> _replace(String ticker, MonthlyPriceHistory history) async {
    final db = await AppDatabase.instance();
    await db.transaction((txn) async {
      await txn.delete('price_month_close', where: 'ticker = ?', whereArgs: [ticker]);
      await txn.delete('price_split_event', where: 'ticker = ?', whereArgs: [ticker]);
      for (final entry in history.closes.entries) {
        await txn.insert('price_month_close', {'ticker': ticker, 'month': entry.key, 'close': entry.value});
      }
      for (final split in history.splits) {
        await txn.insert('price_split_event', {'ticker': ticker, 'split_date': _isoDay(split)});
      }
    });
  }

  static String _isoDay(DateTime d) =>
      '${d.year.toString().padLeft(4, '0')}-${d.month.toString().padLeft(2, '0')}-${d.day.toString().padLeft(2, '0')}';

  Future<FxHistory> _loadFx(String currency, DateTime from) {
    final custom = fetchFx;
    if (custom != null) return custom(currency, from);
    return _yahoo.fetchFxHistoryWithFallback(from: from, currency: currency);
  }

  /// Rentabilidade com o que está gravado. O câmbio só é buscado (na rede) quando há ativo
  /// medido em moeda estrangeira; sem ele, a moeda vira erro nomeado de câmbio faltando.
  Future<ProfitabilityResult> compute() async {
    final assets = await _portfolio.listAssets();
    final transactions = await _portfolio.listAllTransactions();
    final measured = _measured(assets, transactions);

    final db = await AppDatabase.instance();
    final closes = <String, Map<String, double>>{};
    for (final row in await db.query('price_month_close')) {
      (closes[row['ticker']! as String] ??= {})[row['month']! as String] = (row['close']! as num).toDouble();
    }
    final splits = <String, List<DateTime>>{};
    for (final row in await db.query('price_split_event')) {
      (splits[row['ticker']! as String] ??= []).add(DateTime.parse('${row['split_date']}T00:00:00Z'));
    }

    final fx = <String, FxHistory>{};
    final measuredIds = {for (final a in measured) a.id!};
    for (final currency in {for (final a in measured) a.currency}) {
      if (currency == 'BRL') continue;
      final firstDate = transactions
          .where((tx) => tx.assetId != null && measuredIds.contains(tx.assetId) && _relevantTypes.contains(tx.type))
          .where((tx) => measured.firstWhere((a) => a.id == tx.assetId).currency == currency)
          .map((tx) => tx.date)
          .reduce((a, b) => a.isBefore(b) ? a : b);
      try {
        fx[currency] = await _loadFx(currency, firstDate);
      } on QuoteNotFoundException {
        throw ProfitabilityError(ProfitabilityErrorCode.missingFx, {'currency': currency});
      } on QuoteUnavailableException {
        throw ProfitabilityError(ProfitabilityErrorCode.missingFx, {'currency': currency});
      }
    }

    return computeProfitability(
      assets: measured,
      transactions: transactions,
      monthCloses: closes,
      splits: splits,
      fxToBrl: fx,
      today: _now(),
    );
  }
}

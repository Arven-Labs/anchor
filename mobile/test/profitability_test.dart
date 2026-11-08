import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/fx_history.dart';
import 'package:anchor_mobile/domain/profitability.dart';
import 'package:anchor_mobile/domain/profitability_error.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';

// Rentabilidade mensal (porta de `compute_profitability`, desktop). Dados fictícios; os
// números esperados saem da fórmula do Dietz Modificado escrita à mão em cada teste.
Asset _asset(
  int id,
  String ticker, {
  AssetClass assetClass = AssetClass.acaoBr,
  String currency = 'BRL',
}) => Asset(
  id: id,
  ticker: ticker,
  name: 'Empresa $ticker',
  assetClass: assetClass,
  currency: currency,
  createdAt: DateTime(2026, 1, 1),
);

PortfolioTransaction _tx(
  int? assetId,
  TransactionType type,
  DateTime date, {
  double? quantity,
  required double total,
  double? fee,
}) => PortfolioTransaction(
  assetId: assetId,
  type: type,
  quantity: quantity,
  totalValue: total,
  fee: fee,
  date: date,
  createdAt: date,
);

PortfolioTransaction _buy(int id, DateTime d, double qty, double total, {double? fee}) =>
    _tx(id, TransactionType.compra, d, quantity: qty, total: total, fee: fee);

PortfolioTransaction _sell(int id, DateTime d, double qty, double total) =>
    _tx(id, TransactionType.venda, d, quantity: qty, total: total);

ProfitabilityResult _run({
  List<Asset>? assets,
  required List<PortfolioTransaction> txs,
  Map<String, Map<String, double>> closes = const {},
  Map<String, List<DateTime>> splits = const {},
  Map<String, FxHistory> fx = const {},
  required DateTime today,
}) => computeProfitability(
  assets: assets ?? [_asset(1, 'PETR4')],
  transactions: txs,
  monthCloses: closes,
  splits: splits,
  fxToBrl: fx,
  today: today,
);

Matcher _error(ProfitabilityErrorCode code, [Map<String, String> args = const {}]) => throwsA(
  isA<ProfitabilityError>().having((e) => e.code, 'code', code).having((e) => e.args, 'args', args),
);

void main() {
  const w21of31 = 21 / 31; // compra no dia 10 de janeiro
  final jan10 = DateTime(2026, 1, 10);

  group('retorno do mês', () {
    test('1: compra de 10 a 30 no dia 10 e fechamento a 32 rende 20 ÷ (300 × 21/31) = +9,84%', () {
      final r = _run(
        txs: [_buy(1, jan10, 10, 300)],
        closes: {'PETR4': {'2026-01': 32.0}},
        today: DateTime(2026, 1, 31),
      );
      expect(r.months, hasLength(1));
      final jan = r.months.single;
      expect(jan.yearMonth, '2026-01');
      expect(jan.bmv, 0);
      expect(jan.emv, 320);
      expect(jan.cfTotal, 300);
      expect(jan.rMonthPct, closeTo(20 / (300 * w21of31) * 100, 1e-9));
      expect(jan.rCumulativePct, closeTo(jan.rMonthPct, 1e-9));
    });

    test('2: fevereiro cai de 320 para 288 (−10%) e o acumulado encadeia: −1,1429%', () {
      final r = _run(
        txs: [_buy(1, jan10, 10, 300)],
        closes: {'PETR4': {'2026-01': 32.0, '2026-02': 28.8}},
        today: DateTime(2026, 2, 28),
      );
      final feb = r.months[1];
      expect(feb.bmv, 320);
      expect(feb.emv, closeTo(288, 1e-9));
      expect(feb.rMonthPct, closeTo(-10, 1e-9));
      expect(feb.rCumulativePct, closeTo(((1 + 20 / (300 * w21of31)) * 0.9 - 1) * 100, 1e-9));
    });

    test('3: a taxa de corretagem de 3 na compra entra como aporte maior (303)', () {
      final r = _run(
        txs: [_buy(1, jan10, 10, 300, fee: 3)],
        closes: {'PETR4': {'2026-01': 32.0}},
        today: DateTime(2026, 1, 31),
      );
      expect(r.months.single.cfTotal, 303);
      expect(r.months.single.rMonthPct, closeTo(17 / (303 * w21of31) * 100, 1e-9));
    });

    test('4: venda de 5 a 34 (total 170) no dia 20 de fevereiro sai como fluxo negativo', () {
      final r = _run(
        txs: [_buy(1, jan10, 10, 300), _sell(1, DateTime(2026, 2, 20), 5, 170)],
        closes: {'PETR4': {'2026-01': 32.0, '2026-02': 36.0}},
        today: DateTime(2026, 2, 28),
      );
      final feb = r.months[1];
      expect(feb.emv, 180);
      expect(feb.cfTotal, -170);
      expect(feb.rMonthPct, closeTo((180 - 320 + 170) / (320 - 170 * 8 / 28) * 100, 1e-9));
    });

    test('5: provento de 12 no dia 15 de fevereiro eleva o retorno (carteira parada em 320)', () {
      final r = _run(
        txs: [_buy(1, jan10, 10, 300), _tx(1, TransactionType.provento, DateTime(2026, 2, 15), total: 12)],
        closes: {'PETR4': {'2026-01': 32.0, '2026-02': 32.0}},
        today: DateTime(2026, 2, 28),
      );
      final feb = r.months[1];
      expect(feb.cfTotal, -12);
      expect(feb.rMonthPct, closeTo(12 / (320 - 12 * 13 / 28) * 100, 1e-9));
      expect(feb.rMonthPct, greaterThan(0));
    });

    test('6: vendeu tudo em fevereiro: março rende 0%, sem preço e sem erro', () {
      final r = _run(
        txs: [_buy(1, jan10, 10, 300), _sell(1, DateTime(2026, 2, 10), 10, 330)],
        closes: {'PETR4': {'2026-01': 32.0, '2026-02': 33.0}},
        today: DateTime(2026, 3, 15),
      );
      expect(r.months.map((m) => m.yearMonth), ['2026-01', '2026-02', '2026-03']);
      final mar = r.months[2];
      expect((mar.bmv, mar.emv, mar.rMonthPct), (0, 0, 0));
      expect(mar.rCumulativePct, closeTo(r.months[1].rCumulativePct, 1e-9));
    });

    test('7: compra no último dia sem saldo anterior (denominador zero) rende 0% e não quebra', () {
      final r = _run(
        txs: [_buy(1, DateTime(2026, 1, 31), 10, 300)],
        closes: {'PETR4': {'2026-01': 32.0}},
        today: DateTime(2026, 1, 31),
      );
      expect(r.months.single.rMonthPct, 0);
      expect(r.months.single.emv, 320);
    });

    test('16: dois ativos somam valor e fluxos no mesmo mês', () {
      final r = _run(
        assets: [_asset(1, 'PETR4'), _asset(2, 'VALE3')],
        txs: [_buy(1, jan10, 10, 300), _buy(2, jan10, 10, 200)],
        closes: {'PETR4': {'2026-01': 32.0}, 'VALE3': {'2026-01': 19.0}},
        today: DateTime(2026, 1, 31),
      );
      expect(r.months.single.emv, 510);
      expect(r.months.single.rMonthPct, closeTo(10 / (500 * w21of31) * 100, 1e-9));
    });

    test('17: as transações fora de ordem dão o mesmo resultado do teste 2', () {
      final r = _run(
        txs: [_sell(1, DateTime(2026, 2, 20), 5, 170), _buy(1, jan10, 10, 300)],
        closes: {'PETR4': {'2026-01': 32.0, '2026-02': 36.0}},
        today: DateTime(2026, 2, 28),
      );
      expect(r.months[1].emv, 180);
    });
  });

  group('desdobramento', () {
    final sharedCloses = {'VALE3': {'2026-01': 21.0}, 'PETR4': {'2026-01': 32.0}};
    final bothAssets = [_asset(1, 'PETR4'), _asset(2, 'VALE3')];

    test('8: split depois da 1ª compra tira o ativo do cálculo e o lista como excluído', () {
      final r = _run(
        assets: bothAssets,
        txs: [_buy(1, jan10, 10, 300), _buy(2, jan10, 10, 200)],
        closes: sharedCloses,
        splits: {'PETR4': [DateTime(2026, 1, 20)]},
        today: DateTime(2026, 1, 31),
      );
      expect(r.excluded.map((e) => e.asset.ticker), ['PETR4']);
      expect(r.excluded.single.splitDate, DateTime(2026, 1, 20));
      // Só a VALE3 conta: (210 − 200) ÷ (200 × 21/31).
      expect(r.months.single.emv, 210);
      expect(r.months.single.rMonthPct, closeTo(10 / (200 * w21of31) * 100, 1e-9));
    });

    test('9: split antes da 1ª compra não exclui', () {
      final r = _run(
        txs: [_buy(1, jan10, 10, 300)],
        closes: sharedCloses,
        splits: {'PETR4': [DateTime(2026, 1, 5)]},
        today: DateTime(2026, 1, 31),
      );
      expect(r.excluded, isEmpty);
      expect(r.months.single.emv, 320);
    });

    test('10: split no mesmo dia da 1ª compra exclui (lado conservador)', () {
      final r = _run(
        txs: [_buy(1, jan10, 10, 300)],
        closes: sharedCloses,
        splits: {'PETR4': [jan10]},
        today: DateTime(2026, 1, 31),
      );
      expect(r.excluded, hasLength(1));
      expect(r.months, isEmpty);
    });
  });

  group('preço e câmbio', () {
    test('11: preço do mês faltando para ativo em carteira é recusado com erro nomeado', () {
      expect(
        () => _run(
          txs: [_buy(1, jan10, 10, 300)],
          closes: {'PETR4': {'2026-01': 32.0}},
          today: DateTime(2026, 2, 28),
        ),
        _error(ProfitabilityErrorCode.missingMonthPrice, {'ticker': 'PETR4', 'month': '2026-02'}),
      );
    });

    final usd = _asset(1, 'AAPL', assetClass: AssetClass.acaoInternacional, currency: 'USD');
    final fx = {
      'USD': FxHistory([
        (DateTime.utc(2026, 1, 10), 5.0),
        (DateTime.utc(2026, 1, 31), 5.5),
        (DateTime.utc(2026, 2, 10), 6.0),
        (DateTime.utc(2026, 2, 28), 9.0),
      ]),
    };

    test('12: ação em dólar: fluxo e valor final convertidos pela taxa da própria data', () {
      // Compra US$ 1.000 a 5,00 = R$ 5.000; fim de janeiro 10 × US$ 110 × 5,50 = R$ 6.050.
      final r = _run(
        assets: [usd],
        txs: [_buy(1, jan10, 10, 1000)],
        closes: {'AAPL': {'2026-01': 110.0}},
        fx: fx,
        today: DateTime(2026, 1, 31),
      );
      expect(r.months.single.cfTotal, closeTo(5000, 1e-9));
      expect(r.months.single.emv, closeTo(6050, 1e-9));
      expect(r.months.single.rMonthPct, closeTo(31, 1e-9));
    });

    test('13: sem série de câmbio da moeda o cálculo é recusado com erro nomeado', () {
      expect(
        () => _run(
          assets: [usd],
          txs: [_buy(1, jan10, 10, 1000)],
          closes: {'AAPL': {'2026-01': 110.0}},
          today: DateTime(2026, 1, 31),
        ),
        _error(ProfitabilityErrorCode.missingFx, {'currency': 'USD'}),
      );
    });

    test('14: o mês corrente usa a taxa de hoje (10/02), não a do fim do mês', () {
      final r = _run(
        assets: [usd],
        txs: [_buy(1, jan10, 10, 1000)],
        closes: {'AAPL': {'2026-01': 100.0, '2026-02': 100.0}},
        fx: fx,
        today: DateTime(2026, 2, 10),
      );
      expect(r.months, hasLength(2));
      expect(r.months[1].emv, closeTo(10 * 100 * 6.0, 1e-9));
    });
  });

  group('o que entra', () {
    test('15: cripto, tesouro, fundo e aporte sem ativo ficam fora; não mexem nos números', () {
      final r = _run(
        assets: [
          _asset(1, 'PETR4'),
          _asset(2, 'BTC', assetClass: AssetClass.cripto),
          _asset(3, 'LFT', assetClass: AssetClass.tesouroDireto),
          _asset(4, 'FIDX', assetClass: AssetClass.fundo),
        ],
        txs: [
          _buy(1, jan10, 10, 300),
          _buy(2, jan10, 1, 90000),
          _buy(3, jan10, 1, 1000),
          _buy(4, jan10, 100, 500),
          _tx(null, TransactionType.aporte, jan10, total: 5000),
        ],
        closes: {'PETR4': {'2026-01': 32.0}},
        today: DateTime(2026, 1, 31),
      );
      expect(r.months.single.emv, 320);
      expect(r.months.single.cfTotal, 300);
    });

    test('18: sem nenhuma transação relevante: lista vazia, sem erro', () {
      final r = _run(txs: [_tx(null, TransactionType.aporte, jan10, total: 100)], today: DateTime(2026, 3, 1));
      expect(r.months, isEmpty);
      expect(r.excluded, isEmpty);
    });
  });
}

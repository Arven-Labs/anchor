import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/profitability.dart';
import 'package:anchor_mobile/domain/profitability_error.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/screens/profitability_screen.dart';
import 'package:anchor_mobile/services/profitability_repository.dart';

import 'support/localized_app.dart';

// A tela de rentabilidade montada de verdade, com cálculo e atualização injetados (sem banco
// nem rede). Tabela de testes B15 a B19.
MonthlyReturn _month(String ym, double bmv, double emv, double cf, double r, double cum) =>
    MonthlyReturn(yearMonth: ym, bmv: bmv, emv: emv, cfTotal: cf, rMonthPct: r, rCumulativePct: cum);

final _twoMonths = ProfitabilityResult(
  months: [
    _month('2026-01', 0, 320, 300, 9.8412698, 9.8412698),
    _month('2026-02', 320, 288, 0, -10.0, -1.1428571),
  ],
  excluded: const [],
);

Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 20; i++) {
    await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
    await tester.pump();
  }
}

Future<void> _pump(
  WidgetTester tester, {
  required Future<ProfitabilityResult> Function() compute,
  Future<PriceRefreshSummary> Function()? refresh,
  String locale = 'pt',
}) async {
  tester.view.physicalSize = const Size(800, 3000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    localizedApp(
      ProfitabilityScreen(
        compute: compute,
        refresh: refresh ?? () async => const PriceRefreshSummary(updated: [], failed: []),
      ),
      locale: locale,
    ),
  );
  await _settle(tester);
}

void main() {
  testWidgets('B15: mostra "antes de impostos", acumulado, 12M sem dado e as linhas com vírgula', (tester) async {
    await _pump(tester, compute: () async => _twoMonths);

    expect(find.text('Retorno antes de impostos'), findsOneWidget);
    expect(find.text('Acumulado: -1,14%'), findsOneWidget);
    expect(find.text('Últimos 12 meses: —'), findsOneWidget);
    expect(find.text('2026-01'), findsOneWidget);
    expect(find.text('+9,84%'), findsOneWidget);
    expect(find.text('Valor final: R\$ 320,00'), findsOneWidget);
    expect(find.text('2026-02'), findsOneWidget);
    expect(find.text('-10,00%'), findsOneWidget);
  });

  testWidgets('B15: em inglês, ponto decimal', (tester) async {
    await _pump(tester, compute: () async => _twoMonths, locale: 'en');

    expect(find.text('Return before taxes'), findsOneWidget);
    expect(find.text('Cumulative: -1.14%'), findsOneWidget);
    expect(find.text('+9.84%'), findsOneWidget);
  });

  testWidgets('B16: preço faltando mostra ticker e mês e o botão de atualizar', (tester) async {
    await _pump(
      tester,
      compute: () async => throw ProfitabilityError(ProfitabilityErrorCode.missingMonthPrice, {
        'ticker': 'PETR4',
        'month': '2026-02',
      }),
    );

    expect(find.text('Falta o preço de PETR4 em 2026-02. Atualize o histórico de preços.'), findsOneWidget);
    expect(find.byTooltip('Atualizar histórico de preços'), findsOneWidget);
  });

  testWidgets('B17: atualizar busca, recalcula e mostra quantos ativos foram atualizados', (tester) async {
    var computeCalls = 0;
    var refreshCalls = 0;
    await _pump(
      tester,
      compute: () async {
        computeCalls++;
        return _twoMonths;
      },
      refresh: () async {
        refreshCalls++;
        return const PriceRefreshSummary(updated: ['PETR4'], failed: []);
      },
    );
    expect(computeCalls, 1);

    await tester.tap(find.byTooltip('Atualizar histórico de preços'));
    await _settle(tester);

    expect(refreshCalls, 1);
    expect(computeCalls, 2);
    expect(find.text('1 ativo atualizado'), findsOneWidget);
  });

  testWidgets('B17: ativo que falhou na atualização aparece no resumo', (tester) async {
    await _pump(
      tester,
      compute: () async => _twoMonths,
      refresh: () async => const PriceRefreshSummary(updated: ['PETR4'], failed: ['VALE3']),
    );

    await tester.tap(find.byTooltip('Atualizar histórico de preços'));
    await _settle(tester);

    expect(find.text('Falharam: VALE3'), findsOneWidget);
  });

  testWidgets('B18: ativo com desdobramento aparece como fora do cálculo, com a data', (tester) async {
    final asset = Asset(
      id: 1,
      ticker: 'PETR4',
      name: 'Petrobras',
      assetClass: AssetClass.acaoBr,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 1),
    );
    await _pump(
      tester,
      compute: () async => ProfitabilityResult(
        months: _twoMonths.months,
        excluded: [ExcludedAsset(asset: asset, splitDate: DateTime.utc(2026, 2, 15))],
      ),
    );

    expect(find.text('Fora do cálculo (desdobramento)'), findsOneWidget);
    expect(find.text('PETR4: desdobramento em 2026-02-15'), findsOneWidget);
  });

  testWidgets('B19: sem transação medida mostra a mensagem de vazio', (tester) async {
    await _pump(tester, compute: () async => const ProfitabilityResult(months: [], excluded: []));

    expect(
      find.text('Nenhuma compra, venda ou provento de ação, FII, ETF, BDR ou metal para medir.'),
      findsOneWidget,
    );
  });
}

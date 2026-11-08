import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/profitability_error.dart';
import 'package:anchor_mobile/domain/twr.dart';

// Porta de `desktop/src-tauri/src/domain/twr.rs`: os números esperados são os do desktop.
void main() {
  group('modifiedDietz', () {
    test('janeiro: carteira de 10.000 vira 12.500 com aporte de 2.000 no dia 10 (peso 21/31)', () {
      final r = modifiedDietz(10000, 12500, [const CashFlow(amount: 2000, weight: 21 / 31)]);
      expect(r, closeTo(0.04403409090909091, 1e-9));
    });

    test('fevereiro sem fluxo: 12.500 viram 12.250, queda de 2%', () {
      expect(modifiedDietz(12500, 12250, const []), closeTo(-0.02, 1e-9));
    });

    test('carteira parada em 100 que recebeu 5 de provento no fim do mês rende +5%, não −5%', () {
      final r = modifiedDietz(100, 100, [CashFlow(amount: flowAmount(FlowKind.income, 5), weight: 0)]);
      expect(r, closeTo(0.05, 1e-9));
    });

    test('BMV 0, EMV 0 e sem fluxo: denominador zero é recusado com erro nomeado', () {
      expect(
        () => modifiedDietz(0, 0, const []),
        throwsA(isA<ProfitabilityError>().having((e) => e.code, 'code', ProfitabilityErrorCode.zeroDenominator)),
      );
    });
  });

  group('flowAmount', () {
    test('compra entra (+), venda sai (−), provento sai (−) da carteira medida', () {
      expect(flowAmount(FlowKind.contribution, 100), 100);
      expect(flowAmount(FlowKind.withdrawal, 100), -100);
      expect(flowAmount(FlowKind.income, 100), -100);
    });
  });

  group('chain', () {
    test('janeiro +4,403% e fevereiro −2% encadeados dão +2,3153% (produto, não soma)', () {
      expect(chain([0.04403409090909091, -0.02]), closeTo(0.023153409090908905, 1e-9));
    });

    test('sem meses o acumulado é zero', () {
      expect(chain(const []), 0);
    });
  });

  group('trailingTwelveMonthPct', () {
    test('13 pontos acumulados: de 10% a 21% é um retorno de 10% nos últimos 12 meses', () {
      final series = [10.0, ...List.filled(11, 15.0), 21.0];
      expect(trailingTwelveMonthPct(series), closeTo(10.0, 1e-9));
    });

    test('menos de 13 pontos devolve null em vez de um número mal rotulado', () {
      expect(trailingTwelveMonthPct(List.filled(12, 5.0)), isNull);
      expect(trailingTwelveMonthPct(const []), isNull);
    });
  });
}

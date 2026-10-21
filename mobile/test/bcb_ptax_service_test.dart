import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/domain/ptax_series.dart';
import 'package:anchor_mobile/services/bcb_ptax_service.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';

void main() {
  // Respostas reais da Olinda (amostradas em 2026-10-21).
  const dollarBody = '''
{"@odata.context":"x","value":[
 {"cotacaoCompra":6.19170,"cotacaoVenda":6.19230,"dataHoraCotacao":"2024-12-30 13:06:26.618"},
 {"cotacaoCompra":6.19170,"cotacaoVenda":6.19230,"dataHoraCotacao":"2024-12-31 10:04:09.264"},
 {"cotacaoCompra":6.20800,"cotacaoVenda":6.20860,"dataHoraCotacao":"2025-01-02 13:09:42.489"},
 {"cotacaoCompra":6.15570,"cotacaoVenda":6.15630,"dataHoraCotacao":"2025-01-03 13:07:30.503"}]}''';

  const euroBody = '''
{"@odata.context":"x","value":[
 {"paridadeCompra":1.0318,"paridadeVenda":1.032,"cotacaoCompra":6.42320,"cotacaoVenda":6.42500,"dataHoraCotacao":"2025-01-02 10:10:25.056","tipoBoletim":"Abertura"},
 {"paridadeCompra":1.0274,"paridadeVenda":1.0275,"cotacaoCompra":6.35980,"cotacaoVenda":6.36100,"dataHoraCotacao":"2025-01-02 13:09:42.485","tipoBoletim":"Intermediário"},
 {"paridadeCompra":1.0274,"paridadeVenda":1.0275,"cotacaoCompra":6.37810,"cotacaoVenda":6.37930,"dataHoraCotacao":"2025-01-02 13:09:42.489","tipoBoletim":"Fechamento"},
 {"paridadeCompra":1.0296,"paridadeVenda":1.0297,"cotacaoCompra":6.34420,"cotacaoVenda":6.34540,"dataHoraCotacao":"2025-01-03 10:11:26.859","tipoBoletim":"Abertura"}]}''';

  BcbPtaxService serviceReturning(int status, [String body = '']) =>
      BcbPtaxService(
        client: MockClient((_) async => http.Response(body, status)),
      );

  group('parsePtax', () {
    test('dólar: uma cotação de fechamento por dia útil', () {
      final series = BcbPtaxService.parsePtax(dollarBody, dollar: true);

      expect(series.rate(DateTime(2025, 1, 2), RateKind.sell), 6.2086);
      expect(series.rate(DateTime(2025, 1, 2), RateKind.buy), 6.208);
      expect(series.rate(DateTime(2025, 1, 1), RateKind.sell), 6.1923,
          reason: 'feriado: vale a última anterior (31/12)');
    });

    test('dólar: de duas linhas no mesmo dia vale a mais tardia', () {
      const body = '''
{"value":[
 {"cotacaoCompra":5.0,"cotacaoVenda":5.1,"dataHoraCotacao":"2025-01-02 10:00:00.000"},
 {"cotacaoCompra":5.2,"cotacaoVenda":5.3,"dataHoraCotacao":"2025-01-02 13:00:00.000"}]}''';

      final series = BcbPtaxService.parsePtax(body, dollar: true);

      expect(series.rate(DateTime(2025, 1, 2), RateKind.sell), 5.3);
    });

    test('outras moedas: só o boletim de fechamento vale', () {
      final series = BcbPtaxService.parsePtax(euroBody, dollar: false);

      expect(series.rate(DateTime(2025, 1, 2), RateKind.sell), 6.3793);
      expect(series.rate(DateTime(2025, 1, 2), RateKind.buy), 6.3781);
      // 03/01 só tem a abertura (resposta cortada): vale a de 02/01
      expect(series.rate(DateTime(2025, 1, 3), RateKind.sell), 6.3793);
    });

    test('cotação não positiva é descartada', () {
      const body =
          '{"value":[{"cotacaoCompra":0.0,"cotacaoVenda":0.0,'
          '"dataHoraCotacao":"2025-01-02 13:00:00.000"}]}';

      expect(BcbPtaxService.parsePtax(body, dollar: true).isEmpty, isTrue);
    });

    test('HTML com status 200 é fonte indisponível, não série vazia', () {
      expect(
        () => BcbPtaxService.parsePtax('<html>erro</html>', dollar: true),
        throwsA(isA<QuoteUnavailableException>()),
      );
    });
  });

  group('fetchPtax', () {
    test('dólar pede o endpoint do dólar com datas mês-dia-ano', () async {
      late Uri url;
      final service = BcbPtaxService(
        client: MockClient((request) async {
          url = request.url;
          return http.Response(dollarBody, 200);
        }),
      );

      final series = await service.fetchPtax(
        currency: 'USD',
        from: DateTime(2024, 12, 30),
      );

      expect(url.toString(), contains('CotacaoDolarPeriodo'));
      expect(url.toString(), contains("@dataInicial='12-30-2024'"));
      expect(url.toString(), contains(r'$format=json'));
      expect(series.rate(DateTime(2025, 1, 2), RateKind.sell), 6.2086);
    });

    test('euro pede o endpoint de moedas e aceita minúscula com espaço', () async {
      late Uri url;
      final service = BcbPtaxService(
        client: MockClient((request) async {
          url = request.url;
          return http.Response(euroBody, 200);
        }),
      );

      final series = await service.fetchPtax(
        currency: ' eur ',
        from: DateTime(2025, 1, 2),
      );

      expect(url.toString(), contains('CotacaoMoedaPeriodo'));
      expect(url.toString(), contains("@moeda='EUR'"));
      expect(series.rate(DateTime(2025, 1, 2), RateKind.sell), 6.3793);
    });

    test('moeda que a PTAX não cota é não encontrada sem ir à rede', () async {
      var called = false;
      final service = BcbPtaxService(
        client: MockClient((_) async {
          called = true;
          return http.Response('', 200);
        }),
      );

      await expectLater(
        service.fetchPtax(currency: 'CNY', from: DateTime(2025, 1, 2)),
        throwsA(isA<QuoteNotFoundException>()),
      );
      expect(called, isFalse);
      expect(BcbPtaxService.isSupported('eur'), isTrue);
      expect(BcbPtaxService.isSupported('BRL'), isFalse);
    });

    test('429 e 5xx são indisponível; 404 e série vazia são não encontrado', () {
      Future<void> fetch(BcbPtaxService s) =>
          s.fetchPtax(currency: 'USD', from: DateTime(2025, 1, 2));

      expect(fetch(serviceReturning(429)), throwsA(isA<QuoteUnavailableException>()));
      expect(fetch(serviceReturning(503)), throwsA(isA<QuoteUnavailableException>()));
      expect(fetch(serviceReturning(404)), throwsA(isA<QuoteNotFoundException>()));
      expect(
        fetch(serviceReturning(200, '{"value":[]}')),
        throwsA(isA<QuoteNotFoundException>()),
      );
    });

    test('timeout e rede fora são indisponível', () {
      final timeout = BcbPtaxService(
        client: MockClient((_) async => throw TimeoutException('lento')),
      );
      final offline = BcbPtaxService(
        client: MockClient((_) async => throw const SocketException('sem rede')),
      );

      Future<void> fetch(BcbPtaxService s) =>
          s.fetchPtax(currency: 'USD', from: DateTime(2025, 1, 2));

      expect(fetch(timeout), throwsA(isA<QuoteUnavailableException>()));
      expect(fetch(offline), throwsA(isA<QuoteUnavailableException>()));
    });
  });
}

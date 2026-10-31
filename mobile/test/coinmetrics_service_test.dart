import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/coinmetrics_service.dart';
import 'package:anchor_mobile/services/crypto_source_http.dart';

// Corpos reais de `/v4/timeseries/asset-metrics` do ETH (2026-10-31): `data` com
// uma linha por dia, `time` em ISO com nanossegundos e todo número como texto.
// Os de endereços ativos e ETH/BTC são as 31 últimas linhas, tal como o serviço
// pede. Supply e netflow reais têm ~6 e ~10 KB e as linhas do dia vêm com
// `<métrica>-status: "flash"`; esses casos usam séries inventadas (abaixo), com
// o mesmo formato das linhas flash.
const _ethAddresses =
    r'{"data":[{"asset":"eth","time":"2026-09-30T00:00:00.000000000Z","AdrActCnt":"908889"},{"asset":"eth","time":"2026-10-01T00:00:00.000000000Z","AdrActCnt":"834251"},{"asset":"eth","time":"2026-10-02T00:00:00.000000000Z","AdrActCnt":"835088"},{"asset":"eth","time":"2026-10-03T00:00:00.000000000Z","AdrActCnt":"788281"},{"asset":"eth","time":"2026-10-04T00:00:00.000000000Z","AdrActCnt":"654127"},{"asset":"eth","time":"2026-10-05T00:00:00.000000000Z","AdrActCnt":"647739"},{"asset":"eth","time":"2026-10-06T00:00:00.000000000Z","AdrActCnt":"732508"},{"asset":"eth","time":"2026-10-07T00:00:00.000000000Z","AdrActCnt":"755689"},{"asset":"eth","time":"2026-10-08T00:00:00.000000000Z","AdrActCnt":"707006"},{"asset":"eth","time":"2026-10-09T00:00:00.000000000Z","AdrActCnt":"727142"},{"asset":"eth","time":"2026-10-10T00:00:00.000000000Z","AdrActCnt":"972309"},{"asset":"eth","time":"2026-10-11T00:00:00.000000000Z","AdrActCnt":"839464"},{"asset":"eth","time":"2026-10-12T00:00:00.000000000Z","AdrActCnt":"766375"},{"asset":"eth","time":"2026-10-13T00:00:00.000000000Z","AdrActCnt":"793700"},{"asset":"eth","time":"2026-10-14T00:00:00.000000000Z","AdrActCnt":"774585"},{"asset":"eth","time":"2026-10-15T00:00:00.000000000Z","AdrActCnt":"728856"},{"asset":"eth","time":"2026-10-16T00:00:00.000000000Z","AdrActCnt":"821046"},{"asset":"eth","time":"2026-10-17T00:00:00.000000000Z","AdrActCnt":"879181"},{"asset":"eth","time":"2026-10-18T00:00:00.000000000Z","AdrActCnt":"912816"},{"asset":"eth","time":"2026-10-19T00:00:00.000000000Z","AdrActCnt":"759502"},{"asset":"eth","time":"2026-10-20T00:00:00.000000000Z","AdrActCnt":"782317"},{"asset":"eth","time":"2026-10-21T00:00:00.000000000Z","AdrActCnt":"813190"},{"asset":"eth","time":"2026-10-22T00:00:00.000000000Z","AdrActCnt":"753365"},{"asset":"eth","time":"2026-10-23T00:00:00.000000000Z","AdrActCnt":"890889"},{"asset":"eth","time":"2026-10-24T00:00:00.000000000Z","AdrActCnt":"896486"},{"asset":"eth","time":"2026-10-25T00:00:00.000000000Z","AdrActCnt":"881585"},{"asset":"eth","time":"2026-10-26T00:00:00.000000000Z","AdrActCnt":"751785"},{"asset":"eth","time":"2026-10-27T00:00:00.000000000Z","AdrActCnt":"831925"},{"asset":"eth","time":"2026-10-28T00:00:00.000000000Z","AdrActCnt":"933642"},{"asset":"eth","time":"2026-10-29T00:00:00.000000000Z","AdrActCnt":"745978"},{"asset":"eth","time":"2026-10-30T00:00:00.000000000Z","AdrActCnt":"677054"}]}';
const _ethPriceBtc =
    r'{"data":[{"asset":"eth","time":"2026-09-30T00:00:00.000000000Z","PriceBTC":"0.03213775921282206"},{"asset":"eth","time":"2026-10-01T00:00:00.000000000Z","PriceBTC":"0.03189067083930242"},{"asset":"eth","time":"2026-10-02T00:00:00.000000000Z","PriceBTC":"0.03156757322151605"},{"asset":"eth","time":"2026-10-03T00:00:00.000000000Z","PriceBTC":"0.03170453136738679"},{"asset":"eth","time":"2026-10-04T00:00:00.000000000Z","PriceBTC":"0.031532023296238956"},{"asset":"eth","time":"2026-10-05T00:00:00.000000000Z","PriceBTC":"0.03160740872788796"},{"asset":"eth","time":"2026-10-06T00:00:00.000000000Z","PriceBTC":"0.031542434016940867"},{"asset":"eth","time":"2026-10-07T00:00:00.000000000Z","PriceBTC":"0.030899721373671436"},{"asset":"eth","time":"2026-10-08T00:00:00.000000000Z","PriceBTC":"0.030270865397781583"},{"asset":"eth","time":"2026-10-09T00:00:00.000000000Z","PriceBTC":"0.030113370396223863"},{"asset":"eth","time":"2026-10-10T00:00:00.000000000Z","PriceBTC":"0.0301980534751811"},{"asset":"eth","time":"2026-10-11T00:00:00.000000000Z","PriceBTC":"0.030293528909719116"},{"asset":"eth","time":"2026-10-12T00:00:00.000000000Z","PriceBTC":"0.029942039275368527"},{"asset":"eth","time":"2026-10-13T00:00:00.000000000Z","PriceBTC":"0.030037320767780453"},{"asset":"eth","time":"2026-10-14T00:00:00.000000000Z","PriceBTC":"0.02965350506673841"},{"asset":"eth","time":"2026-10-15T00:00:00.000000000Z","PriceBTC":"0.029704965765346415"},{"asset":"eth","time":"2026-10-16T00:00:00.000000000Z","PriceBTC":"0.02968039519785179"},{"asset":"eth","time":"2026-10-17T00:00:00.000000000Z","PriceBTC":"0.02946930041712068"},{"asset":"eth","time":"2026-10-18T00:00:00.000000000Z","PriceBTC":"0.029330459731608832"},{"asset":"eth","time":"2026-10-19T00:00:00.000000000Z","PriceBTC":"0.029367217863030367"},{"asset":"eth","time":"2026-10-20T00:00:00.000000000Z","PriceBTC":"0.02950939663985285"},{"asset":"eth","time":"2026-10-21T00:00:00.000000000Z","PriceBTC":"0.029385795986138066"},{"asset":"eth","time":"2026-10-22T00:00:00.000000000Z","PriceBTC":"0.029322334160244316"},{"asset":"eth","time":"2026-10-23T00:00:00.000000000Z","PriceBTC":"0.029388747440576932"},{"asset":"eth","time":"2026-10-24T00:00:00.000000000Z","PriceBTC":"0.02914504550628711"},{"asset":"eth","time":"2026-10-25T00:00:00.000000000Z","PriceBTC":"0.02905832574539534"},{"asset":"eth","time":"2026-10-26T00:00:00.000000000Z","PriceBTC":"0.028975616358177612"},{"asset":"eth","time":"2026-10-27T00:00:00.000000000Z","PriceBTC":"0.029053794709038575"},{"asset":"eth","time":"2026-10-28T00:00:00.000000000Z","PriceBTC":"0.029013575170587634"},{"asset":"eth","time":"2026-10-29T00:00:00.000000000Z","PriceBTC":"0.028851939320297684"},{"asset":"eth","time":"2026-10-30T00:00:00.000000000Z","PriceBTC":"0.029052546621210748"}]}';

/// Uma linha real (`page_size=1`): emissão bruta diária em ETH e oferta circulante.
const _ethIssuance =
    r'{"data":[{"asset":"eth","time":"2026-10-30T00:00:00.000000000Z","IssTotNtv":"2992.561249519","SplyCur":"122158026.107449378823202721"}]}';

/// Dia seguinte à última linha dos corpos reais.
final _today = DateTime.utc(2026, 10, 31);

/// `count` inteiros consecutivos a partir de [from], como texto (a API manda
/// todo número como string).
List<Object?> _range(int from, int count) => [
  for (var i = 0; i < count; i++) '${from + i}',
];

List<Object?> _same(Object? value, int count) => List.filled(count, value);

/// Corpo no formato real: uma linha por dia terminando em [last], com cada
/// coluna de [columns] (do dia mais antigo ao mais novo). [flash] marca a
/// última linha como provisória nessas métricas, como faz a API.
String _body(
  Map<String, List<Object?>> columns, {
  String last = '2026-10-30',
  Set<String> flash = const {},
}) {
  final count = columns.values.first.length;
  final lastDay = DateTime.parse('${last}T00:00:00Z');
  final rows = <Map<String, Object?>>[];
  for (var i = 0; i < count; i++) {
    final day = lastDay.subtract(Duration(days: count - 1 - i));
    rows.add({
      'asset': 'eth',
      'time': '${day.toIso8601String().substring(0, 10)}T00:00:00.000000000Z',
      for (final e in columns.entries) e.key: e.value[i],
      if (i == count - 1)
        for (final metric in flash) ...{
          '$metric-status': 'flash',
          '$metric-status-time': '2026-10-31T02:28:21.346263000Z',
        },
    });
  }
  return jsonEncode({'data': rows});
}

typedef _Parse = double Function(String body, {required DateTime today});

/// Um parser, com um corpo válido terminando em [last]. [bad] troca o valor da
/// primeira métrica na última linha (em registro, para poder trocar por `null`).
typedef _Bad = ({Object? value})?;
typedef _Case = ({_Parse parse, String Function(String last, {_Bad bad}) body});

List<Object?> _withLast(List<Object?> values, _Bad bad) =>
    bad == null ? values : [...values.take(values.length - 1), bad.value];

final Map<String, _Case> _cases = {
  'mvrv_z_score': (
    parse: parseMvrvZScore,
    body: (last, {bad}) => _body({
      'CapMrktCurUSD': _withLast(_range(10, 4), bad),
      'CapMVRVCur': _same('2', 4),
    }, last: last),
  ),
  'puell_multiple': (
    parse: parsePuellMultiple,
    body: (last, {bad}) =>
        _body({'IssTotUSD': _withLast(_range(1, 365), bad)}, last: last),
  ),
  'mayer_multiple': (
    parse: parseMayerMultiple,
    body: (last, {bad}) =>
        _body({'PriceUSD': _withLast(_range(1, 200), bad)}, last: last),
  ),
  'active_addresses_trend': (
    parse: parseActiveAddressesTrend,
    body: (last, {bad}) =>
        _body({'AdrActCnt': _withLast(_range(100, 31), bad)}, last: last),
  ),
  'exchange_supply_change_30d': (
    parse: parseExchangeSupplyChange30d,
    body: (last, {bad}) =>
        _body({'SplyExNtv': _withLast(_range(100, 31), bad)}, last: last),
  ),
  'eth_btc_trend_30d': (
    parse: parseEthBtcTrend30d,
    body: (last, {bad}) =>
        _body({'PriceBTC': _withLast(_range(100, 31), bad)}, last: last),
  ),
  'exchange_netflow': (
    parse: parseExchangeNetflow,
    body: (last, {bad}) => _body({
      'FlowInExUSD': _withLast(_same('3', 30), bad),
      'FlowOutExUSD': _same('1', 30),
    }, last: last),
  ),
  'gross_inflation_pct': (
    parse: parseGrossInflationPct,
    body: (last, {bad}) => _body({
      'IssTotNtv': _withLast(['3000'], bad),
      'SplyCur': ['120000000'],
    }, last: last),
  ),
};

Matcher _sourceError() => throwsA(
  isA<CryptoSourceException>().having(
    (e) => e.toString(),
    'mensagem',
    startsWith('CoinMetrics'),
  ),
);

void main() {
  group('tendência de 30 dias (endereços ativos, oferta nas exchanges, ETH/BTC)',
      () {
    test('endereços ativos reais do ETH: o último contra o de 30 posições atrás',
        () {
      // (677054 − 908889) ÷ 908889 × 100, calculado à parte.
      expect(
        parseActiveAddressesTrend(_ethAddresses, today: _today),
        closeTo(-25.507515219130166, 1e-9),
      );
    });

    test('ETH/BTC real: variação do preço em BTC', () {
      expect(
        parseEthBtcTrend30d(_ethPriceBtc, today: _today),
        closeTo(-9.59996174960574, 1e-9),
      );
    });

    test('oferta nas exchanges: ignora as colunas -status da linha flash', () {
      final body = _body(
        {'SplyExNtv': _range(100, 31)},
        flash: {'SplyExNtv'},
      );

      expect(
        parseExchangeSupplyChange30d(body, today: _today),
        closeTo(30.0, 1e-9),
      );
    });

    test('a base é o ponto de 30 posições atrás, não o de 29 nem o de 31', () {
      // 32 pontos de 100 a 130 e um último de 200: o de 30 atrás é o 101.
      final body = _body({'AdrActCnt': [..._range(100, 31), '200']});

      expect(
        parseActiveAddressesTrend(body, today: _today),
        closeTo(98.01980198019803, 1e-9),
      );
    });

    test('31 linhas bastam; 30 não', () {
      expect(
        parseActiveAddressesTrend(
          _body({'AdrActCnt': _range(100, 31)}),
          today: _today,
        ),
        closeTo(30.0, 1e-9),
      );
      final byMetric = <String, _Parse>{
        'AdrActCnt': parseActiveAddressesTrend,
        'SplyExNtv': parseExchangeSupplyChange30d,
        'PriceBTC': parseEthBtcTrend30d,
      };
      for (final MapEntry(key: metric, value: parse) in byMetric.entries) {
        expect(
          () => parse(_body({metric: _range(100, 30)}), today: _today),
          _sourceError(),
          reason: metric,
        );
      }
    });

    test('base zero é recusada (não divide por zero)', () {
      final body = _body({
        'AdrActCnt': ['0', ..._same('5', 29), '10'],
      });

      expect(
        () => parseActiveAddressesTrend(body, today: _today),
        _sourceError(),
      );
    });
  });

  group('MVRV Z-Score', () {
    test('usa o desvio-padrão populacional (divide por n, não por n − 1)', () {
      // Valores de mercado 10, 20, 30, 40 e MVRV de hoje 2: o realizado de hoje é
      // 40 ÷ 2 = 20 e o desvio populacional é √125, então (40 − 20) ÷ √125.
      // Com n − 1 o resultado seria 1,5492.
      final body = _body({
        'CapMrktCurUSD': ['10', '20', '30', '40'],
        'CapMVRVCur': ['1.5', '1.5', '1.5', '2'],
      });

      expect(
        parseMvrvZScore(body, today: _today),
        closeTo(1.7888543819998317, 1e-9),
      );
    });

    test('só o MVRV de hoje define o realizado de hoje; os antigos não pesam',
        () {
      final body = _body({
        'CapMrktCurUSD': ['10', '20', '30', '40'],
        'CapMVRVCur': ['9', '9', '9', '2'],
      });

      expect(
        parseMvrvZScore(body, today: _today),
        closeTo(1.7888543819998317, 1e-9),
      );
    });

    test('MVRV zero ou negativo em qualquer linha é recusado', () {
      for (final mvrv in ['0', '-1.5']) {
        final body = _body({
          'CapMrktCurUSD': ['10', '20', '30', '40'],
          'CapMVRVCur': [mvrv, '2', '2', '2'],
        });
        expect(
          () => parseMvrvZScore(body, today: _today),
          _sourceError(),
          reason: mvrv,
        );
      }
    });

    test('desvio-padrão zero (série constante ou de uma linha) é recusado', () {
      for (final caps in [
        ['10', '10', '10'],
        ['10'],
      ]) {
        final body = _body({
          'CapMrktCurUSD': caps,
          'CapMVRVCur': _same('2', caps.length),
        });
        expect(
          () => parseMvrvZScore(body, today: _today),
          _sourceError(),
          reason: '$caps',
        );
      }
    });

    test('uma linha com métrica nula no meio da série é recusada', () {
      final body = _body({
        'CapMrktCurUSD': ['10', null, '30', '40'],
        'CapMVRVCur': _same('2', 4),
      });

      expect(() => parseMvrvZScore(body, today: _today), _sourceError());
    });
  });

  group('Puell Multiple', () {
    test('emissão de hoje ÷ média das últimas 365 linhas', () {
      // 1..365: hoje 365, média 183.
      expect(
        parsePuellMultiple(
          _body({'IssTotUSD': _range(1, 365)}),
          today: _today,
        ),
        closeTo(1.994535519125683, 1e-12),
      );
    });

    test('linhas anteriores à janela de 365 não entram na média', () {
      // 35 linhas enormes antes das 365 de 1..365: se entrassem, a média subiria.
      final body = _body({
        'IssTotUSD': [..._same('1000000', 35), ..._range(1, 365)],
      });

      expect(
        parsePuellMultiple(body, today: _today),
        closeTo(1.994535519125683, 1e-12),
      );
    });

    test('364 linhas não bastam', () {
      expect(
        () => parsePuellMultiple(
          _body({'IssTotUSD': _range(1, 364)}),
          today: _today,
        ),
        _sourceError(),
      );
    });

    test('média zero é recusada', () {
      expect(
        () => parsePuellMultiple(
          _body({'IssTotUSD': _same('0', 365)}),
          today: _today,
        ),
        _sourceError(),
      );
    });

    test('moeda sem emissão em USD (todas nulas, como o XRP) é recusada', () {
      expect(
        () => parsePuellMultiple(
          _body({'IssTotUSD': _same(null, 365)}),
          today: _today,
        ),
        _sourceError(),
      );
    });
  });

  group('Múltiplo de Mayer', () {
    test('preço de hoje ÷ média das últimas 200 linhas', () {
      // 1..200: hoje 200, média 100,5.
      expect(
        parseMayerMultiple(_body({'PriceUSD': _range(1, 200)}), today: _today),
        closeTo(1.9900497512437811, 1e-12),
      );
    });

    test('linhas anteriores à janela de 200 não entram na média', () {
      final body = _body({
        'PriceUSD': [..._same('1000000', 50), ..._range(1, 200)],
      });

      expect(parseMayerMultiple(body, today: _today),
          closeTo(1.9900497512437811, 1e-12));
    });

    test('199 linhas não bastam', () {
      expect(
        () => parseMayerMultiple(
          _body({'PriceUSD': _range(1, 199)}),
          today: _today,
        ),
        _sourceError(),
      );
    });

    test('média zero é recusada', () {
      expect(
        () => parseMayerMultiple(
          _body({'PriceUSD': _same('0', 200)}),
          today: _today,
        ),
        _sourceError(),
      );
    });
  });

  group('netflow das exchanges', () {
    test('entrada maior que saída dá positivo: (3 − 1) ÷ (3 + 1) = 0,5', () {
      final body = _body(
        {'FlowInExUSD': _same('3', 30), 'FlowOutExUSD': _same('1', 30)},
        flash: {'FlowInExUSD', 'FlowOutExUSD'},
      );

      expect(parseExchangeNetflow(body, today: _today), closeTo(0.5, 1e-12));
    });

    test('saída maior que entrada dá negativo (acumulação)', () {
      final body = _body({
        'FlowInExUSD': _same('1', 30),
        'FlowOutExUSD': _same('3', 30),
      });

      expect(parseExchangeNetflow(body, today: _today), closeTo(-0.5, 1e-12));
    });

    test('só as últimas 30 linhas entram na soma', () {
      // 10 linhas antigas só com entrada: se entrassem, o resultado passaria de 0,5.
      final body = _body({
        'FlowInExUSD': [..._same('1000', 10), ..._same('3', 30)],
        'FlowOutExUSD': [..._same('0', 10), ..._same('1', 30)],
      });

      expect(parseExchangeNetflow(body, today: _today), closeTo(0.5, 1e-12));
    });

    test('menos de 30 linhas não bastam', () {
      final body = _body({
        'FlowInExUSD': _same('3', 29),
        'FlowOutExUSD': _same('1', 29),
      });

      expect(() => parseExchangeNetflow(body, today: _today), _sourceError());
    });

    test('fluxo total zero é recusado; fluxo negativo também', () {
      for (final (inflow, outflow) in [('0', '0'), ('-3', '1')]) {
        final body = _body({
          'FlowInExUSD': _same(inflow, 30),
          'FlowOutExUSD': _same(outflow, 30),
        });
        expect(
          () => parseExchangeNetflow(body, today: _today),
          _sourceError(),
          reason: '$inflow/$outflow',
        );
      }
    });
  });

  group('inflação bruta anual (para o staking yield real)', () {
    test('linha real do ETH: emissão × 365 ÷ oferta × 100', () {
      // 2992,561249519 × 365 ÷ 122158026,107449378823 × 100, calculado à parte.
      expect(
        parseGrossInflationPct(_ethIssuance, today: _today),
        closeTo(0.8941572575130418, 1e-12),
      );
    });

    test('3000 ETH por dia sobre 120 M: 0,9125 % ao ano', () {
      final body = _body({
        'IssTotNtv': ['3000'],
        'SplyCur': ['120000000'],
      });

      expect(
        parseGrossInflationPct(body, today: _today),
        closeTo(0.9125, 1e-12),
      );
    });

    test('só a última linha conta', () {
      final body = _body({
        'IssTotNtv': ['1', '1', '3000'],
        'SplyCur': ['1', '1', '120000000'],
      });

      expect(
        parseGrossInflationPct(body, today: _today),
        closeTo(0.9125, 1e-12),
      );
    });

    test('emissão zero é uma inflação válida de 0 %', () {
      final body = _body({
        'IssTotNtv': ['0'],
        'SplyCur': ['120000000'],
      });

      expect(parseGrossInflationPct(body, today: _today), 0.0);
    });

    test('oferta zero ou nula é recusada (não divide por zero)', () {
      for (final supply in <Object?>['0', null]) {
        final body = _body({
          'IssTotNtv': ['3000'],
          'SplyCur': [supply],
        });
        expect(
          () => parseGrossInflationPct(body, today: _today),
          _sourceError(),
          reason: '$supply',
        );
      }
    });
  });

  group('regras de todos os parsers', () {
    test('os corpos válidos de cada caso dão um número finito', () {
      for (final MapEntry(:key, :value) in _cases.entries) {
        final result = value.parse(value.body('2026-10-30'), today: _today);
        expect(result.isFinite, isTrue, reason: key);
      }
    });

    test('dado velho: 7 dias de atraso passa, 8 não', () {
      for (final MapEntry(:key, :value) in _cases.entries) {
        expect(
          value.parse(value.body('2026-10-24'), today: _today).isFinite,
          isTrue,
          reason: '$key com 7 dias',
        );
        expect(
          () => value.parse(value.body('2026-10-23'), today: _today),
          _sourceError(),
          reason: '$key com 8 dias',
        );
      }
    });

    test('valor nulo, texto, NaN, infinito ou negativo na última linha é recusado',
        () {
      // BNB e XRP trazem métricas `null`; `double.tryParse` aceita 'NaN' e
      // 'Infinity', que o classify trataria como neutro sem avisar.
      for (final MapEntry(:key, :value) in _cases.entries) {
        for (final bad in <Object?>[null, 'abc', 'NaN', 'Infinity', '-5', '']) {
          expect(
            () => value.parse(
              value.body('2026-10-30', bad: (value: bad)),
              today: _today,
            ),
            _sourceError(),
            reason: '$key com ${bad ?? 'null'}',
          );
        }
      }
    });

    test('número que a API manda como número JSON também vale', () {
      final body = _body({
        'AdrActCnt': [for (var i = 0; i < 31; i++) 100 + i],
      });

      expect(
        parseActiveAddressesTrend(body, today: _today),
        closeTo(30.0, 1e-9),
      );
    });

    test('corpo fora do formato vira erro da fonte, não TypeError', () {
      final bodies = [
        'não é json',
        '{"error":"x"}',
        '[]',
        '{"data":{}}',
        '{"data":[]}',
        '{"data":[1,2]}',
        '{"data":[{"asset":"eth"}]}',
        // `time` ausente ou inválido na última linha.
        '{"data":[{"asset":"eth","AdrActCnt":"1"}]}',
        '{"data":[{"asset":"eth","time":"ontem","AdrActCnt":"1"}]}',
      ];
      for (final MapEntry(:key, :value) in _cases.entries) {
        for (final body in bodies) {
          expect(
            () => value.parse(body, today: _today),
            _sourceError(),
            reason: '$key com $body',
          );
        }
      }
    });
  });

  group('CoinMetricsService', () {
    // (método, métricas pedidas, linhas pedidas, corpo válido para esse pedido)
    final calls =
        <
          String,
          ({
            Future<double> Function(CoinMetricsService, String) call,
            String metrics,
            String pageSize,
            String body,
          })
        >{
          'mvrv_z_score': (
            call: (s, c) => s.mvrvZScore(c),
            metrics: 'CapMrktCurUSD,CapMVRVCur',
            pageSize: '10000',
            body: _cases['mvrv_z_score']!.body('2026-10-30'),
          ),
          'puell_multiple': (
            call: (s, c) => s.puellMultiple(c),
            metrics: 'IssTotUSD',
            pageSize: '365',
            body: _cases['puell_multiple']!.body('2026-10-30'),
          ),
          'mayer_multiple': (
            call: (s, c) => s.mayerMultiple(c),
            metrics: 'PriceUSD',
            pageSize: '200',
            body: _cases['mayer_multiple']!.body('2026-10-30'),
          ),
          'active_addresses_trend': (
            call: (s, c) => s.activeAddressesTrend(c),
            metrics: 'AdrActCnt',
            pageSize: '31',
            body: _ethAddresses,
          ),
          'exchange_supply_change_30d': (
            call: (s, c) => s.exchangeSupplyChange30d(c),
            metrics: 'SplyExNtv',
            pageSize: '31',
            body: _cases['exchange_supply_change_30d']!.body('2026-10-30'),
          ),
          'eth_btc_trend_30d': (
            call: (s, c) => s.ethBtcTrend30d(c),
            metrics: 'PriceBTC',
            pageSize: '31',
            body: _ethPriceBtc,
          ),
          'exchange_netflow': (
            call: (s, c) => s.exchangeNetflow(c),
            metrics: 'FlowInExUSD,FlowOutExUSD',
            pageSize: '30',
            body: _cases['exchange_netflow']!.body('2026-10-30'),
          ),
          'gross_inflation_pct': (
            call: (s, c) => s.grossInflationPct(c),
            metrics: 'IssTotNtv,SplyCur',
            pageSize: '1',
            body: _ethIssuance,
          ),
        };

    (CoinMetricsService, List<Uri>) service(
      http.Response Function(Uri) handler,
    ) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return handler(request.url);
      });
      return (CoinMetricsService(client: client, now: () => _today), requests);
    }

    test('cada indicador pede ao CoinMetrics só as suas métricas e linhas',
        () async {
      for (final MapEntry(:key, :value) in calls.entries) {
        final (cm, requests) = service((_) => http.Response(value.body, 200));

        final result = await value.call(cm, 'ETH');

        expect(result.isFinite, isTrue, reason: key);
        final url = requests.single;
        expect(url.host, 'community-api.coinmetrics.io', reason: key);
        expect(url.path, '/v4/timeseries/asset-metrics', reason: key);
        expect(url.queryParameters, {
          'assets': 'eth',
          'metrics': value.metrics,
          'frequency': '1d',
          'page_size': value.pageSize,
        }, reason: key);
      }
    });

    test('o id do ativo é a moeda em minúsculas; BNB também é aceito', () async {
      for (final coin in ['ETH', 'btc', 'Ada', 'XRP', 'bnb']) {
        final (cm, requests) = service(
          (_) => http.Response(calls['mayer_multiple']!.body, 200),
        );

        await cm.mayerMultiple(coin);

        expect(requests.single.queryParameters['assets'], coin.toLowerCase());
      }
    });

    test('moeda fora do score é erro de uso, sem pedido à rede', () async {
      final (cm, requests) = service(
        (_) => http.Response(calls['mayer_multiple']!.body, 200),
      );

      await expectLater(cm.mayerMultiple('SOL'), throwsArgumentError);
      await expectLater(cm.mvrvZScore('DOGE'), throwsArgumentError);

      expect(requests, isEmpty);
    });

    test('dado velho depois do download vira erro (usa o relógio injetado)',
        () async {
      final stale = DateTime.utc(2026, 11, 7); // 8 dias depois de 2026-10-30
      final cm = CoinMetricsService(
        client: MockClient(
          (_) async => http.Response(calls['mayer_multiple']!.body, 200),
        ),
        now: () => stale,
      );

      await expectLater(cm.mayerMultiple('ETH'), _sourceError());
    });

    test('falha de HTTP sobe como erro da fonte, com o nome dela', () async {
      for (final status in [429, 503, 404]) {
        final (cm, _) = service((_) => http.Response('', status));

        await expectLater(
          cm.mayerMultiple('ETH'),
          _sourceError(),
          reason: '$status',
        );
      }
    });
  });
}

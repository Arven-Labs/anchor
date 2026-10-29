import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/coingecko_nvt_service.dart';
import 'package:anchor_mobile/services/crypto_source_http.dart';

// Os 61 últimos pontos reais de `market_caps` e `total_volumes` do ethereum em
// `/coins/ethereum/market_chart?vs_currency=usd&days=90&interval=daily`
// (2026-10-29): 60 dias fechados e o ponto de hoje, que vem no meio do dia
// (1793282640000). A resposta completa tem 91 pontos e também `prices`, que o
// parser ignora.
const _capsJson =
    r'[[1788134400000,291594867605.8527],[1788220800000,297667970873.7225],[1788307200000,291767768408.91174],[1788393600000,291704729861.0524],[1788480000000,305971011968.8528],[1788566400000,299686660200.51276],[1788652800000,302612300112.52106],[1788739200000,306769108939.70807],[1788825600000,303773075685.29297],[1788912000000,303252321699.8239],[1788998400000,301119066908.05597],[1789084800000,297463477324.3543],[1789171200000,306908073951.8628],[1789257600000,308187084056.6601],[1789344000000,302264990103.7054],[1789430400000,306919471184.89514],[1789516800000,292614926206.1527],[1789603200000,294875323390.85944],[1789689600000,298566484290.6061],[1789776000000,318763679004.78766],[1789862400000,321267925139.67786],[1789948800000,322828893559.51447],[1790035200000,338758308121.54047],[1790121600000,336014807500.0825],[1790208000000,327686843603.20233],[1790294400000,328034207688.9713],[1790380800000,328500746742.66174],[1790467200000,329084170100.4967],[1790553600000,328106879703.77484],[1790640000000,328161568756.54565],[1790726400000,326882384721.9066],[1790812800000,327783962324.3409],[1790899200000,330305392614.80695],[1790985600000,325753252610.3761],[1791072000000,328073789645.7034],[1791158400000,332846167523.6478],[1791244800000,330962511259.41034],[1791331200000,329352893752.80865],[1791417600000,314225721923.2913],[1791504000000,301959830755.3213],[1791590400000,303551579906.16174],[1791676800000,305831441169.97095],[1791763200000,309541484283.7787],[1791849600000,298710869490.4768],[1791936000000,304910319328.6137],[1792022400000,295525520649.86206],[1792108800000,298008599988.15814],[1792195200000,295645987052.46277],[1792281600000,293179515941.90076],[1792368000000,294106909234.6188],[1792454400000,293879026865.33386],[1792540800000,298841751224.8341],[1792627200000,302036532023.7666],[1792713600000,291514722930.01965],[1792800000000,291309985945.85144],[1792886400000,287874457234.6905],[1792972800000,284248605755.0931],[1793059200000,283503198237.63214],[1793145600000,286765901434.55536],[1793232000000,288012814110.9622],[1793282640000,283902445068.5447]]';
const _volsJson =
    r'[[1788134400000,10762275574.892046],[1788220800000,12336663342.988674],[1788307200000,12760379027.821028],[1788393600000,12772993324.596373],[1788480000000,16639170367.446753],[1788566400000,16234345954.343023],[1788652800000,6784086852.01072],[1788739200000,9948019407.716267],[1788825600000,11300837599.905233],[1788912000000,11965805217.194431],[1788998400000,13328861510.407354],[1789084800000,15266781786.913017],[1789171200000,24774688367.63771],[1789257600000,7645722055.298775],[1789344000000,9592266575.143324],[1789430400000,16123922989.756006],[1789516800000,19487916667.87325],[1789603200000,15793566275.442846],[1789689600000,11943767083.350086],[1789776000000,23593939177.26936],[1789862400000,11034395644.108103],[1789948800000,11749949626.385357],[1790035200000,27217607923.39273],[1790121600000,16456702906.15831],[1790208000000,17829847307.081375],[1790294400000,14564506864.808453],[1790380800000,14335818784.962042],[1790467200000,5851863209.137865],[1790553600000,8833449436.595728],[1790640000000,17048616414.52823],[1790726400000,15609520153.835028],[1790812800000,13831956665.772991],[1790899200000,13688689733.998228],[1790985600000,18947257093.519634],[1791072000000,5155711549.0922785],[1791158400000,6168266219.038013],[1791244800000,12360388631.332169],[1791331200000,11013438109.181702],[1791417600000,19380856621.938263],[1791504000000,19211446963.28274],[1791590400000,9237247812.481485],[1791676800000,5821686502.407691],[1791763200000,8566468363.628672],[1791849600000,13499532438.027359],[1791936000000,12664145444.948595],[1792022400000,14011311984.6873],[1792108800000,19072612435.258205],[1792195200000,12413691615.07689],[1792281600000,6392442801.18807],[1792368000000,8562877335.43275],[1792454400000,9189766395.664333],[1792540800000,11659953648.207783],[1792627200000,13777880330.779068],[1792713600000,12782685343.559818],[1792800000000,9909686957.943005],[1792886400000,9030947692.418264],[1792972800000,6329620691.518975],[1793059200000,10108268440.438486],[1793145600000,9363890189.034079],[1793232000000,9785092231.088587],[1793282640000,10461234934.239365]]';

typedef _Points = List<List<num>>;

_Points _decode(String json) => [
  for (final pair in jsonDecode(json) as List)
    [for (final n in pair as List) n as num],
];

final _caps = _decode(_capsJson);
final _vols = _decode(_volsJson);

String _chart(_Points caps, _Points volumes) =>
    jsonEncode({'market_caps': caps, 'total_volumes': volumes});

/// `previous` dias fechados com o NVT dado mais o de hoje, todos com volume 10.
(_Points, _Points) _series(
  int previous, {
  double previousNvt = 20,
  double todayNvt = 40,
}) {
  final caps = <List<num>>[];
  final volumes = <List<num>>[];
  for (var i = 0; i <= previous; i++) {
    final nvt = i == previous ? todayNvt : previousNvt;
    caps.add([1700000000000 + i * 86400000, nvt * 10]);
    volumes.add([1700000000000 + i * 86400000, 10]);
  }
  return (caps, volumes);
}

void main() {
  group('parseNvtVsMa90', () {
    test('série real do ETH: o NVT de hoje contra a média dos 60 anteriores', () {
      // NVT diário = market_cap ÷ volume; hoje ÷ média dos anteriores, calculado
      // à parte sobre os mesmos 61 pontos.
      expect(
        parseNvtVsMa90(_chart(_caps, _vols)),
        closeTo(0.991592364293119, 1e-9),
      );
    });

    test(
      'hoje é o último ponto, não o maior: 60 dias a 20 e hoje a 40 dão 2,0',
      () {
        final (caps, vols) = _series(60);

        expect(parseNvtVsMa90(_chart(caps, vols)), closeTo(2.0, 1e-12));
      },
    );

    test('ignora o campo prices que a resposta real traz', () {
      final body = jsonEncode({
        'prices': [
          [1793282640000, 2320.5],
        ],
        'market_caps': _caps,
        'total_volumes': _vols,
      });

      expect(parseNvtVsMa90(body), closeTo(0.991592364293119, 1e-9));
    });

    test('60 dias anteriores bastam; 59 não', () {
      final (caps60, vols60) = _series(60);
      final (caps59, vols59) = _series(59);

      expect(parseNvtVsMa90(_chart(caps60, vols60)), closeTo(2.0, 1e-12));
      expect(
        () => parseNvtVsMa90(_chart(caps59, vols59)),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            allOf(contains('CoinGecko NVT'), contains('history')),
          ),
        ),
      );
    });

    test('série vazia ou de um ponto é erro da fonte', () {
      for (final body in [
        _chart([], []),
        _chart(_caps.take(1).toList(), _vols.take(1).toList()),
      ]) {
        expect(
          () => parseNvtVsMa90(body),
          throwsA(isA<CryptoSourceException>()),
        );
      }
    });

    test('volume zero, negativo ou não numérico é recusado', () {
      for (final bad in <Object?>[0, -5, 'x', null]) {
        final (caps, vols) = _series(60);
        final body = jsonEncode({
          'market_caps': caps,
          'total_volumes': [
            for (var i = 0; i < vols.length; i++)
              i == 10 ? [vols[i][0], bad] : vols[i],
          ],
        });
        expect(
          () => parseNvtVsMa90(body),
          throwsA(isA<CryptoSourceException>()),
          reason: '$bad',
        );
      }
    });

    test('market cap negativo é recusado', () {
      final (caps, vols) = _series(60);
      caps[5] = [caps[5][0], -1];

      expect(
        () => parseNvtVsMa90(_chart(caps, vols)),
        throwsA(isA<CryptoSourceException>()),
      );
    });

    test('média anterior zero (todos os market caps zerados) é recusada', () {
      final (caps, vols) = _series(60, previousNvt: 0);

      expect(
        () => parseNvtVsMa90(_chart(caps, vols)),
        throwsA(isA<CryptoSourceException>()),
      );
    });

    test(
      'market_caps e total_volumes de tamanhos diferentes são recusados',
      () {
        // O `zip` do desktop trunca em silêncio; aqui a divergência é erro.
        final (caps, vols) = _series(60);

        expect(
          () =>
              parseNvtVsMa90(_chart(caps, vols.take(vols.length - 1).toList())),
          throwsA(isA<CryptoSourceException>()),
        );
      },
    );

    test('corpo fora do formato vira erro da fonte, não TypeError', () {
      for (final body in [
        '{}',
        '[]',
        '{"error":{"status":429}}',
        '{"market_caps":[[1,2]],"total_volumes":"x"}',
        '{"market_caps":[1,2],"total_volumes":[1,2]}',
        '{"market_caps":[[1]],"total_volumes":[[1]]}',
        '{"market_caps":[["a","b"]],"total_volumes":[[1,2]]}',
        'não é json',
      ]) {
        expect(
          () => parseNvtVsMa90(body),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('CoinGecko NVT'),
            ),
          ),
          reason: body,
        );
      }
    });
  });

  group('CoinGeckoNvtService', () {
    (CoinGeckoNvtService, List<Uri>) service(
      http.Response Function(Uri) handler,
    ) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return handler(request.url);
      });
      return (CoinGeckoNvtService(client: client), requests);
    }

    http.Response ok(Uri _) => http.Response(_chart(_caps, _vols), 200);

    test('pede 90 dias diários em USD do id CoinGecko da moeda', () async {
      final (coingecko, requests) = service(ok);

      final value = await coingecko.nvtVsMa90('ETH');

      expect(value, closeTo(0.991592364293119, 1e-9));
      expect(requests.single.host, 'api.coingecko.com');
      expect(requests.single.path, '/api/v3/coins/ethereum/market_chart');
      expect(requests.single.queryParameters, {
        'vs_currency': 'usd',
        'days': '90',
        'interval': 'daily',
      });
    });

    test('cada moeda do score usa o seu id; minúsculas funcionam', () async {
      final (coingecko, requests) = service(ok);

      for (final coin in ['ETH', 'btc', 'ADA', 'XRP', 'BNB']) {
        await coingecko.nvtVsMa90(coin);
      }

      expect(requests.map((u) => u.path.split('/')[4]), [
        'ethereum',
        'bitcoin',
        'cardano',
        'ripple',
        'binancecoin',
      ]);
    });

    test('moeda fora do score é erro de uso, sem pedido à rede', () async {
      final (coingecko, requests) = service(ok);

      await expectLater(coingecko.nvtVsMa90('SOL'), throwsArgumentError);

      expect(requests, isEmpty);
    });

    test('limite de requisições (429) sobe como erro da fonte', () async {
      final (coingecko, _) = service((_) => http.Response('', 429));

      await expectLater(
        coingecko.nvtVsMa90('ETH'),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('CoinGecko NVT'),
          ),
        ),
      );
    });
  });
}

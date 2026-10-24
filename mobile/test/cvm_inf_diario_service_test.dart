import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:archive/archive.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/domain/cvm_inf_diario.dart';
import 'package:anchor_mobile/services/cvm_inf_diario_service.dart';

void main() {
  DateTime d(String s) => DateTime.parse('${s}T00:00:00Z');

  const header =
      'TP_FUNDO_CLASSE;CNPJ_FUNDO_CLASSE;ID_SUBCLASSE;DT_COMPTC;VL_TOTAL;VL_QUOTA;VL_PATRIM_LIQ;CAPTC_DIA;RESG_DIA;NR_COTST';
  final csv = [
    header,
    'CLASSES - FIF;00.017.024/0001-53;;2026-10-01;1119426.37;44.990762600000;1221524.87;0.00;2902.97;1',
    'CLASSES - FIF;00.017.024/0001-53;;2026-10-02;1120004.10;45.009590600000;1222036.06;0.00;0.00;1',
    '',
  ].join('\r\n');

  Uint8List zipOf(String name, String content) => Uint8List.fromList(
    ZipEncoder().encode(
      Archive()..addFile(ArchiveFile.bytes(name, latin1.encode(content))),
    ),
  );

  late Directory cache;
  var requests = <Uri>[];
  var now = DateTime(2026, 10, 23, 12);

  setUp(() async {
    cache = await Directory.systemTemp.createTemp('anchor-cvm-test-');
    requests = [];
    now = DateTime(2026, 10, 23, 12);
  });

  tearDown(() async {
    if (await cache.exists()) await cache.delete(recursive: true);
  });

  CvmInfDiarioService service(
    FutureOr<http.Response> Function(http.Request) handler,
  ) => CvmInfDiarioService(
    client: MockClient((request) async {
      requests.add(request.url);
      return handler(request);
    }),
    cacheDir: () async => cache,
    now: () => now,
  );

  final targets = [const CvmTarget('00017024000153')];

  test('baixa o zip do mês, lê as cotas e guarda no cache', () async {
    final svc = service(
      (_) => http.Response.bytes(zipOf('inf_diario_fi_202610.csv', csv), 200),
    );

    final out = await svc.extractMonth((2026, 10), targets);

    expect(out, isNotNull);
    expect(out!.quotas.single, {
      d('2026-10-01'): 44.9907626,
      d('2026-10-02'): 45.0095906,
    });
    expect(requests.single.toString(), '${CvmInfDiarioService.baseUrl}/inf_diario_fi_202610.zip');
    expect(
      File('${cache.path}/cvm_inf_diario/inf_diario_fi_202610.zip').existsSync(),
      isTrue,
    );
    expect(
      Directory('${cache.path}/cvm_inf_diario').listSync().map((e) => e.path),
      everyElement(endsWith('.zip')),
      reason: 'sem .part nem .csv sobrando',
    );
  });

  test('o segundo pedido do mesmo mês sai do cache, sem rede', () async {
    final svc = service(
      (_) => http.Response.bytes(zipOf('inf_diario_fi_202610.csv', csv), 200),
    );

    await svc.extractMonth((2026, 10), targets);
    final again = await svc.extractMonth((2026, 10), targets);

    expect(requests, hasLength(1));
    expect(again!.quotas.single, hasLength(2));
  });

  test('o cache do mês recente vale 12 h e o do mês fechado, 30 dias', () async {
    final svc = service(
      (_) => http.Response.bytes(zipOf('inf_diario_fi_202610.csv', csv), 200),
    );
    await svc.extractMonth((2026, 10), targets);
    await svc.extractMonth((2026, 6), targets);
    expect(requests, hasLength(2));
    // A validade usa a data do arquivo: aqui ela vale o "agora" do teste.
    for (final file in Directory('${cache.path}/cvm_inf_diario').listSync()) {
      await (file as File).setLastModified(now);
    }

    // 13 h depois: o recente expirou, o fechado (junho) ainda vale
    now = now.add(const Duration(hours: 13));
    await svc.extractMonth((2026, 10), targets);
    await svc.extractMonth((2026, 6), targets);
    expect(requests, hasLength(3));
    expect(requests.last.path, endsWith('inf_diario_fi_202610.zip'));
  });

  test('404 é "mês ainda não publicado": devolve null e não guarda nada', () async {
    final svc = service((_) => http.Response('', 404));

    expect(await svc.extractMonth((2026, 10), targets), isNull);
    expect(
      Directory('${cache.path}/cvm_inf_diario').listSync(),
      isEmpty,
    );
  });

  test('429, 5xx e rede fora viram CvmUnavailableException', () async {
    for (final status in [429, 500, 503]) {
      final svc = service((_) => http.Response('', status));
      await expectLater(
        svc.extractMonth((2026, 10), targets),
        throwsA(isA<CvmUnavailableException>()),
        reason: 'HTTP $status',
      );
    }
    final offline = service((_) => throw const SocketException('sem rede'));
    await expectLater(
      offline.extractMonth((2026, 10), targets),
      throwsA(isA<CvmUnavailableException>()),
    );
    expect(Directory('${cache.path}/cvm_inf_diario').listSync(), isEmpty);
  });

  test('zip corrompido vira indisponível e sai do cache', () async {
    final svc = service(
      (_) => http.Response.bytes(utf8.encode('isto não é um zip'), 200),
    );

    await expectLater(
      svc.extractMonth((2026, 10), targets),
      throwsA(isA<CvmUnavailableException>()),
    );
    expect(Directory('${cache.path}/cvm_inf_diario').listSync(), isEmpty);
  });

  test('zip sem o CSV do informe é erro, não "sem cota"', () async {
    final svc = service(
      (_) => http.Response.bytes(zipOf('leia-me.txt', 'oi'), 200),
    );

    await expectLater(
      svc.extractMonth((2026, 10), targets),
      throwsA(
        isA<CvmUnavailableException>().having(
          (e) => e.detail,
          'detail',
          contains('inf_diario_fi_'),
        ),
      ),
    );
  });
}

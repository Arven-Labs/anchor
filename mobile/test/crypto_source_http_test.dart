import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/crypto_source_http.dart';

void main() {
  group('cryptoGet', () {
    Future<String> get(
      http.Response Function(http.Request) handler, {
      Duration timeout = const Duration(seconds: 5),
    }) => cryptoGet(
      MockClient((request) async => handler(request)),
      Uri.parse('https://exemplo.test/dado'),
      source: 'Fonte X',
      timeout: timeout,
    );

    test('200 devolve o corpo como texto', () async {
      expect(await get((_) => http.Response('{"ok":1}', 200)), '{"ok":1}');
    });

    test('429 e 5xx viram erro da fonte, com o nome dela', () async {
      for (final status in [429, 500, 503]) {
        await expectLater(
          get((_) => http.Response('', status)),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('Fonte X'),
            ),
          ),
          reason: '$status',
        );
      }
    });

    test('outros status (ex.: 451, bloqueio regional) dizem o HTTP', () async {
      for (final status in [400, 403, 404, 451]) {
        await expectLater(
          get((_) => http.Response('blocked', status)),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              allOf(contains('Fonte X'), contains('HTTP $status')),
            ),
          ),
          reason: '$status',
        );
      }
    });

    test('rede fora vira erro da fonte', () async {
      await expectLater(
        get((_) => throw const SocketException('sem rede')),
        throwsA(isA<CryptoSourceException>()),
      );
    });

    test('estouro de tempo vira erro da fonte', () async {
      final client = MockClient((_) async {
        await Future<void>.delayed(const Duration(milliseconds: 300));
        return http.Response('{}', 200);
      });

      await expectLater(
        cryptoGet(
          client,
          Uri.parse('https://exemplo.test/lento'),
          source: 'Fonte X',
          timeout: const Duration(milliseconds: 20),
        ),
        throwsA(isA<CryptoSourceException>()),
      );
    });

    test('sem charset no cabeçalho o corpo é lido como UTF-8', () async {
      // `http.Response.body` cai em Latin-1 sem charset; as fontes mandam UTF-8.
      final client = MockClient(
        (_) async => http.Response.bytes(utf8.encode('ação'), 200),
      );

      final body = await cryptoGet(
        client,
        Uri.parse('https://exemplo.test/x'),
        source: 'Fonte X',
      );

      expect(body, 'ação');
    });
  });

  group('parseSourceNumber', () {
    test('lê número comum, negativo e científico', () {
      expect(parseSourceNumber('Fonte X', 'campo', '0.00005355'), 0.00005355);
      expect(parseSourceNumber('Fonte X', 'campo', '-3.5'), -3.5);
      expect(parseSourceNumber('Fonte X', 'campo', '1e-4'), 0.0001);
    });

    test('recusa texto, vazio, NaN e infinito, dizendo fonte e campo', () {
      for (final text in ['abc', '', 'NaN', 'Infinity', '-Infinity']) {
        expect(
          () => parseSourceNumber('Fonte X', 'campo', text),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              allOf(contains('Fonte X'), contains('campo')),
            ),
          ),
          reason: '"$text"',
        );
      }
    });
  });
}

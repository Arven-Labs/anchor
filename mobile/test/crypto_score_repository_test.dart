import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';
import 'package:anchor_mobile/services/crypto_score_repository.dart';

void main() {
  final repository = CryptoScoreRepository();
  final asOf = DateTime(2026, 10, 29);
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-crypto-score-test-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<void> record(
    String coin,
    String indicator,
    double rawValue, {
    String date = '2026-10-28',
  }) => repository.recordReading(
    coin: coin,
    indicator: indicator,
    readingDate: date,
    rawValue: rawValue,
    source: 'teste',
  );

  Future<int> count(String table, {String? where, List<Object?>? args}) async {
    final db = await AppDatabase.instance();
    final rows = await db.rawQuery(
      'SELECT COUNT(*) AS n FROM $table${where == null ? '' : ' WHERE $where'}',
      args,
    );
    return rows.single['n']! as int;
  }

  Future<List<String>> signalsOf(String coin, String indicator) async {
    final readings = await repository.listReadings(coin);
    return [
      for (final r in readings)
        if (r.indicator == indicator) r.signal,
    ];
  }

  group('seeds e migração', () {
    test('banco novo nasce com os limites de fábrica de cada moeda', () async {
      expect(await count('indicator_thresholds'), 63);
      for (final (coin, size) in [
        (coinEth, 20),
        (coinBtc, 16),
        (coinAda, 10),
        (coinXrp, 9),
        (coinBnb, 8),
      ]) {
        final views = await repository.listThresholds(coin);
        expect(views, hasLength(size), reason: coin);
        expect(views.every((v) => v.isDefault), isTrue, reason: coin);
        expect(
          {
            for (final v in views)
              v.indicator: (v.greenBoundary, v.redBoundary),
          },
          {for (final (name, g, r) in defaultThresholds(coin)) name: (g, r)},
          reason: coin,
        );
      }
    });

    test('reabrir o banco não duplica os limites nem apaga edição', () async {
      await repository.updateThreshold(coinBtc, 'funding_rate_7d', 4.0, 12.0);
      await AppDatabase.resetForTest();

      expect(await count('indicator_thresholds'), 63);
      final funding = (await repository.listThresholds(
        coinBtc,
      )).singleWhere((v) => v.indicator == 'funding_rate_7d');
      expect((funding.greenBoundary, funding.redBoundary), (4.0, 12.0));
      expect(funding.isDefault, isFalse);
    });

    test('banco v20 migra sem perder linha e ganha os 63 limites', () async {
      final old = await openDatabase(
        '${dir.path}/anchor_mobile.db',
        version: 20,
        onCreate: (db, _) async {
          await db.execute('''
            CREATE TABLE assets (
              id INTEGER PRIMARY KEY AUTOINCREMENT, ticker TEXT NOT NULL,
              name TEXT NOT NULL, asset_class TEXT NOT NULL,
              currency TEXT NOT NULL, created_at TEXT NOT NULL)''');
          await db.execute('''
            CREATE TABLE bank_accounts (
              id INTEGER PRIMARY KEY AUTOINCREMENT, nome TEXT NOT NULL,
              titular TEXT NOT NULL, currency TEXT NOT NULL,
              created_at TEXT NOT NULL)''');
          await db.insert('assets', {
            'ticker': 'PETR4',
            'name': 'Petrobras',
            'asset_class': 'acao',
            'currency': 'BRL',
            'created_at': '2026-01-01T00:00:00.000',
          });
          await db.insert('bank_accounts', {
            'nome': 'Nubank',
            'titular': 'Eu',
            'currency': 'BRL',
            'created_at': '2026-01-01T00:00:00.000',
          });
        },
      );
      await old.close();

      expect(await repository.listThresholds(coinEth), hasLength(20));
      expect(await count('indicator_thresholds'), 63);
      expect(await count('crypto_indicators'), 0);
      expect(await count('assets', where: "ticker = 'PETR4'"), 1);
      expect(await count('bank_accounts', where: "nome = 'Nubank'"), 1);
    });
  });

  group('gravar leitura', () {
    test('classifica pelos limites da própria moeda', () async {
      // ETH: verde ≤ 5, vermelho ≥ 20. BTC: verde ≤ 5, vermelho ≥ 15.
      await record(coinEth, 'funding_rate_7d', 17);
      await record(coinBtc, 'funding_rate_7d', 17);

      expect(await signalsOf(coinEth, 'funding_rate_7d'), [
        Signal.neutral.wire,
      ]);
      expect(await signalsOf(coinBtc, 'funding_rate_7d'), [Signal.red.wire]);
    });

    test('moeda sem conjunto próprio usa os limites do ETH', () async {
      await record('SOL', 'funding_rate_7d', 17);

      expect(await signalsOf('SOL', 'funding_rate_7d'), [Signal.neutral.wire]);
    });

    test('guarda o valor bruto, a data e a fonte como vieram', () async {
      await record(coinEth, 'funding_rate_7d', 3.25, date: '2026-10-27');

      final reading = (await repository.listReadings(coinEth)).single;
      expect(reading.coin, coinEth);
      expect(reading.rawValue, 3.25);
      expect(reading.readingDate, '2026-10-27');
      expect(reading.source, 'teste');
      expect(reading.signal, Signal.green.wire);
    });

    test('recusa indicador que a moeda não tem, sem gravar nada', () async {
      // `sopr` existe no BTC mas não no conjunto da ADA.
      await expectLater(record(coinAda, 'sopr', 1.0), throwsArgumentError);
      await expectLater(
        record(coinEth, 'nao_existe', 1.0),
        throwsArgumentError,
      );

      expect(await count('crypto_indicators'), 0);
    });

    test('recusa valor não finito e data impossível', () async {
      for (final bad in [
        double.nan,
        double.infinity,
        double.negativeInfinity,
      ]) {
        await expectLater(
          record(coinEth, 'funding_rate_7d', bad),
          throwsArgumentError,
          reason: '$bad',
        );
      }
      for (final date in [
        '2026-02-30',
        '2026-10-28T10:00:00',
        '28/10/2026',
        '',
      ]) {
        await expectLater(
          record(coinEth, 'funding_rate_7d', 3, date: date),
          throwsArgumentError,
          reason: date,
        );
      }

      expect(await count('crypto_indicators'), 0);
    });

    test('é append-only: duas leituras do mesmo dia ficam as duas', () async {
      await record(coinEth, 'funding_rate_7d', 3);
      await record(coinEth, 'funding_rate_7d', 30);

      expect(await count('crypto_indicators'), 2);
    });
  });

  group('editar limite', () {
    test(
      'reclassifica as leituras guardadas, mantendo o valor bruto',
      () async {
        await record(coinBtc, 'funding_rate_7d', 10); // NEUTRAL com 5/15
        expect(await signalsOf(coinBtc, 'funding_rate_7d'), [
          Signal.neutral.wire,
        ]);

        final view = await repository.updateThreshold(
          coinBtc,
          'funding_rate_7d',
          4.0,
          8.0,
        );

        expect((view.greenBoundary, view.redBoundary), (4.0, 8.0));
        expect(view.isDefault, isFalse);
        expect(await signalsOf(coinBtc, 'funding_rate_7d'), [Signal.red.wire]);
        expect((await repository.listReadings(coinBtc)).single.rawValue, 10);
      },
    );

    test(
      'editar o BTC não toca as leituras do ETH nem de outra moeda',
      () async {
        await record(coinBtc, 'funding_rate_7d', 10);
        await record(coinEth, 'funding_rate_7d', 10);
        await record('SOL', 'funding_rate_7d', 10);

        await repository.updateThreshold(coinBtc, 'funding_rate_7d', 4.0, 8.0);

        expect(await signalsOf(coinBtc, 'funding_rate_7d'), [Signal.red.wire]);
        expect(await signalsOf(coinEth, 'funding_rate_7d'), [
          Signal.neutral.wire,
        ]);
        expect(await signalsOf('SOL', 'funding_rate_7d'), [
          Signal.neutral.wire,
        ]);
      },
    );

    test(
      'editar o ETH reclassifica as moedas sem conjunto próprio, não o BTC',
      () async {
        await record(coinEth, 'funding_rate_7d', 10);
        await record('SOL', 'funding_rate_7d', 10);
        await record(coinBtc, 'funding_rate_7d', 10);

        await repository.updateThreshold(coinEth, 'funding_rate_7d', 4.0, 8.0);

        expect(await signalsOf(coinEth, 'funding_rate_7d'), [Signal.red.wire]);
        expect(await signalsOf('SOL', 'funding_rate_7d'), [Signal.red.wire]);
        expect(await signalsOf(coinBtc, 'funding_rate_7d'), [
          Signal.neutral.wire,
        ]);
      },
    );

    test('limites iguais ou não finitos são recusados e nada muda', () async {
      await record(coinBtc, 'funding_rate_7d', 10);

      await expectLater(
        repository.updateThreshold(coinBtc, 'funding_rate_7d', 6.0, 6.0),
        throwsArgumentError,
      );
      await expectLater(
        repository.updateThreshold(coinBtc, 'funding_rate_7d', double.nan, 6.0),
        throwsArgumentError,
      );

      final funding = (await repository.listThresholds(
        coinBtc,
      )).singleWhere((v) => v.indicator == 'funding_rate_7d');
      expect(funding.isDefault, isTrue);
      expect(await signalsOf(coinBtc, 'funding_rate_7d'), [
        Signal.neutral.wire,
      ]);
    });

    test('indicador desconhecido na moeda é recusado', () async {
      await expectLater(
        repository.updateThreshold(coinAda, 'sopr', 1.0, 2.0),
        throwsArgumentError,
      );
    });

    test(
      'inverter o sentido (verde > vermelho) vira "maior é melhor"',
      () async {
        final view = await repository.updateThreshold(
          coinBtc,
          'funding_rate_7d',
          15.0,
          5.0,
        );

        expect(view.higherIsBetter, isTrue);
        await record(coinBtc, 'funding_rate_7d', 20);
        expect(await signalsOf(coinBtc, 'funding_rate_7d'), [
          Signal.green.wire,
        ]);
      },
    );
  });

  group('reset', () {
    test('volta ao valor de fábrica e reclassifica as leituras', () async {
      await record(coinBtc, 'funding_rate_7d', 10);
      await repository.updateThreshold(coinBtc, 'funding_rate_7d', 4.0, 8.0);
      expect(await signalsOf(coinBtc, 'funding_rate_7d'), [Signal.red.wire]);

      final view = await repository.resetThreshold(coinBtc, 'funding_rate_7d');

      expect((view.greenBoundary, view.redBoundary), (5.0, 15.0));
      expect(view.isDefault, isTrue);
      expect(await signalsOf(coinBtc, 'funding_rate_7d'), [
        Signal.neutral.wire,
      ]);
    });

    test('indicador que a moeda não tem é recusado', () async {
      await expectLater(
        repository.resetThreshold(coinAda, 'sopr'),
        throwsArgumentError,
      );
    });
  });

  group('listar limites', () {
    test('segue a ordem dos grupos do score e marca o sentido', () async {
      final views = await repository.listThresholds(coinEth);
      final order = [
        for (final group in IndicatorGroup.forCoin(coinEth))
          ...group.indicators(coinEth),
      ];

      expect(views.map((v) => v.indicator), order);
      final funding = views.singleWhere(
        (v) => v.indicator == 'funding_rate_7d',
      );
      final staking = views.singleWhere((v) => v.indicator == 'staking_yield');
      expect(funding.higherIsBetter, isFalse);
      expect(staking.higherIsBetter, isTrue); // verde 2.0, vermelho 0.0
    });

    test('moeda sem conjunto próprio mostra o do ETH', () async {
      final views = await repository.listThresholds('SOL');

      expect(views, hasLength(20));
      expect(views.every((v) => v.coin == coinEth), isTrue);
    });
  });

  group('score e histórico lidos do banco', () {
    test('score usa a última leitura de cada indicador', () async {
      await record(coinEth, 'funding_rate_7d', 3);
      await record(coinEth, 'fear_greed_index', 10);
      await record(coinEth, 'dvol', 90); // vermelho

      final score = await repository.score(coinEth, asOf);

      expect(score.total, 20);
      expect(score.logged, 3);
      expect(score.green, 2);
    });

    test('no mesmo dia vence a leitura gravada por último', () async {
      await record(coinEth, 'funding_rate_7d', 3); // verde
      await record(coinEth, 'funding_rate_7d', 30); // vermelho, depois

      final score = await repository.score(coinEth, asOf);

      expect(score.logged, 1);
      expect(score.green, 0);
    });

    test('leitura de data mais nova vence mesmo gravada antes', () async {
      await record(coinEth, 'funding_rate_7d', 30, date: '2026-10-28');
      await record(coinEth, 'funding_rate_7d', 3, date: '2026-10-20');

      final score = await repository.score(coinEth, asOf);

      expect(score.green, 0);
    });

    test('score só enxerga a própria moeda', () async {
      await record(coinBtc, 'funding_rate_7d', 3);

      final score = await repository.score(coinEth, asOf);

      expect(score.logged, 0);
    });

    test(
      'histórico bate com scoreHistory sobre as leituras em ordem',
      () async {
        await record(coinEth, 'funding_rate_7d', 3, date: '2026-10-26');
        await record(coinEth, 'fear_greed_index', 50, date: '2026-10-27');
        await record(coinEth, 'funding_rate_7d', 30, date: '2026-10-28');

        final history = await repository.scoreHistory(coinEth);

        expect(history.map((p) => p.date), [
          '2026-10-26',
          '2026-10-27',
          '2026-10-28',
        ]);
        expect(history.map((p) => (p.green, p.logged)), [
          (1, 1),
          (1, 2),
          (0, 2),
        ]);
      },
    );
  });
}

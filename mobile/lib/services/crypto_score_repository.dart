import 'package:sqflite/sqflite.dart' show DatabaseExecutor;

import '../data/db.dart';
import '../domain/crypto_score.dart';
import '../domain/crypto_score.dart' as domain show scoreHistory;
import '../models/crypto_reading.dart';
import '../models/indicator_threshold_view.dart';

/// Leituras e limites do score cripto, no banco do celular. A regra (classificar,
/// somar, janela de validade) fica em `domain/crypto_score.dart`, pura; aqui só
/// entra o que precisa de banco. Espelha `commands/crypto_indicator.rs` do desktop.
class CryptoScoreRepository {
  final DateTime Function() _now;

  /// [now] é o relógio (injetável nos testes). `created_at` das leituras é
  /// gravado em UTC, como no desktop: "coletado hoje" é por dia UTC, e a hora
  /// local sem fuso que o resto do app usa contaria o dia errado.
  CryptoScoreRepository({DateTime Function()? now})
    : _now = now ?? DateTime.now;

  /// Classifica a leitura pelos limites da moeda e a grava. Recusa valor não
  /// finito (o `classify` o trataria como neutro, sem avisar), data que não é
  /// `YYYY-MM-DD` de um dia real e indicador que a moeda não tem.
  Future<CryptoReading> recordReading({
    required String coin,
    required String indicator,
    required String readingDate,
    required double rawValue,
    required String source,
  }) async {
    if (!rawValue.isFinite) {
      throw ArgumentError('A leitura precisa ser um número finito');
    }
    if (!isValidReadingDate(readingDate)) {
      throw ArgumentError('Data da leitura inválida: $readingDate');
    }
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      final threshold = await _thresholdRow(txn, coin, indicator);
      final signal = classify(
        rawValue,
        Threshold(
          greenBoundary: threshold['green_boundary']! as double,
          redBoundary: threshold['red_boundary']! as double,
        ),
      );
      final row = {
        'coin': coin,
        'indicator': indicator,
        'reading_date': readingDate,
        'raw_value': rawValue,
        'signal': signal.wire,
        'source': source,
        'created_at': _now().toUtc().toIso8601String(),
      };
      final id = await txn.insert('crypto_indicators', row);
      return CryptoReading.fromMap({...row, 'id': id});
    });
  }

  /// Indicadores da moeda com leitura **criada** hoje (dia UTC), venha da fonte
  /// ou digitada à mão. Conta a criação e não a `reading_date`: SOPR e NUPL
  /// chegam com a data do dado 7 dias antes, e o que evita gastar a cota da
  /// fonte é saber que já foram buscados hoje.
  Future<Set<String>> indicatorsCollectedToday(String coin) async {
    final today = _now().toUtc().toIso8601String().substring(0, 10);
    final db = await AppDatabase.instance();
    final rows = await db.rawQuery(
      'SELECT DISTINCT indicator FROM crypto_indicators '
      'WHERE coin = ? AND substr(created_at, 1, 10) = ?',
      [coin, today],
    );
    return {for (final row in rows) row['indicator']! as String};
  }

  /// Leituras da moeda, da data mais nova para a mais antiga.
  Future<List<CryptoReading>> listReadings(String coin) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'crypto_indicators',
      where: 'coin = ?',
      whereArgs: [coin],
      orderBy: 'reading_date DESC, created_at DESC, id DESC',
    );
    return rows.map(CryptoReading.fromMap).toList();
  }

  /// Limites da moeda na ordem dos grupos do score (moeda sem conjunto próprio
  /// mostra os do ETH).
  Future<List<IndicatorThresholdView>> listThresholds(String coin) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'indicator_thresholds',
      where: 'coin = ?',
      whereArgs: [thresholdCoin(coin)],
    );
    final order = [
      for (final group in IndicatorGroup.forCoin(coin))
        ...group.indicators(coin),
    ];
    int position(Map<String, Object?> row) {
      final index = order.indexOf(row['indicator']! as String);
      return index == -1 ? order.length : index;
    }

    final sorted = [...rows]
      ..sort((a, b) {
        final byGroup = position(a).compareTo(position(b));
        if (byGroup != 0) return byGroup;
        return (a['indicator']! as String).compareTo(b['indicator']! as String);
      });
    return sorted.map(_view).toList();
  }

  /// Grava os limites e **reclassifica as leituras já guardadas** do indicador
  /// (o `raw_value` fica; só o `signal` muda), na mesma transação: o score lê o
  /// `signal` gravado, e sem isso ele ficaria com a régua antiga até a próxima
  /// coleta. Só as leituras que usam esses limites: editar o BTC mexe só nas do
  /// BTC; editar o ETH, nas dele e nas das moedas sem conjunto próprio.
  Future<IndicatorThresholdView> updateThreshold(
    String coin,
    String indicator,
    double greenBoundary,
    double redBoundary,
  ) async {
    validateBoundaries(greenBoundary, redBoundary);
    final threshold = Threshold(
      greenBoundary: greenBoundary,
      redBoundary: redBoundary,
    );
    final owner = thresholdCoin(coin);
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      final current = await _thresholdRow(txn, coin, indicator);
      final updatedAt = DateTime.now().toIso8601String();
      await txn.update(
        'indicator_thresholds',
        {
          'green_boundary': greenBoundary,
          'red_boundary': redBoundary,
          'updated_at': updatedAt,
        },
        where: 'id = ?',
        whereArgs: [current['id']],
      );

      // O ETH é dono das moedas sem conjunto próprio: tudo que não é de outro dono.
      final others = [coinBtc, coinAda, coinXrp, coinBnb];
      final readings = owner == coinEth
          ? await txn.query(
              'crypto_indicators',
              where:
                  'indicator = ? AND coin NOT IN (${others.map((_) => '?').join(', ')})',
              whereArgs: [indicator, ...others],
            )
          : await txn.query(
              'crypto_indicators',
              where: 'indicator = ? AND coin = ?',
              whereArgs: [indicator, owner],
            );
      for (final reading in readings) {
        final signal = classify(
          reading['raw_value']! as double,
          threshold,
        ).wire;
        if (reading['signal'] != signal) {
          await txn.update(
            'crypto_indicators',
            {'signal': signal},
            where: 'id = ?',
            whereArgs: [reading['id']],
          );
        }
      }

      return _view({
        ...current,
        'green_boundary': greenBoundary,
        'red_boundary': redBoundary,
        'updated_at': updatedAt,
      });
    });
  }

  /// Volta o indicador aos valores de fábrica (e reclassifica as leituras).
  Future<IndicatorThresholdView> resetThreshold(
    String coin,
    String indicator,
  ) async {
    final defaults = defaultThreshold(coin, indicator);
    if (defaults == null) {
      throw ArgumentError('Indicador desconhecido: $indicator');
    }
    return updateThreshold(coin, indicator, defaults.$1, defaults.$2);
  }

  /// Score da moeda visto em [asOf]: a última leitura de cada indicador. No
  /// mesmo dia vale a gravada por último (as leituras são append-only).
  Future<CryptoScore> score(String coin, DateTime asOf) async {
    final readings = await _readingsOf(coin, ascending: false);
    final latest = <LatestReading>[];
    for (final reading in readings) {
      if (latest.any((l) => l.indicator == reading.indicator)) continue;
      latest.add(reading);
    }
    return computeScore(coin, latest, asOf);
  }

  /// Evolução do score, um ponto por `readingDate`. Ordena pela data do dado,
  /// não pela da coleta: SOPR e NUPL chegam com a data de 7 dias antes.
  Future<List<ScoreHistoryPoint>> scoreHistory(String coin) async {
    return domain.scoreHistory(coin, await _readingsOf(coin, ascending: true));
  }

  Future<List<LatestReading>> _readingsOf(
    String coin, {
    required bool ascending,
  }) async {
    final db = await AppDatabase.instance();
    final direction = ascending ? 'ASC' : 'DESC';
    final rows = await db.query(
      'crypto_indicators',
      where: 'coin = ?',
      whereArgs: [coin],
      orderBy: 'reading_date $direction, created_at $direction, id $direction',
    );
    return [
      for (final row in rows)
        LatestReading(
          indicator: row['indicator']! as String,
          rawValue: row['raw_value']! as double,
          signal: row['signal']! as String,
          readingDate: row['reading_date']! as String,
          source: row['source']! as String,
        ),
    ];
  }

  /// Linha de limites do dono de [coin] para [indicator]; recusa o que a moeda
  /// não tem.
  Future<Map<String, Object?>> _thresholdRow(
    DatabaseExecutor db,
    String coin,
    String indicator,
  ) async {
    final rows = await db.query(
      'indicator_thresholds',
      where: 'coin = ? AND indicator = ?',
      whereArgs: [thresholdCoin(coin), indicator],
    );
    if (rows.isEmpty) {
      throw ArgumentError('Indicador desconhecido: $indicator');
    }
    return rows.first;
  }

  IndicatorThresholdView _view(Map<String, Object?> row) {
    final coin = row['coin']! as String;
    final indicator = row['indicator']! as String;
    final green = row['green_boundary']! as double;
    final red = row['red_boundary']! as double;
    final defaults = defaultThreshold(coin, indicator);
    return IndicatorThresholdView(
      coin: coin,
      indicator: indicator,
      greenBoundary: green,
      redBoundary: red,
      defaultGreen: defaults?.$1,
      defaultRed: defaults?.$2,
      isDefault: defaults != null && defaults.$1 == green && defaults.$2 == red,
      higherIsBetter: higherIsBetter(green, red),
      updatedAt: DateTime.parse(row['updated_at']! as String),
    );
  }
}

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';
import 'package:anchor_mobile/screens/crypto_score_screen.dart';
import 'package:anchor_mobile/services/crypto_collector.dart';
import 'package:anchor_mobile/services/crypto_score_repository.dart';
import 'package:anchor_mobile/theme.dart';

// Capturas da tela do score cripto para o CI publicar como artefato (o CI do
// mobile não roda emulador). Só grava se `SCREENSHOT_DIR` estiver definida; sem
// ela o teste passa sem fazer nada. Só dados inventados (o repositório é público).
void main() {
  final outDir = Platform.environment['SCREENSHOT_DIR'];
  final now = DateTime.utc(2026, 10, 29, 12);
  late Directory dir;
  late CryptoScoreRepository repository;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-score-shots-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
    repository = CryptoScoreRepository(now: () => now);
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<void> loadFonts() async {
    final root = Platform.environment['FLUTTER_ROOT'];
    if (root == null) throw StateError('FLUTTER_ROOT não definida');
    final fonts = '$root/bin/cache/artifacts/material_fonts';
    Future<ByteData> bytes(String file) async =>
        ByteData.sublistView(await File('$fonts/$file').readAsBytes());
    final roboto = FontLoader('Roboto')
      ..addFont(bytes('Roboto-Regular.ttf'))
      ..addFont(bytes('Roboto-Medium.ttf'))
      ..addFont(bytes('Roboto-Bold.ttf'));
    final icons = FontLoader('MaterialIcons')
      ..addFont(bytes('MaterialIcons-Regular.otf'));
    await roboto.load();
    await icons.load();
  }

  Future<void> settle(WidgetTester tester, bool Function() ready) async {
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 25)),
      );
      await tester.pump();
      if (ready()) break;
    }
    await tester.pump(const Duration(milliseconds: 300));
  }

  Future<void> capture(WidgetTester tester, String name) async {
    final boundary =
        tester.renderObject(find.byKey(const Key('shot'))) as RenderRepaintBoundary;
    await tester.runAsync(() async {
      final image = await boundary.toImage(pixelRatio: 2);
      final data = await image.toByteData(format: ui.ImageByteFormat.png);
      await Directory(outDir!).create(recursive: true);
      await File('$outDir/$name.png').writeAsBytes(data!.buffer.asUint8List());
    });
  }

  Future<void> show(
    WidgetTester tester,
    String name, {
    required String coin,
    CollectionResult? result,
    Future<void> Function()? beforeCapture,
  }) async {
    // 390 x 2700 lógicos: a tela do ETH tem 20 linhas e precisa caber inteira.
    tester.view.physicalSize = const Size(780, 5400);
    tester.view.devicePixelRatio = 2;
    addTearDown(tester.view.reset);
    addTearDown(() async {
      await tester.pumpWidget(const SizedBox());
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 300)),
      );
    });
    await tester.runAsync(AppDatabase.instance);
    await tester.pumpWidget(
      RepaintBoundary(
        key: const Key('shot'),
        child: MaterialApp(
          debugShowCheckedModeBanner: false,
          theme: buildAnchorTheme(),
          home: CryptoScoreScreen(
            active: true,
            repository: repository,
            now: () => now,
            collect: (_) async =>
                result ?? const CollectionResult(readings: [], errors: [], skipped: 0),
          ),
        ),
      ),
    );
    await settle(tester, () => find.text('On-chain').evaluate().isNotEmpty);
    if (coin != 'ETH') {
      await tester.tap(find.widgetWithText(ChoiceChip, coin));
      await settle(tester, () => find.text('Atualizar').evaluate().isNotEmpty);
    }
    await beforeCapture?.call();
    await capture(tester, name);
  }

  /// Série inventada de 4 meses para o gráfico do histórico: em cada data, os
  /// primeiros N indicadores do conjunto da moeda ficam verdes.
  Future<void> seedHistory(String coin) async {
    final limits = await repository.listThresholds(coin);
    final names = [
      for (final group in IndicatorGroup.forCoin(coin)) ...group.indicators(coin),
    ];
    const dates = [
      '2026-06-30',
      '2026-07-20',
      '2026-08-10',
      '2026-09-01',
      '2026-09-20',
      '2026-10-10',
    ];
    const greens = [3, 6, 5, 9, 12, 15];
    for (var d = 0; d < dates.length; d++) {
      for (final name in names.take(greens[d])) {
        final limit = limits.firstWhere((t) => t.indicator == name);
        await repository.recordReading(
          coin: coin,
          indicator: name,
          readingDate: dates[d],
          rawValue: limit.higherIsBetter
              ? limit.greenBoundary + 1
              : limit.greenBoundary - 1,
          source: 'coinmetrics',
        );
      }
    }
  }

  /// Leituras inventadas do conjunto da moeda: verde, neutro e vermelho em rodízio
  /// (pelos limites do banco), uma vencida e uma que fica sem leitura.
  Future<void> seedMix(String coin, {int? skip, int? expired}) async {
    final limits = await repository.listThresholds(coin);
    final names = [
      for (final group in IndicatorGroup.forCoin(coin)) ...group.indicators(coin),
    ];
    for (var i = 0; i < names.length; i++) {
      if (i == skip) continue;
      final limit = limits.firstWhere((t) => t.indicator == names[i]);
      final green = limit.greenBoundary;
      final red = limit.redBoundary;
      final towardGreen = limit.higherIsBetter ? 1.0 : -1.0;
      final value = switch (i % 3) {
        0 => green + towardGreen,
        1 => (green + red) / 2,
        _ => red - towardGreen,
      };
      await repository.recordReading(
        coin: coin,
        indicator: names[i],
        readingDate: i == expired ? '2026-09-02' : '2026-10-28',
        rawValue: value,
        source: 'coinmetrics',
      );
    }
  }

  testWidgets('capturas da tela do score cripto', (tester) async {
    if (outDir == null) return;
    await tester.runAsync(loadFonts);

    await tester.runAsync(() async {
      await seedHistory('ETH');
      await seedMix('ETH', skip: 7, expired: 5);
    });
    await show(tester, 'cripto_eth', coin: 'ETH');

    // Mesmo histórico, 90D marcado e um ponto tocado.
    await show(
      tester,
      'cripto_eth_historico_toque',
      coin: 'ETH',
      beforeCapture: () async {
        await tester.tap(find.widgetWithText(ChoiceChip, '90D'));
        // Espera a animação do chip: sem isso a captura pega o quadro do meio.
        await tester.pump(const Duration(milliseconds: 400));
        final rect = tester.getRect(find.byKey(const Key('history-chart')));
        await tester.tapAt(Offset(rect.left + rect.width * 0.55, rect.center.dy));
        await tester.pump(const Duration(milliseconds: 400));
      },
    );

    await tester.runAsync(() async {
      await seedMix('BTC', expired: 11);
    });
    await show(
      tester,
      'cripto_btc_falhas',
      coin: 'BTC',
      result: const CollectionResult(
        readings: [],
        errors: [
          'BTC sopr: bitcoin-data.com: unavailable (rate limit, server error, timeout or network)',
          'BTC exchange_netflow: CoinMetrics: HTTP 404',
        ],
        skipped: 0,
      ),
    );

    await show(tester, 'cripto_bnb_vazio', coin: 'BNB');
  });
}

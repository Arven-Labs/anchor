import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';
import 'package:anchor_mobile/models/indicator_threshold_view.dart';
import 'package:anchor_mobile/screens/crypto_thresholds_screen.dart';
import 'package:anchor_mobile/services/crypto_score_repository.dart';
import 'package:anchor_mobile/theme.dart';

// A tela de limites montada de verdade (SQLite ffi). Nenhum teste toca a rede.
class _FailingRepository extends CryptoScoreRepository {
  @override
  Future<IndicatorThresholdView> updateThreshold(
    String coin,
    String indicator,
    double greenBoundary,
    double redBoundary,
  ) async => throw ArgumentError('falha de teste');
}

void main() {
  late Directory dir;
  late CryptoScoreRepository repository;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-thresholds-screen-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
    repository = CryptoScoreRepository(now: () => DateTime.utc(2026, 10, 29, 12));
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<void> settle(WidgetTester tester, [bool Function()? ready]) async {
    for (var i = 0; i < (ready == null ? 12 : 200); i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 10)),
      );
      await tester.pump();
      if (ready != null && ready()) break;
    }
    await tester.pump(const Duration(milliseconds: 300));
  }

  Future<void> pumpScreen(
    WidgetTester tester, {
    String coin = coinEth,
    CryptoScoreRepository? repo,
  }) async {
    tester.view.physicalSize = const Size(800, 5000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    addTearDown(() async {
      await tester.pumpWidget(const SizedBox());
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 300)),
      );
    });
    await tester.runAsync(AppDatabase.instance);
    await tester.pumpWidget(
      MaterialApp(
        theme: buildAnchorTheme(),
        home: CryptoThresholdsScreen(coin: coin, repository: repo ?? repository),
      ),
    );
    await settle(
      tester,
      () => find.byKey(const ValueKey('threshold-mvrv_z_score')).evaluate().isNotEmpty ||
          find.byKey(const ValueKey('threshold-nvt_ratio')).evaluate().isNotEmpty,
    );
  }

  Future<void> inDb(WidgetTester tester, Future<void> Function() body) async {
    await tester.runAsync(body);
  }

  Finder row(String indicator) => find.byKey(ValueKey('threshold-$indicator'));

  Finder inRow(String indicator, String text) =>
      find.descendant(of: row(indicator), matching: find.text(text));

  Future<void> openDialog(WidgetTester tester, String indicator) async {
    await tester.tap(row(indicator));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
  }

  Future<void> typeLimits(WidgetTester tester, String green, String red) async {
    await tester.enterText(find.byKey(const Key('threshold-green')), green);
    await tester.enterText(find.byKey(const Key('threshold-red')), red);
    await tester.pump();
  }

  FilledButton saveButton(WidgetTester tester) =>
      tester.widget<FilledButton>(find.byKey(const Key('threshold-save')));

  OutlinedButton resetButton(WidgetTester tester) =>
      tester.widget<OutlinedButton>(find.byKey(const Key('threshold-reset')));

  group('a lista', () {
    testWidgets('mostra os 20 indicadores do ETH na ordem dos grupos do score', (tester) async {
      await pumpScreen(tester);

      final shown = <String>[
        for (final element in find.byWidgetPredicate(
          (w) => w.key is ValueKey<String> &&
              (w.key! as ValueKey<String>).value.startsWith('threshold-'),
        ).evaluate())
          (element.widget.key! as ValueKey<String>).value.substring('threshold-'.length),
      ];
      final expected = [
        for (final group in IndicatorGroup.forCoin(coinEth)) ...group.indicators(coinEth),
      ];
      expect(shown, expected);
      expect(shown.length, 20);
    });

    testWidgets('cada linha mostra o nome e a faixa verde/vermelho', (tester) async {
      await pumpScreen(tester);

      // MVRV (0, 7): menor é melhor. Staking yield (2, 0): maior é melhor.
      expect(inRow('mvrv_z_score', 'MVRV Z-Score'), findsOneWidget);
      expect(inRow('mvrv_z_score', 'Verde ≤ 0 · Vermelho ≥ 7'), findsOneWidget);
      expect(inRow('staking_yield', 'Staking yield real'), findsOneWidget);
      expect(inRow('staking_yield', 'Verde ≥ 2 · Vermelho ≤ 0'), findsOneWidget);
      // Nunca a chave interna.
      expect(find.textContaining('mvrv_z_score'), findsNothing);
    });

    testWidgets('marca Padrão e, depois de editar, Editado', (tester) async {
      await inDb(tester, () => repository.updateThreshold(coinEth, 'mvrv_z_score', 1, 8));
      await pumpScreen(tester);

      expect(inRow('mvrv_z_score', 'Editado'), findsOneWidget);
      expect(inRow('mvrv_z_score', 'Padrão'), findsNothing);
      expect(inRow('nvt_ratio', 'Padrão'), findsOneWidget);
    });

    testWidgets('mostra os limites da moeda aberta (BTC tem 16)', (tester) async {
      await pumpScreen(tester, coin: coinBtc);

      final rows = find.byWidgetPredicate(
        (w) => w.key is ValueKey<String> &&
            (w.key! as ValueKey<String>).value.startsWith('threshold-'),
      );
      expect(rows.evaluate().length, 16);
    });
  });

  group('o diálogo', () {
    testWidgets('abre com os valores atuais e a direção', (tester) async {
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');

      expect(find.text('MVRV Z-Score'), findsWidgets);
      expect(
        tester.widget<TextField>(find.byKey(const Key('threshold-green'))).controller!.text,
        '0',
      );
      expect(
        tester.widget<TextField>(find.byKey(const Key('threshold-red'))).controller!.text,
        '7',
      );
      expect(find.text('Menor é melhor'), findsOneWidget);
      expect(find.textContaining('Padrão: verde 0, vermelho 7'), findsOneWidget);
    });

    testWidgets('Salvar começa desabilitado (nada mudou)', (tester) async {
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');

      expect(saveButton(tester).onPressed, isNull);
    });

    testWidgets('Salvar desabilita com valor vazio, texto ou iguais, e diz por quê', (tester) async {
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');

      await typeLimits(tester, '', '7');
      expect(saveButton(tester).onPressed, isNull);
      expect(find.text('Digite dois números.'), findsOneWidget);

      await typeLimits(tester, 'abc', '7');
      expect(saveButton(tester).onPressed, isNull);

      await typeLimits(tester, '5', '5');
      expect(saveButton(tester).onPressed, isNull);
      expect(find.text('Use dois números diferentes.'), findsOneWidget);
    });

    testWidgets('trocar os valores troca o rótulo da direção', (tester) async {
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');

      await typeLimits(tester, '9', '2');

      expect(find.text('Maior é melhor'), findsOneWidget);
      expect(find.text('Menor é melhor'), findsNothing);
    });

    testWidgets('aceita vírgula e habilita o Salvar', (tester) async {
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');

      await typeLimits(tester, '0,5', '7,5');

      expect(saveButton(tester).onPressed, isNotNull);
    });

    testWidgets('Salvar grava o limite, fecha o diálogo e a linha vira Editado', (tester) async {
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');
      await typeLimits(tester, '1,5', '8');

      await tester.tap(find.byKey(const Key('threshold-save')));
      await settle(tester, () => inRow('mvrv_z_score', 'Editado').evaluate().isNotEmpty);

      expect(find.byKey(const Key('threshold-save')), findsNothing, reason: 'diálogo fechou');
      expect(inRow('mvrv_z_score', 'Editado'), findsOneWidget);
      expect(inRow('mvrv_z_score', 'Verde ≤ 1,5 · Vermelho ≥ 8'), findsOneWidget);

      late List<IndicatorThresholdView> saved;
      await inDb(tester, () async => saved = await repository.listThresholds(coinEth));
      final mvrv = saved.firstWhere((t) => t.indicator == 'mvrv_z_score');
      expect(mvrv.greenBoundary, 1.5);
      expect(mvrv.redBoundary, 8.0);
    });

    testWidgets('Salvar reclassifica o sinal da leitura já gravada', (tester) async {
      await inDb(tester, () async {
        // MVRV (0, 7), menor é melhor: -1 é verde.
        await repository.recordReading(
          coin: coinEth,
          indicator: 'mvrv_z_score',
          readingDate: '2026-10-28',
          rawValue: -1,
          source: 'coinmetrics',
        );
      });
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');
      // Verde até -100 e vermelho a partir de -50: o -1 passa a vermelho.
      await typeLimits(tester, '-100', '-50');

      await tester.tap(find.byKey(const Key('threshold-save')));
      await settle(tester, () => inRow('mvrv_z_score', 'Editado').evaluate().isNotEmpty);

      late List<dynamic> readings;
      await inDb(tester, () async => readings = await repository.listReadings(coinEth));
      expect(readings.single.signal, 'RED');
      expect(readings.single.rawValue, -1.0, reason: 'o valor bruto não muda');
    });

    testWidgets('o diálogo avisa que a edição muda o histórico', (tester) async {
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');

      expect(find.textContaining('histórico'), findsOneWidget);
    });
  });

  group('redefinir', () {
    testWidgets('começa desabilitado quando já é o padrão', (tester) async {
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');

      expect(resetButton(tester).onPressed, isNull);
    });

    testWidgets('volta aos valores de fábrica e a linha volta a Padrão', (tester) async {
      await inDb(tester, () => repository.updateThreshold(coinEth, 'mvrv_z_score', 1, 8));
      await pumpScreen(tester);
      expect(inRow('mvrv_z_score', 'Editado'), findsOneWidget);

      await openDialog(tester, 'mvrv_z_score');
      expect(resetButton(tester).onPressed, isNotNull);
      await tester.tap(find.byKey(const Key('threshold-reset')));
      await settle(tester, () => inRow('mvrv_z_score', 'Padrão').evaluate().isNotEmpty);

      expect(inRow('mvrv_z_score', 'Padrão'), findsOneWidget);
      expect(inRow('mvrv_z_score', 'Verde ≤ 0 · Vermelho ≥ 7'), findsOneWidget);
      expect(find.byKey(const Key('threshold-reset')), findsNothing, reason: 'diálogo fechou');
    });
  });

  group('erro do repositório', () {
    testWidgets('aparece no diálogo, que continua aberto', (tester) async {
      await pumpScreen(tester, repo: _FailingRepository());
      await openDialog(tester, 'mvrv_z_score');
      await typeLimits(tester, '1', '8');

      await tester.tap(find.byKey(const Key('threshold-save')));
      await settle(tester, () => find.byKey(const Key('threshold-error')).evaluate().isNotEmpty);

      expect(find.byKey(const Key('threshold-error')), findsOneWidget);
      expect(find.textContaining('falha de teste'), findsOneWidget);
      expect(find.byKey(const Key('threshold-save')), findsOneWidget, reason: 'segue aberto');
    });
  });

  group('cancelar', () {
    testWidgets('fecha sem gravar', (tester) async {
      await pumpScreen(tester);
      await openDialog(tester, 'mvrv_z_score');
      await typeLimits(tester, '3', '9');

      await tester.tap(find.text('Cancelar'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));

      expect(find.byKey(const Key('threshold-save')), findsNothing);
      expect(inRow('mvrv_z_score', 'Padrão'), findsOneWidget);
    });
  });
}

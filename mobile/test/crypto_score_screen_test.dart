import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';
import 'package:anchor_mobile/screens/crypto_score_screen.dart';
import 'package:anchor_mobile/services/crypto_collector.dart';
import 'package:anchor_mobile/services/crypto_score_repository.dart';
import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/theme.dart';

// A tela do score montada de verdade (SQLite ffi) com a coleta trocada por uma
// função falsa: nenhum teste aqui toca a rede.
void main() {
  late Directory dir;
  late DateTime current;
  late CryptoScoreRepository repository;
  late List<String> collected;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-score-screen-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
    current = DateTime.utc(2026, 10, 29, 12);
    repository = CryptoScoreRepository(now: () => current);
    collected = [];
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  /// Coleta que só anota a moeda e devolve [result].
  Future<CollectionResult> Function(String) recording([
    CollectionResult result = const CollectionResult(
      readings: [],
      errors: [],
      skipped: 0,
    ),
  ]) =>
      (coin) async {
        collected.add(coin);
        return result;
      };

  Widget app({
    required bool active,
    required Future<CollectionResult> Function(String) collect,
  }) => MaterialApp(
    theme: buildAnchorTheme(),
    home: CryptoScoreScreen(
      active: active,
      repository: repository,
      collect: collect,
      now: () => current,
    ),
  );

  // O SQLite ffi responde fora do relógio falso do teste: sonda até a condição
  // valer, em vez de esperar um tempo fixo.
  Future<void> settle(WidgetTester tester, [bool Function()? ready]) async {
    // Sem condição, deixa o que está pendente correr um pouco e segue.
    for (var i = 0; i < (ready == null ? 12 : 200); i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 10)),
      );
      await tester.pump();
      if (ready != null && ready()) break;
    }
    await tester.pump(const Duration(milliseconds: 200));
  }

  Future<void> pumpScreen(
    WidgetTester tester, {
    bool active = true,
    Future<CollectionResult> Function(String)? collect,
  }) async {
    tester.view.physicalSize = const Size(800, 5000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    // Antes de o `tearDown` fechar o banco: desmonta a tela (o que ela ainda não
    // pediu ao banco não será mais pedido) e deixa terminar o que já está em voo.
    // Fechar o SQLite ffi por baixo de uma consulta pendente dá "disk I/O error".
    addTearDown(() async {
      await tester.pumpWidget(const SizedBox());
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 300)),
      );
    });
    // `AppDatabase.instance()` não protege a primeira abertura concorrente (dois
    // chamadores veem `null` e abrem); abrir antes evita que a tela e a coleta
    // disputem a abertura, que só é o caso do teste, não do uso normal.
    await tester.runAsync(AppDatabase.instance);
    await tester.pumpWidget(app(active: active, collect: collect ?? recording()));
    // Pronta quando o primeiro score carregou (o primeiro grupo aparece).
    await settle(tester, () => find.text('On-chain').evaluate().isNotEmpty);
  }

  Future<void> seed(
    String coin,
    String indicator,
    double value, {
    String date = '2026-10-28',
    String source = 'coinmetrics',
  }) => repository.recordReading(
    coin: coin,
    indicator: indicator,
    readingDate: date,
    rawValue: value,
    source: source,
  );

  /// Grava [count] leituras VERDES do conjunto da moeda (pega os limites do banco
  /// e vai um passo para o lado verde de cada um).
  Future<void> seedGreens(String coin, int count, {String date = '2026-10-28'}) async {
    final limits = await repository.listThresholds(coin);
    final names = [
      for (final group in IndicatorGroup.forCoin(coin)) ...group.indicators(coin),
    ];
    for (final name in names.take(count)) {
      final limit = limits.firstWhere((t) => t.indicator == name);
      await seed(
        coin,
        name,
        limit.higherIsBetter ? limit.greenBoundary + 1 : limit.greenBoundary - 1,
        date: date,
      );
    }
  }

  /// Gravação no banco a partir do corpo do teste: a E/S real do SQLite só anda
  /// dentro de `runAsync`; fora dele o `await` fica parado no relógio falso.
  Future<void> inDb(WidgetTester tester, Future<void> Function() body) async {
    await tester.runAsync(body);
  }

  Finder inRow(String indicator, String text) => find.descendant(
    of: find.byKey(ValueKey('row-$indicator')),
    matching: find.text(text),
  );

  group('o que a tela mostra', () {
    testWidgets('as 5 moedas, a ETH selecionada e os grupos dela', (tester) async {
      await pumpScreen(tester);

      for (final coin in ['ETH', 'BTC', 'ADA', 'XRP', 'BNB']) {
        expect(find.widgetWithText(ChoiceChip, coin), findsOneWidget, reason: coin);
      }
      expect(
        tester.widget<ChoiceChip>(find.widgetWithText(ChoiceChip, 'ETH')).selected,
        isTrue,
      );
      for (final group in [
        'On-chain',
        'Fundamentos da rede',
        'Derivativos e opções',
        'Liquidez de mercado',
        'Sentimento de mercado',
      ]) {
        expect(find.text(group), findsOneWidget, reason: group);
      }
    });

    testWidgets('sem nenhuma leitura: aviso para coletar e todos "Sem leitura"', (
      tester,
    ) async {
      await pumpScreen(tester);

      expect(
        find.text('Nenhuma leitura ainda. Toque em Atualizar para coletar.'),
        findsOneWidget,
      );
      expect(find.text('Sem leitura'), findsNWidgets(20));
      expect(find.text('0 verdes de 20'), findsOneWidget);
      expect(find.text('0 com leitura válida'), findsOneWidget);
      expect(find.text('Reduzir risco'), findsOneWidget);
    });

    testWidgets('leituras: valor formatado, sinal em texto, fonte e data do dado', (
      tester,
    ) async {
      await inDb(tester, () async {
        await seed('ETH', 'mvrv_z_score', -0.5); // verde (≤ 0)
        await seed('ETH', 'puell_multiple', 5); // vermelho (≥ 4)
        await seed('ETH', 'mayer_multiple', 1.6, source: 'coinmetrics'); // neutro
        await seed(
          'ETH',
          'staking_yield',
          3,
          source: 'defillama+coinmetrics',
        ); // verde
      });

      await pumpScreen(tester);

      expect(inRow('mvrv_z_score', 'MVRV Z-Score'), findsOneWidget);
      expect(inRow('mvrv_z_score', '-0,50'), findsOneWidget);
      expect(inRow('mvrv_z_score', 'Verde'), findsOneWidget);
      expect(inRow('mvrv_z_score', 'coinmetrics · 28/10/2026'), findsOneWidget);
      expect(inRow('puell_multiple', '5,00'), findsOneWidget);
      expect(inRow('puell_multiple', 'Vermelho'), findsOneWidget);
      expect(inRow('mayer_multiple', '1,60'), findsOneWidget);
      expect(inRow('mayer_multiple', 'Neutro'), findsOneWidget);
      expect(inRow('staking_yield', '3,00 %'), findsOneWidget);
      expect(
        inRow('staking_yield', 'defillama+coinmetrics · 28/10/2026'),
        findsOneWidget,
      );
      // Sem aviso de "nenhuma leitura" quando há leitura.
      expect(find.textContaining('Nenhuma leitura ainda'), findsNothing);
      // Dois verdes de 20 = 10 %, quatro lidas.
      expect(find.text('2 verdes de 20'), findsOneWidget);
      expect(find.text('4 com leitura válida'), findsOneWidget);
    });

    testWidgets('o subscore de cada grupo é verdes/total do grupo', (tester) async {
      await inDb(tester, () async {
        await seed('ETH', 'mvrv_z_score', -0.5);
        await seed('ETH', 'puell_multiple', -1);
      });

      await pumpScreen(tester);

      // On-chain tem 6 indicadores; os 2 verdes estão nele.
      final onChain = find.byKey(const ValueKey('group-onChain'));
      expect(
        find.descendant(of: onChain, matching: find.text('2/6')),
        findsOneWidget,
      );
      final sentiment = find.byKey(const ValueKey('group-sentiment'));
      expect(
        find.descendant(of: sentiment, matching: find.text('0/3')),
        findsOneWidget,
      );
    });

    testWidgets('singular: "1 verde de 20"', (tester) async {
      await inDb(tester, () => seed('ETH', 'mvrv_z_score', -0.5));

      await pumpScreen(tester);

      expect(find.text('1 verde de 20'), findsOneWidget);
      expect(find.text('1 com leitura válida'), findsOneWidget);
    });

    testWidgets('leitura vencida continua visível, mas não conta verde nem válida', (
      tester,
    ) async {
      // 58 dias antes de 2026-10-29; o prazo do MVRV é 14.
      await inDb(
        tester,
        () => seed('ETH', 'mvrv_z_score', -0.5, date: '2026-09-01'),
      );

      await pumpScreen(tester);

      expect(inRow('mvrv_z_score', '-0,50'), findsOneWidget);
      expect(inRow('mvrv_z_score', 'Vencida'), findsOneWidget);
      expect(inRow('mvrv_z_score', 'Verde'), findsNothing);
      expect(inRow('mvrv_z_score', 'coinmetrics · 01/09/2026'), findsOneWidget);
      expect(find.text('0 verdes de 20'), findsOneWidget);
      expect(find.text('0 com leitura válida'), findsOneWidget);
      // Tem leitura (vencida): não é o estado vazio.
      expect(find.textContaining('Nenhuma leitura ainda'), findsNothing);
    });

    // Veredito pela fração de verdes sobre o TOTAL do conjunto. BNB tem 8
    // indicadores: 6 verdes = 75 % (intacta); 4 = 50 % (neutro); 2 = 25 %. Um teste
    // por caso, cada um com banco novo (o arquivo do banco sobrevive ao reset).
    for (final (greens, verdict) in [
      (6, 'Tese intacta — manter / seguir aportando'),
      (4, 'Neutro — acompanhar de perto'),
      (2, 'Reduzir risco'),
    ]) {
      testWidgets('veredito com $greens verdes de 8: $verdict', (tester) async {
        await inDb(tester, () => seedGreens('BNB', greens));

        await pumpScreen(tester);
        await tester.tap(find.widgetWithText(ChoiceChip, 'BNB'));
        await settle(
          tester,
          () => find.text('$greens verdes de 8').evaluate().isNotEmpty,
        );

        expect(find.text(verdict), findsOneWidget);
        expect(find.text('$greens verdes de 8'), findsOneWidget);
      });
    }

    testWidgets('trocar de moeda troca os grupos: o BTC não tem fundamentos da rede', (
      tester,
    ) async {
      await pumpScreen(tester);
      expect(find.text('Fundamentos da rede'), findsOneWidget);

      await tester.tap(find.widgetWithText(ChoiceChip, 'BTC'));
      await settle(tester, () => find.text('SOPR').evaluate().isNotEmpty);

      expect(find.text('Fundamentos da rede'), findsNothing);
      expect(find.text('SOPR'), findsOneWidget);
      expect(find.text('0 verdes de 16'), findsOneWidget);
    });
  });

  group('coleta automática (1x por moeda por sessão) e botão Atualizar', () {
    testWidgets('com a aba inativa nada é coletado', (tester) async {
      await pumpScreen(tester, active: false);

      expect(collected, isEmpty);
    });

    testWidgets('ao ativar a aba coleta a moeda selecionada, uma vez só', (
      tester,
    ) async {
      await pumpScreen(tester, active: false);

      await tester.pumpWidget(app(active: true, collect: recording()));
      await settle(tester);
      // Sai e volta para a aba: já foi coletado nesta sessão.
      await tester.pumpWidget(app(active: false, collect: recording()));
      await settle(tester);
      await tester.pumpWidget(app(active: true, collect: recording()));
      await settle(tester);

      expect(collected, ['ETH']);
    });

    testWidgets('moeda nova coleta uma vez; voltar a uma já coletada não repete', (
      tester,
    ) async {
      await pumpScreen(tester); // ativa de saída: ETH
      await settle(tester);

      await tester.tap(find.widgetWithText(ChoiceChip, 'BNB'));
      await settle(tester);
      await tester.tap(find.widgetWithText(ChoiceChip, 'ETH'));
      await settle(tester);
      await tester.tap(find.widgetWithText(ChoiceChip, 'BNB'));
      await settle(tester);

      expect(collected, ['ETH', 'BNB']);
    });

    testWidgets('Atualizar força a coleta mesmo já coletada', (tester) async {
      await pumpScreen(tester);
      await settle(tester);
      expect(collected, ['ETH']);

      await tester.tap(find.text('Atualizar'));
      await settle(tester);

      expect(collected, ['ETH', 'ETH']);
    });

    testWidgets('durante a coleta o botão mostra "Atualizando…" e clique duplo não repete', (
      tester,
    ) async {
      final pending = Completer<CollectionResult>();
      final calls = <String>[];
      Future<CollectionResult> slow(String coin) {
        calls.add(coin);
        return pending.future;
      }

      await pumpScreen(tester, collect: slow); // a automática já fica pendente
      await settle(tester);
      expect(find.text('Atualizando…'), findsOneWidget);
      expect(find.text('Atualizar'), findsNothing);

      await tester.tap(find.text('Atualizando…'), warnIfMissed: false);
      await tester.pump();
      expect(calls, ['ETH']);

      pending.complete(
        const CollectionResult(readings: [], errors: [], skipped: 0),
      );
      await settle(tester, () => find.text('Atualizar').evaluate().isNotEmpty);
      expect(find.text('Atualizar'), findsOneWidget);
    });

    testWidgets('terminada a coleta, a tela relê o banco e mostra a leitura nova', (
      tester,
    ) async {
      Future<CollectionResult> collectAndWrite(String coin) async {
        collected.add(coin);
        await seed(coin, 'mvrv_z_score', -0.5);
        return const CollectionResult(readings: [], errors: [], skipped: 0);
      }

      await pumpScreen(tester, collect: collectAndWrite);
      await settle(tester, () => inRow('mvrv_z_score', 'Verde').evaluate().isNotEmpty);

      expect(inRow('mvrv_z_score', 'Verde'), findsOneWidget);
      expect(find.text('1 verde de 20'), findsOneWidget);
    });
  });

  group('histórico (cartão com o gráfico)', () {
    Finder summary(String text) => find.descendant(
      of: find.byKey(const Key('history-summary')),
      matching: find.textContaining(text),
    );

    testWidgets('sem nenhuma leitura o cartão avisa que não há histórico', (tester) async {
      await pumpScreen(tester);

      expect(find.byKey(const Key('history-card')), findsOneWidget);
      expect(find.text('Sem histórico ainda.'), findsOneWidget);
      expect(find.byKey(const Key('history-chart')), findsNothing);
    });

    testWidgets('com leituras o cartão mostra a série da moeda, abaixo do veredito', (tester) async {
      await inDb(tester, () async {
        await seedGreens(coinEth, 2);
        // Os mesmos 2 indicadores em outro dia, para a série ter dois pontos.
        await seedGreens(coinEth, 2, date: '2026-10-29');
      });
      await pumpScreen(tester);
      await settle(tester, () => find.byKey(const Key('history-chart')).evaluate().isNotEmpty);

      expect(find.byKey(const Key('history-chart')), findsOneWidget);
      expect(summary('2 datas'), findsOneWidget);
      // Ordem na tela: o veredito vem antes do histórico, que vem antes dos grupos.
      final verdictY = tester.getTopLeft(find.text('2 verdes de 20')).dy;
      final historyY = tester.getTopLeft(find.byKey(const Key('history-card'))).dy;
      final groupY = tester.getTopLeft(find.byKey(const ValueKey('group-onChain'))).dy;
      expect(verdictY, lessThan(historyY));
      expect(historyY, lessThan(groupY));
    });

    testWidgets('trocar de moeda recarrega o histórico daquela moeda', (tester) async {
      await inDb(tester, () async {
        await seedGreens(coinEth, 2);
        await seedGreens(coinBtc, 1);
      });
      await pumpScreen(tester);
      await settle(tester, () => summary('2 verdes de 20').evaluate().isNotEmpty);

      await tester.tap(find.widgetWithText(ChoiceChip, 'BTC'));
      await settle(tester, () => summary('1 verde de 16').evaluate().isNotEmpty);

      expect(summary('1 verde de 16'), findsOneWidget);
      expect(summary('2 verdes de 20'), findsNothing);
    });

    testWidgets('moeda sem leitura troca o gráfico pelo aviso', (tester) async {
      await inDb(tester, () => seedGreens(coinEth, 2));
      await pumpScreen(tester);
      await settle(tester, () => find.byKey(const Key('history-chart')).evaluate().isNotEmpty);

      await tester.tap(find.widgetWithText(ChoiceChip, 'ADA'));
      await settle(tester, () => find.text('Sem histórico ainda.').evaluate().isNotEmpty);

      expect(find.byKey(const Key('history-chart')), findsNothing);
    });
  });

  group('falhas da coleta', () {
    testWidgets('as linhas de erro aparecem e somem na coleta seguinte sem erro', (
      tester,
    ) async {
      var fail = true;
      Future<CollectionResult> flaky(String coin) async => CollectionResult(
        readings: const [],
        errors: fail
            ? ['ETH mvrv_z_score: CoinMetrics: unavailable', 'ETH sopr: x']
            : const [],
        skipped: 0,
      );

      await pumpScreen(tester, collect: flaky);
      await settle(tester, () => find.text('Falhas da última coleta').evaluate().isNotEmpty);

      expect(find.text('Falhas da última coleta'), findsOneWidget);
      // O nome do indicador, não a chave do banco.
      expect(find.text('ETH MVRV Z-Score: CoinMetrics: unavailable'), findsOneWidget);
      expect(find.text('ETH SOPR: x'), findsOneWidget);
      expect(find.textContaining('mvrv_z_score'), findsNothing);

      fail = false;
      await tester.tap(find.text('Atualizar'));
      await settle(tester, () => find.text('Falhas da última coleta').evaluate().isEmpty);

      expect(find.text('Falhas da última coleta'), findsNothing);
    });

    testWidgets('coleta que lança erro de fonte: a mensagem aparece, a tela segue de pé', (
      tester,
    ) async {
      Future<CollectionResult> down(String coin) async =>
          throw const CryptoSourceException('crypto score', 'ETH x: unavailable');

      await pumpScreen(tester, collect: down);
      await settle(tester, () => find.text('Falhas da última coleta').evaluate().isNotEmpty);

      expect(find.text('Falhas da última coleta'), findsOneWidget);
      expect(find.textContaining('ETH x: unavailable'), findsOneWidget);
      expect(find.text('Atualizar'), findsOneWidget);
    });

    testWidgets('bug na coleta (não é erro de fonte) aparece como erro inesperado, sem derrubar a tela', (
      tester,
    ) async {
      Future<CollectionResult> broken(String coin) async => throw StateError('bug');

      await pumpScreen(tester, collect: broken);
      await settle(tester, () => find.text('Falhas da última coleta').evaluate().isNotEmpty);

      expect(find.textContaining('Erro inesperado'), findsOneWidget);
      expect(find.text('Atualizar'), findsOneWidget);
    });
  });
}

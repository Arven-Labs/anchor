import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/fx_history.dart';
import 'package:anchor_mobile/domain/language_preference.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/language_settings.dart';
import 'package:anchor_mobile/main.dart';
import 'package:anchor_mobile/screens/home_screen.dart';
import 'package:anchor_mobile/services/language_preference_repository.dart';

import 'support/localized_app.dart';

/// Tela mínima para provar o mecanismo sem rede: mostra um texto traduzido, guarda um
/// contador no estado (para provar que trocar o idioma não recria a tela) e troca o idioma.
class _Probe extends StatefulWidget {
  const _Probe();

  @override
  State<_Probe> createState() => _ProbeState();
}

class _ProbeState extends State<_Probe> {
  int taps = 0;

  @override
  Widget build(BuildContext context) {
    final settings = LanguageSettings.of(context);
    return Scaffold(
      body: Column(
        children: [
          Text(AppLocalizations.of(context).navHome, key: const Key('probe-text')),
          Text('$taps', key: const Key('probe-taps')),
          TextButton(
            key: const Key('probe-count'),
            onPressed: () => setState(() => taps++),
            child: const Text('+'),
          ),
          TextButton(
            key: const Key('probe-pt'),
            onPressed: () => settings.onChanged(LanguagePreference.pt),
            child: const Text('pt'),
          ),
          TextButton(
            key: const Key('probe-en'),
            onPressed: () => settings.onChanged(LanguagePreference.en),
            child: const Text('en'),
          ),
          TextButton(
            key: const Key('probe-system'),
            onPressed: () => settings.onChanged(LanguagePreference.system),
            child: const Text('system'),
          ),
        ],
      ),
    );
  }
}

void main() {
  final repository = LanguagePreferenceRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-language-ui-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  // O SQLite ffi responde em tempo real, fora do relógio falso do teste: sonda até a
  // condição valer em vez de esperar um tempo fixo.
  Future<void> settleUntil(WidgetTester tester, bool Function() ready) async {
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump();
      if (ready()) break;
    }
    await tester.pump(const Duration(milliseconds: 300));
  }

  String probeText(WidgetTester tester) =>
      tester.widget<Text>(find.byKey(const Key('probe-text'))).data!;

  Future<void> pumpApp(
    WidgetTester tester, {
    required List<Locale> device,
    Widget home = const _Probe(),
    LanguagePreference? saved,
  }) async {
    tester.platformDispatcher.localesTestValue = device;
    addTearDown(tester.platformDispatcher.clearLocalesTestValue);
    if (saved != null) await tester.runAsync(() => repository.save(saved));
    // Abre o banco antes, para a leitura do app ser rápida e caber na espera abaixo.
    await tester.runAsync(repository.load);
    await tester.pumpWidget(const SizedBox());
    await tester.pumpWidget(AnchorApp(repository: repository, home: home));
    await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 150)));
    await tester.pump(const Duration(milliseconds: 300));
  }

  Future<void> choose(WidgetTester tester, String key) async {
    await tester.tap(find.byKey(Key(key)));
    await settleUntil(tester, () => false);
  }

  group('idioma aplicado ao abrir o app', () {
    testWidgets('aparelho em inglês, nada salvo: inglês', (tester) async {
      await pumpApp(tester, device: const [Locale('en', 'US')]);

      expect(probeText(tester), 'Home');
    });

    testWidgets('aparelho em português, nada salvo: português', (tester) async {
      await pumpApp(tester, device: const [Locale('pt', 'BR')]);

      expect(probeText(tester), 'Início');
    });

    testWidgets('aparelho em inglês, salvo português: português', (tester) async {
      await pumpApp(
        tester,
        device: const [Locale('en', 'US')],
        saved: LanguagePreference.pt,
      );

      expect(probeText(tester), 'Início');
    });

    testWidgets('aparelho em português, salvo inglês: inglês', (tester) async {
      await pumpApp(
        tester,
        device: const [Locale('pt', 'BR')],
        saved: LanguagePreference.en,
      );

      expect(probeText(tester), 'Home');
    });

    testWidgets('aparelho em francês, nada salvo: cai em inglês', (tester) async {
      await pumpApp(tester, device: const [Locale('fr')]);

      expect(probeText(tester), 'Home');
    });
  });

  group('trocar o idioma com o app aberto', () {
    testWidgets('os textos mudam na hora e voltar a "aparelho" segue o aparelho', (
      tester,
    ) async {
      await pumpApp(tester, device: const [Locale('en', 'US')]);

      await choose(tester, 'probe-pt');
      expect(probeText(tester), 'Início');

      await choose(tester, 'probe-system');
      expect(probeText(tester), 'Home');
    });

    testWidgets('a escolha fica salva e vale na próxima abertura', (tester) async {
      await pumpApp(tester, device: const [Locale('en', 'US')]);
      await choose(tester, 'probe-pt');

      await pumpApp(tester, device: const [Locale('en', 'US')]);

      expect(probeText(tester), 'Início');
    });

    testWidgets('o estado da tela é preservado (não volta ao início)', (tester) async {
      await pumpApp(tester, device: const [Locale('en', 'US')]);
      await tester.tap(find.byKey(const Key('probe-count')));
      await tester.tap(find.byKey(const Key('probe-count')));
      await tester.pump();

      await choose(tester, 'probe-pt');

      expect(tester.widget<Text>(find.byKey(const Key('probe-taps'))).data, '2');
    });
  });

  group('engrenagem e tela de Configurações na Início', () {
    Widget home() => HomeScreen(
      fetchFx: (currency) async => FxHistory([(DateTime.now(), 1.0)]),
      loadPositions: () async => [],
    );

    bool has(String key) => find.byKey(Key(key)).evaluate().isNotEmpty;

    testWidgets('sem o app por cima (sem LanguageSettings) a engrenagem não aparece', (
      tester,
    ) async {
      await tester.pumpWidget(localizedApp(home()));
      // a Início lê o layout do SQLite; espera acabar antes de o teste apagar a pasta
      await settleUntil(tester, () => has('home-net-worth-text'));

      expect(has('home-settings'), isFalse);
    });

    testWidgets('abre as 3 opções, com a atual marcada e os idiomas no próprio nome', (
      tester,
    ) async {
      await pumpApp(tester, device: const [Locale('en', 'US')], home: home());
      // a Início lê o SQLite em tempo real: sem esperar, o teste acaba com a leitura em voo
      await settleUntil(tester, () => has('home-net-worth-text'));

      await tester.tap(find.byKey(const Key('home-settings')));
      await tester.pumpAndSettle();

      expect(find.text('Follow device'), findsOneWidget);
      expect(find.text('English'), findsOneWidget);
      expect(find.text('Português'), findsOneWidget);
      final group = tester.widget<RadioGroup<LanguagePreference>>(
        find.byType(RadioGroup<LanguagePreference>),
      );
      expect(group.groupValue, LanguagePreference.system);
    });

    testWidgets('escolher Português traduz a própria tela e persiste', (tester) async {
      await pumpApp(tester, device: const [Locale('en', 'US')], home: home());
      await tester.tap(find.byKey(const Key('home-settings')));
      await tester.pumpAndSettle();

      await choose(tester, 'language-option-pt');

      expect(find.text('Seguir o aparelho'), findsOneWidget);
      final saved = await tester.runAsync(repository.load);
      expect(saved, LanguagePreference.pt);
      final group = tester.widget<RadioGroup<LanguagePreference>>(
        find.byType(RadioGroup<LanguagePreference>),
      );
      expect(group.groupValue, LanguagePreference.pt);
    });

    testWidgets('a engrenagem some no modo de personalizar a Início', (tester) async {
      await pumpApp(tester, device: const [Locale('en', 'US')], home: home());
      // a Início lê o SQLite em tempo real: sem esperar, o teste acaba com a leitura em voo
      await settleUntil(tester, () => has('home-net-worth-text'));
      expect(has('home-settings'), isTrue);

      await tester.tap(find.byKey(const Key('home-customize')));
      await tester.pumpAndSettle();

      expect(has('home-settings'), isFalse);
    });
  });
}

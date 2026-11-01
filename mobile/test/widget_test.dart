import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/main.dart';

// O app escolhe o idioma pelo aparelho (ADR 0001): cada teste fixa o idioma do "aparelho".
void _device(WidgetTester tester, Locale locale) {
  tester.platformDispatcher.localesTestValue = [locale];
  addTearDown(tester.platformDispatcher.clearLocalesTestValue);
}

void main() {
  testWidgets('App abre na aba Início, com o botão de personalizar', (
    WidgetTester tester,
  ) async {
    _device(tester, const Locale('pt', 'BR'));
    await tester.pumpWidget(const AnchorApp());

    expect(find.text('Início'), findsWidgets);
    expect(find.byKey(const Key('home-customize')), findsOneWidget);
    expect(find.byKey(const Key('home-card-total_value')), findsOneWidget);
  });

  testWidgets('A busca de cotação segue a um toque, na aba Cotação', (
    WidgetTester tester,
  ) async {
    _device(tester, const Locale('pt', 'BR'));
    await tester.pumpWidget(const AnchorApp());

    await tester.tap(find.widgetWithText(NavigationDestination, 'Cotação'));
    await tester.pump();

    expect(find.widgetWithText(TextField, 'Ticker (Ação BR)'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Buscar'), findsOneWidget);
  });

  testWidgets('Aparelho em pt: a barra de navegação em português', (tester) async {
    _device(tester, const Locale('pt', 'BR'));
    await tester.pumpWidget(const AnchorApp());

    for (final label in ['Início', 'Cotação', 'Portfolio', 'Finanças', 'Cripto']) {
      expect(find.widgetWithText(NavigationDestination, label), findsOneWidget, reason: label);
    }
  });

  testWidgets('Aparelho em inglês: a barra de navegação e o Início em inglês', (tester) async {
    _device(tester, const Locale('en', 'US'));
    await tester.pumpWidget(const AnchorApp());

    for (final label in ['Home', 'Quote', 'Portfolio', 'Finances', 'Crypto']) {
      expect(find.widgetWithText(NavigationDestination, label), findsOneWidget, reason: label);
    }
    expect(find.text('Total value'), findsOneWidget);
    expect(find.text('Início'), findsNothing);
  });

  testWidgets('Aparelho em outro idioma (francês) cai em inglês', (tester) async {
    _device(tester, const Locale('fr', 'FR'));
    await tester.pumpWidget(const AnchorApp());

    expect(find.widgetWithText(NavigationDestination, 'Home'), findsOneWidget);
    expect(find.widgetWithText(NavigationDestination, 'Início'), findsNothing);
  });
}

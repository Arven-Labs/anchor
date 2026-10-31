import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/main.dart';

void main() {
  testWidgets('App abre na aba Início, com o botão de personalizar', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const AnchorApp());

    expect(find.text('Início'), findsWidgets);
    expect(find.byKey(const Key('home-customize')), findsOneWidget);
    expect(find.byKey(const Key('home-card-total_value')), findsOneWidget);
  });

  testWidgets('A busca de cotação segue a um toque, na aba Cotação', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const AnchorApp());

    await tester.tap(find.widgetWithText(NavigationDestination, 'Cotação'));
    await tester.pump();

    expect(find.widgetWithText(TextField, 'Ticker (Ação BR)'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Buscar'), findsOneWidget);
  });
}

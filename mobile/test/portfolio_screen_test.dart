import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/screens/portfolio_screen.dart';

void main() {
  test('preço atual mostra o número, "indisponível agora" ou "—"', () {
    expect(livePriceLabel(3500.5), '3500.50');
    expect(livePriceLabel(null), '—');
    expect(livePriceLabel(null, unavailable: true), 'indisponível agora');
    // Se há preço, a flag de falha não esconde o número.
    expect(livePriceLabel(10, unavailable: true), '10.00');
  });
}

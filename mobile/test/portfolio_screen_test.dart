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

  test('preço atual leva a moeda do ativo na frente, só quando há número', () {
    expect(livePriceLabel(350000, currency: 'BRL'), 'BRL 350000.00');
    expect(livePriceLabel(70000.5, currency: 'USD'), 'USD 70000.50');
    expect(livePriceLabel(null, currency: 'BRL'), '—');
    expect(livePriceLabel(null, unavailable: true, currency: 'BRL'), 'indisponível agora');
  });
}

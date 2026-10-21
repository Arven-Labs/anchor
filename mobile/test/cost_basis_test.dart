import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/cost_basis.dart';

void main() {
  Matcher near(double expected) => closeTo(expected, 1e-6);

  test('a venda baixa o custo médio e o que sobra mantém o preço médio', () {
    final position = CostPosition()..add(10, 1000);

    final (cost, covered) = position.take(4);

    expect(covered, near(4));
    expect(cost, near(400));
    expect(position.quantity, near(6));
    expect(position.cost, near(600));
  });

  test('vender mais do que há cobre só o que existe', () {
    final position = CostPosition()..add(2, 100);

    final (cost, covered) = position.take(5);

    expect(covered, near(2));
    expect(cost, near(100));
  });

  test('posição zerada recomeça do zero na próxima compra', () {
    final position = CostPosition()..add(2, 100);
    position.take(2);
    position.add(1, 30);

    expect(position.quantity, near(1));
    expect(position.cost, near(30));
  });

  test('sem posição, a venda não cobre nada e não custa nada', () {
    final (cost, covered) = CostPosition().take(3);

    expect(covered, 0);
    expect(cost, 0);
  });
}

import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/home_layout.dart';

void main() {
  HomeWidgetConfig config(
    HomeWidgetKey key, {
    bool visible = true,
    bool wide = false,
  }) => HomeWidgetConfig(key: key, visible: visible, wide: wide);

  List<HomeWidgetKey> keys(List<HomeWidgetConfig> layout) => [
    for (final c in layout) c.key,
  ];

  group('layout padrão', () {
    test('tem os 4 widgets visíveis, na ordem, com alocação e patrimônio em largura cheia', () {
      final layout = defaultHomeLayout();

      expect(keys(layout), [
        HomeWidgetKey.totalValue,
        HomeWidgetKey.profit,
        HomeWidgetKey.allocation,
        HomeWidgetKey.netWorth,
      ]);
      expect(layout.every((c) => c.visible), isTrue);
      expect([for (final c in layout) c.wide], [false, false, true, true]);
    });

    test('cada chamada devolve uma lista nova (editar uma não muda a outra)', () {
      final a = defaultHomeLayout();
      final b = defaultHomeLayout();

      a.removeAt(0);

      expect(b, hasLength(4));
    });
  });

  group('mergeWithDefaults', () {
    test('lista vazia (nunca personalizado) vira o padrão', () {
      expect(keys(mergeWithDefaults([])), keys(defaultHomeLayout()));
    });

    test('o salvo manda na ordem, na visibilidade e na largura', () {
      final merged = mergeWithDefaults([
        config(HomeWidgetKey.netWorth, wide: false),
        config(HomeWidgetKey.totalValue, visible: false),
        config(HomeWidgetKey.profit, wide: true),
        config(HomeWidgetKey.allocation),
      ]);

      expect(keys(merged), [
        HomeWidgetKey.netWorth,
        HomeWidgetKey.totalValue,
        HomeWidgetKey.profit,
        HomeWidgetKey.allocation,
      ]);
      expect(merged[0].wide, isFalse);
      expect(merged[1].visible, isFalse);
      expect(merged[2].wide, isTrue);
    });

    test('widget que o salvo não conhece (versão nova do app) entra no fim com o padrão', () {
      final merged = mergeWithDefaults([
        config(HomeWidgetKey.profit, visible: false),
        config(HomeWidgetKey.totalValue),
      ]);

      expect(keys(merged), [
        HomeWidgetKey.profit,
        HomeWidgetKey.totalValue,
        HomeWidgetKey.allocation,
        HomeWidgetKey.netWorth,
      ]);
      expect(merged[2].visible, isTrue);
      expect(merged[2].wide, isTrue, reason: 'alocação é larga por padrão');
    });

    test('chave repetida no salvo: a primeira vale', () {
      final merged = mergeWithDefaults([
        config(HomeWidgetKey.profit, visible: false),
        config(HomeWidgetKey.profit),
      ]);

      expect(keys(merged).where((k) => k == HomeWidgetKey.profit), hasLength(1));
      expect(merged.first.visible, isFalse);
    });
  });

  group('decodeHomeLayout / encodeHomeLayout', () {
    test('ida e volta preserva ordem, visibilidade e largura', () {
      final original = [
        config(HomeWidgetKey.netWorth, wide: false),
        config(HomeWidgetKey.totalValue, visible: false, wide: true),
        config(HomeWidgetKey.profit),
        config(HomeWidgetKey.allocation),
      ];

      final decoded = decodeHomeLayout(encodeHomeLayout(original));

      expect(keys(decoded), keys(original));
      expect([for (final c in decoded) c.visible], [true, false, true, true]);
      expect([for (final c in decoded) c.wide], [false, true, false, false]);
    });

    test('nulo, vazio e lixo viram lista vazia, sem exceção', () {
      expect(decodeHomeLayout(null), isEmpty);
      expect(decodeHomeLayout(''), isEmpty);
      expect(decodeHomeLayout('isto não é json'), isEmpty);
      expect(decodeHomeLayout('{"a": 1}'), isEmpty, reason: 'objeto, não lista');
      expect(decodeHomeLayout('42'), isEmpty);
    });

    test('entradas ruins são puladas e as boas ficam', () {
      final decoded = decodeHomeLayout(
        '[1, "x", {"key": "inexistente"}, {"visible": true}, '
        '{"key": "profit", "visible": false, "wide": true}, null]',
      );

      expect(keys(decoded), [HomeWidgetKey.profit]);
      expect(decoded.single.visible, isFalse);
      expect(decoded.single.wide, isTrue);
    });

    test('visible e wide com tipo errado caem no padrão do widget', () {
      final decoded = decodeHomeLayout(
        '[{"key": "allocation", "visible": "nao", "wide": 1}]',
      );

      expect(decoded.single.visible, isTrue);
      expect(decoded.single.wide, isTrue, reason: 'alocação é larga por padrão');
    });
  });

  group('reorderHomeLayout (índice final, como no onReorderItem)', () {
    final base = [
      config(HomeWidgetKey.totalValue),
      config(HomeWidgetKey.profit),
      config(HomeWidgetKey.allocation),
      config(HomeWidgetKey.netWorth),
    ];

    test('descer: o item termina no índice pedido', () {
      final result = reorderHomeLayout(base, 0, 2);

      expect(keys(result), [
        HomeWidgetKey.profit,
        HomeWidgetKey.allocation,
        HomeWidgetKey.totalValue,
        HomeWidgetKey.netWorth,
      ]);
    });

    test('subir', () {
      final result = reorderHomeLayout(base, 3, 0);

      expect(keys(result), [
        HomeWidgetKey.netWorth,
        HomeWidgetKey.totalValue,
        HomeWidgetKey.profit,
        HomeWidgetKey.allocation,
      ]);
    });

    test('soltar no mesmo lugar não muda nada, e a lista original fica intacta', () {
      expect(keys(reorderHomeLayout(base, 1, 1)), keys(base));
      reorderHomeLayout(base, 0, 2);
      expect(keys(base).first, HomeWidgetKey.totalValue);
    });

    test('trocar vizinhos', () {
      final result = reorderHomeLayout(base, 1, 2);

      expect(keys(result), [
        HomeWidgetKey.totalValue,
        HomeWidgetKey.allocation,
        HomeWidgetKey.profit,
        HomeWidgetKey.netWorth,
      ]);
    });

    test('mover para o fim', () {
      final result = reorderHomeLayout(base, 1, 3);

      expect(keys(result).last, HomeWidgetKey.profit);
    });
  });

  group('homeRows', () {
    test('duas meias larguras seguidas dividem a linha, a larga ocupa a sua', () {
      final rows = homeRows(defaultHomeLayout());

      expect([for (final r in rows) keys(r)], [
        [HomeWidgetKey.totalValue, HomeWidgetKey.profit],
        [HomeWidgetKey.allocation],
        [HomeWidgetKey.netWorth],
      ]);
    });

    test('escondido não aparece e não ocupa lugar na linha', () {
      final rows = homeRows([
        config(HomeWidgetKey.totalValue),
        config(HomeWidgetKey.profit, visible: false),
        config(HomeWidgetKey.allocation, wide: true),
        config(HomeWidgetKey.netWorth, wide: true),
      ]);

      expect([for (final r in rows) keys(r)], [
        [HomeWidgetKey.totalValue],
        [HomeWidgetKey.allocation],
        [HomeWidgetKey.netWorth],
      ]);
    });

    test('três meias larguras: uma linha de duas e uma de uma', () {
      final rows = homeRows([
        config(HomeWidgetKey.totalValue),
        config(HomeWidgetKey.profit),
        config(HomeWidgetKey.netWorth),
      ]);

      expect([for (final r in rows) r.length], [2, 1]);
    });

    test('meia largura entre duas largas fica sozinha', () {
      final rows = homeRows([
        config(HomeWidgetKey.allocation, wide: true),
        config(HomeWidgetKey.totalValue),
        config(HomeWidgetKey.netWorth, wide: true),
      ]);

      expect([for (final r in rows) r.length], [1, 1, 1]);
    });

    test('tudo escondido: nenhuma linha', () {
      expect(
        homeRows([
          for (final k in HomeWidgetKey.values) config(k, visible: false),
        ]),
        isEmpty,
      );
    });
  });
}

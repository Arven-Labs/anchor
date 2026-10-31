// Layout da aba Início: quais cards aparecem, em que ordem e com que largura (meia ou
// cheia). Espelha a Home do desktop (`desktop/src/home/HomeSection.tsx`): os padrões
// moram no código e o que o dono personaliza é só uma lista salva, mesclada com os
// padrões ao ler. Tudo aqui é puro; a gravação fica em `HomeLayoutRepository`.

import 'dart:convert';

enum HomeWidgetKey {
  totalValue('total_value', 'Valor total', wideByDefault: false),
  profit('profit', 'Lucro não realizado', wideByDefault: false),
  allocation('allocation', 'Alocação por classe', wideByDefault: true),
  netWorth('net_worth', 'Patrimônio líquido', wideByDefault: true);

  /// Identificador gravado no JSON e usado nas chaves de teste; não muda nunca.
  final String id;
  final String label;
  final bool wideByDefault;

  const HomeWidgetKey(this.id, this.label, {required this.wideByDefault});

  static HomeWidgetKey? tryParse(Object? id) {
    for (final key in HomeWidgetKey.values) {
      if (key.id == id) return key;
    }
    return null;
  }
}

class HomeWidgetConfig {
  final HomeWidgetKey key;
  final bool visible;

  /// `true` ocupa a linha inteira; `false` divide a linha com o vizinho.
  final bool wide;

  const HomeWidgetConfig({
    required this.key,
    required this.visible,
    required this.wide,
  });

  HomeWidgetConfig copyWith({bool? visible, bool? wide}) => HomeWidgetConfig(
    key: key,
    visible: visible ?? this.visible,
    wide: wide ?? this.wide,
  );

  Map<String, Object?> toJson() => {
    'key': key.id,
    'visible': visible,
    'wide': wide,
  };
}

HomeWidgetConfig _defaultFor(HomeWidgetKey key) =>
    HomeWidgetConfig(key: key, visible: true, wide: key.wideByDefault);

/// Todos os widgets visíveis, na ordem do enum. Lista nova a cada chamada.
List<HomeWidgetConfig> defaultHomeLayout() => [
  for (final key in HomeWidgetKey.values) _defaultFor(key),
];

/// O salvo manda na ordem, na visibilidade e na largura. Widget que o código
/// conhece e o salvo não (uma versão nova do app) entra no fim com o padrão;
/// chave repetida no salvo vale a primeira. Lista vazia vira o padrão.
List<HomeWidgetConfig> mergeWithDefaults(List<HomeWidgetConfig> saved) {
  final seen = <HomeWidgetKey>{};
  final merged = <HomeWidgetConfig>[];
  for (final config in saved) {
    if (seen.add(config.key)) merged.add(config);
  }
  for (final key in HomeWidgetKey.values) {
    if (seen.add(key)) merged.add(_defaultFor(key));
  }
  return merged;
}

String encodeHomeLayout(List<HomeWidgetConfig> layout) =>
    jsonEncode([for (final config in layout) config.toJson()]);

/// Lê o JSON gravado sem nunca lançar: nulo, lixo ou um formato que não é lista
/// viram lista vazia; entrada sem chave conhecida é pulada; `visible` e `wide` com
/// tipo errado caem no padrão do widget.
List<HomeWidgetConfig> decodeHomeLayout(String? source) {
  if (source == null || source.isEmpty) return [];
  final Object? decoded;
  try {
    decoded = jsonDecode(source);
  } on FormatException {
    return [];
  }
  if (decoded is! List) return [];
  final layout = <HomeWidgetConfig>[];
  for (final entry in decoded) {
    if (entry is! Map) continue;
    final key = HomeWidgetKey.tryParse(entry['key']);
    if (key == null) continue;
    final visible = entry['visible'];
    final wide = entry['wide'];
    layout.add(
      HomeWidgetConfig(
        key: key,
        visible: visible is bool ? visible : true,
        wide: wide is bool ? wide : key.wideByDefault,
      ),
    );
  }
  return layout;
}

/// Move o item de `oldIndex` para a posição `newIndex`, com a convenção do
/// `onReorderItem` do `ReorderableListView`: `newIndex` já é o índice final, depois de
/// tirar o item do lugar de origem. Devolve uma lista nova.
List<HomeWidgetConfig> reorderHomeLayout(
  List<HomeWidgetConfig> layout,
  int oldIndex,
  int newIndex,
) {
  final result = [...layout];
  result.insert(newIndex, result.removeAt(oldIndex));
  return result;
}

/// As linhas da tela: só os visíveis, cada cheia sozinha e duas meias seguidas
/// juntas. Uma meia sem par (vizinha cheia ou fim da lista) fica sozinha.
List<List<HomeWidgetConfig>> homeRows(List<HomeWidgetConfig> layout) {
  final visible = [
    for (final config in layout)
      if (config.visible) config,
  ];
  final rows = <List<HomeWidgetConfig>>[];
  var i = 0;
  while (i < visible.length) {
    final current = visible[i];
    if (!current.wide && i + 1 < visible.length && !visible[i + 1].wide) {
      rows.add([current, visible[i + 1]]);
      i += 2;
    } else {
      rows.add([current]);
      i += 1;
    }
  }
  return rows;
}

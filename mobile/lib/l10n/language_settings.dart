import 'package:flutter/widgets.dart';

import '../domain/language_preference.dart';

/// O idioma escolhido e quem o troca, fornecidos pelo `AnchorApp` acima do `MaterialApp`
/// (assim as rotas empilhadas, como a tela de Configurações, também o enxergam).
class LanguageSettings extends InheritedWidget {
  final LanguagePreference preference;
  final ValueChanged<LanguagePreference> onChanged;

  const LanguageSettings({
    super.key,
    required this.preference,
    required this.onChanged,
    required super.child,
  });

  /// `null` fora do app (testes de tela soltos): quem usa decide se esconde a opção.
  static LanguageSettings? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<LanguageSettings>();

  static LanguageSettings of(BuildContext context) {
    final settings = maybeOf(context);
    assert(settings != null, 'LanguageSettings ausente na árvore');
    return settings!;
  }

  @override
  bool updateShouldNotify(LanguageSettings oldWidget) =>
      preference != oldWidget.preference;
}

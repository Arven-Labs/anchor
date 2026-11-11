import 'package:flutter/widgets.dart' show LocalizationsDelegate;
import 'package:material_ui/material_ui.dart' show GlobalMaterialLocalizations;

import 'app_localizations.dart';

/// Os delegates de localização do `MaterialApp`: os textos do app e os do Material, Cupertino e
/// Widgets (todos do `material_ui`).
///
/// Não use `AppLocalizations.localizationsDelegates`: essa lista é gerada pelo `gen-l10n` do
/// Flutter 3.50 com o `GlobalMaterialLocalizations` antigo (de `flutter_localizations`), e os
/// widgets do `material_ui` procuram o `MaterialLocalizations` do próprio `material_ui`. Com a
/// lista gerada, toda tela quebra com "A MaterialLocalizations delegate that supports the pt
/// locale was not found". Voltar a usá-la quando o gerador usar o `material_ui`.
const appLocalizationsDelegates = <LocalizationsDelegate<dynamic>>[
  AppLocalizations.delegate,
  ...GlobalMaterialLocalizations.delegates,
];

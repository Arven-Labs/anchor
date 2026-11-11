import 'package:material_ui/material_ui.dart';

import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/theme.dart';

/// `MaterialApp` dos testes de tela com o idioma fixado (ADR 0001). `pt` mantém os textos
/// que os testes já afirmavam; `en` cobre a tradução. O padrão é `pt` só porque as
/// asserções existentes são em português: o app em si usa o idioma do aparelho.
Widget localizedApp(Widget home, {String locale = 'pt', ThemeData? theme}) => MaterialApp(
  locale: Locale(locale),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  theme: theme ?? buildAnchorTheme(),
  home: home,
);

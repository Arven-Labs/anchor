import 'package:material_ui/material_ui.dart';

/// Mesma paleta usada no desktop (`desktop/src/index.css`, tema dark): verde
/// `#4ade80` sobre fundo navy `#0b0f14` — a mesma identidade visual do
/// escudo/ícone da marca Anchor.
const anchorPrimary = Color(0xFF4ADE80);
const anchorBackground = Color(0xFF0B0F14);
const anchorSurface = Color(0xFF141A21);

ThemeData buildAnchorTheme() {
  final colorScheme = ColorScheme.fromSeed(
    seedColor: anchorPrimary,
    brightness: Brightness.dark,
    primary: anchorPrimary,
    onPrimary: anchorBackground,
    surface: anchorSurface,
  );

  return ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    colorScheme: colorScheme,
    scaffoldBackgroundColor: anchorBackground,
    appBarTheme: const AppBarTheme(
      backgroundColor: anchorBackground,
      foregroundColor: Colors.white,
      elevation: 0,
    ),
    cardTheme: const CardThemeData(color: anchorSurface, elevation: 0),
    inputDecorationTheme: const InputDecorationTheme(
      filled: true,
      fillColor: anchorSurface,
      border: OutlineInputBorder(
        borderRadius: BorderRadius.all(Radius.circular(8)),
        borderSide: BorderSide.none,
      ),
    ),
  );
}

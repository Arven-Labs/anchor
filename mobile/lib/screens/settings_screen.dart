import 'package:flutter/material.dart';

import '../domain/language_preference.dart';
import '../l10n/app_localizations.dart';
import '../l10n/language_settings.dart';

/// Configurações do app. Hoje só o idioma.
class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final settings = LanguageSettings.of(context);
    // Cada idioma aparece no próprio nome (endônimo), igual nos dois idiomas do app.
    final names = {
      LanguagePreference.system: l.settingsLanguageSystem,
      LanguagePreference.en: 'English',
      LanguagePreference.pt: 'Português',
    };
    return Scaffold(
      appBar: AppBar(title: Text(l.settingsTitle)),
      body: ListView(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Text(
              l.settingsLanguage,
              style: Theme.of(context).textTheme.titleSmall,
            ),
          ),
          RadioGroup<LanguagePreference>(
            groupValue: settings.preference,
            onChanged: (value) {
              if (value != null) settings.onChanged(value);
            },
            child: Column(
              children: [
                for (final entry in names.entries)
                  RadioListTile<LanguagePreference>(
                    key: Key('language-option-${entry.key.code}'),
                    value: entry.key,
                    title: Text(entry.value),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

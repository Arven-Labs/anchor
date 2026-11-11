import 'package:material_ui/material_ui.dart';

import 'l10n/app_delegates.dart';
import 'l10n/app_localizations.dart';
import 'domain/language_preference.dart';
import 'l10n/language_settings.dart';
import 'l10n/locale_resolution.dart';
import 'services/language_preference_repository.dart';
import 'screens/crypto_score_screen.dart';
import 'screens/financas_screen.dart';
import 'screens/home_screen.dart';
import 'screens/portfolio_screen.dart';
import 'screens/quote_search_screen.dart';
import 'theme.dart';

void main() {
  runApp(const AnchorApp());
}

class AnchorApp extends StatefulWidget {
  /// Onde o idioma escolhido fica guardado; injetável para teste.
  final LanguagePreferenceRepository? repository;

  /// Tela inicial; o padrão é a navegação por abas. Injetável para teste, sem rede.
  final Widget? home;

  const AnchorApp({super.key, this.repository, this.home});

  @override
  State<AnchorApp> createState() => _AnchorAppState();
}

class _AnchorAppState extends State<AnchorApp> {
  late final LanguagePreferenceRepository _repository =
      widget.repository ?? LanguagePreferenceRepository();

  // Enquanto o banco não responde, segue o aparelho (sem tela de carregamento).
  LanguagePreference _preference = LanguagePreference.system;

  @override
  void initState() {
    super.initState();
    _loadPreference();
  }

  Future<void> _loadPreference() async {
    final saved = await _repository.load();
    if (mounted) setState(() => _preference = saved);
  }

  void _changePreference(LanguagePreference preference) {
    setState(() => _preference = preference);
    _repository.save(preference);
  }

  @override
  Widget build(BuildContext context) {
    return LanguageSettings(
      preference: _preference,
      onChanged: _changePreference,
      child: MaterialApp(
        title: 'Anchor',
        theme: buildAnchorTheme(),
        // A escolha do usuário; `null` (seguir o aparelho) deixa o callback abaixo
        // decidir: o que não for português cai em inglês (ADR 0001).
        locale: _preference.locale,
        localizationsDelegates: appLocalizationsDelegates,
        supportedLocales: appSupportedLocales,
        localeListResolutionCallback: resolveAppLocale,
        home: widget.home ?? const AnchorHome(),
        debugShowCheckedModeBanner: false,
      ),
    );
  }
}

class AnchorHome extends StatefulWidget {
  const AnchorHome({super.key});

  @override
  State<AnchorHome> createState() => _AnchorHomeState();
}

class _AnchorHomeState extends State<AnchorHome> {
  int _tabIndex = 0;

  /// Posição da aba "Cripto": só ela coleta sozinha, e só quando está à vista.
  static const _cryptoTab = 4;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: IndexedStack(
        index: _tabIndex,
        children: [
          const HomeScreen(),
          const QuoteSearchScreen(),
          const PortfolioScreen(),
          const FinancasScreen(),
          CryptoScoreScreen(active: _tabIndex == _cryptoTab),
        ],
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _tabIndex,
        onDestinationSelected: (index) => setState(() => _tabIndex = index),
        destinations: [
          NavigationDestination(
            icon: const Icon(Icons.home_outlined),
            label: AppLocalizations.of(context).navHome,
          ),
          NavigationDestination(
            icon: const Icon(Icons.search),
            label: AppLocalizations.of(context).navQuote,
          ),
          NavigationDestination(
            icon: const Icon(Icons.pie_chart),
            label: AppLocalizations.of(context).navPortfolio,
          ),
          NavigationDestination(
            icon: const Icon(Icons.account_balance_wallet_outlined),
            label: AppLocalizations.of(context).navFinance,
          ),
          NavigationDestination(
            icon: const Icon(Icons.currency_bitcoin),
            label: AppLocalizations.of(context).navCrypto,
          ),
        ],
      ),
    );
  }
}

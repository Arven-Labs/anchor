import 'package:flutter/material.dart';

import 'l10n/app_localizations.dart';
import 'l10n/locale_resolution.dart';
import 'screens/crypto_score_screen.dart';
import 'screens/financas_screen.dart';
import 'screens/home_screen.dart';
import 'screens/portfolio_screen.dart';
import 'screens/quote_search_screen.dart';
import 'theme.dart';

void main() {
  runApp(const AnchorApp());
}

class AnchorApp extends StatelessWidget {
  const AnchorApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Anchor',
      theme: buildAnchorTheme(),
      // Idioma do aparelho; o que não for português cai em inglês (ADR 0001).
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: appSupportedLocales,
      localeListResolutionCallback: resolveAppLocale,
      home: const AnchorHome(),
      debugShowCheckedModeBanner: false,
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

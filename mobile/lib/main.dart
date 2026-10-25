import 'package:flutter/material.dart';

import 'screens/financas_screen.dart';
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

  static const _screens = [
    QuoteSearchScreen(),
    PortfolioScreen(),
    FinancasScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: IndexedStack(index: _tabIndex, children: _screens),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _tabIndex,
        onDestinationSelected: (index) => setState(() => _tabIndex = index),
        destinations: const [
          NavigationDestination(icon: Icon(Icons.search), label: 'Cotação'),
          NavigationDestination(
            icon: Icon(Icons.pie_chart),
            label: 'Portfolio',
          ),
          NavigationDestination(
            icon: Icon(Icons.account_balance_wallet_outlined),
            label: 'Finanças',
          ),
        ],
      ),
    );
  }
}

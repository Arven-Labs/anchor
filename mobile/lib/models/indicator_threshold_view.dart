/// Limites de um indicador como a tela do score os mostra — espelha
/// `IndicatorThresholdView` do desktop (`commands/crypto_indicator.rs`).
/// `coin` é a moeda dona dos limites (ETH, BTC, ADA, XRP ou BNB); as demais
/// usam os do ETH. `defaultGreen`/`defaultRed` são nulos se a moeda não tem o
/// indicador de fábrica.
class IndicatorThresholdView {
  final String coin;
  final String indicator;
  final double greenBoundary;
  final double redBoundary;
  final double? defaultGreen;
  final double? defaultRed;
  final bool isDefault;
  final bool higherIsBetter;
  final DateTime updatedAt;

  const IndicatorThresholdView({
    required this.coin,
    required this.indicator,
    required this.greenBoundary,
    required this.redBoundary,
    required this.defaultGreen,
    required this.defaultRed,
    required this.isDefault,
    required this.higherIsBetter,
    required this.updatedAt,
  });
}

/// As 17 moedas que o app sabe cotar — mesma lista de
/// `desktop/src-tauri/src/finance_core/fx.rs::CURRENCIES` (moedas de valor
/// unitário muito baixo, como VND, ficam de fora: o Yahoo arredonda a
/// cotação para zero).
///
/// Só os códigos: o nome de cada moeda vem do idioma da tela (`currencyName`, em
/// `l10n/finance_labels.dart`).
const currencyCatalog = <String>[
  'USD',
  'EUR',
  'GBP',
  'JPY',
  'CHF',
  'CAD',
  'AUD',
  'NZD',
  'CNY',
  'BRL',
  'ARS',
  'UYU',
  'PYG',
  'CLP',
  'COP',
  'PEN',
  'MXN',
];

/// Moeda padrão do app e base quando nada foi escolhido.
const defaultBaseCurrency = 'BRL';

/// `code` sem espaços e em maiúsculas, ou `null` se não está no catálogo
/// (mesma regra de `validate_currency` do desktop).
String? validateCurrency(String code) {
  final normalized = code.trim().toUpperCase();
  return currencyCatalog.contains(normalized) ? normalized : null;
}

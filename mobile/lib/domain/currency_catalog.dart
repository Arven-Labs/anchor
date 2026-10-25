/// As 17 moedas que o app sabe cotar — mesma lista de
/// `desktop/src-tauri/src/finance_core/fx.rs::CURRENCIES` (moedas de valor
/// unitário muito baixo, como VND, ficam de fora: o Yahoo arredonda a
/// cotação para zero).
const currencyCatalog = <(String, String)>[
  ('USD', 'Dólar americano'),
  ('EUR', 'Euro'),
  ('GBP', 'Libra esterlina'),
  ('JPY', 'Iene'),
  ('CHF', 'Franco suíço'),
  ('CAD', 'Dólar canadense'),
  ('AUD', 'Dólar australiano'),
  ('NZD', 'Dólar neozelandês'),
  ('CNY', 'Yuan'),
  ('BRL', 'Real'),
  ('ARS', 'Peso argentino'),
  ('UYU', 'Peso uruguaio'),
  ('PYG', 'Guarani'),
  ('CLP', 'Peso chileno'),
  ('COP', 'Peso colombiano'),
  ('PEN', 'Sol peruano'),
  ('MXN', 'Peso mexicano'),
];

/// Moeda padrão do app e base quando nada foi escolhido.
const defaultBaseCurrency = 'BRL';

/// `code` sem espaços e em maiúsculas, ou `null` se não está no catálogo
/// (mesma regra de `validate_currency` do desktop).
String? validateCurrency(String code) {
  final normalized = code.trim().toUpperCase();
  return currencyCatalog.any((c) => c.$1 == normalized) ? normalized : null;
}

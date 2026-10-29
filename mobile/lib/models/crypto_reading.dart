/// Leitura de um indicador do score cripto — espelha
/// `desktop/src-tauri/src/entity/crypto_indicators.rs`. `signal` é `GREEN`,
/// `NEUTRAL` ou `RED` (`Signal.wire`), já classificado pelos limites vigentes.
class CryptoReading {
  final int id;
  final String coin;
  final String indicator;

  /// `YYYY-MM-DD`: a data do dado, não a da coleta.
  final String readingDate;
  final double rawValue;
  final String signal;
  final String source;
  final DateTime createdAt;

  const CryptoReading({
    required this.id,
    required this.coin,
    required this.indicator,
    required this.readingDate,
    required this.rawValue,
    required this.signal,
    required this.source,
    required this.createdAt,
  });

  factory CryptoReading.fromMap(Map<String, Object?> map) {
    return CryptoReading(
      id: map['id'] as int,
      coin: map['coin'] as String,
      indicator: map['indicator'] as String,
      readingDate: map['reading_date'] as String,
      rawValue: (map['raw_value'] as num).toDouble(),
      signal: map['signal'] as String,
      source: map['source'] as String,
      createdAt: DateTime.parse(map['created_at'] as String),
    );
  }
}

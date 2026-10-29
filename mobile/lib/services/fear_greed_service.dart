import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';

const _source = 'alternative.me Fear & Greed';

/// Último segundo que o `DateTime` representa com folga (ano 9999); acima disso
/// o `timestamp * 1000` poderia estourar o inteiro em silêncio.
const _maxTimestampSeconds = 253402300799;

/// Leitura do índice. `readingDate` é o dia UTC do `timestamp` da fonte
/// (`YYYY-MM-DD`), não o dia em que o app buscou.
typedef FearGreed = ({int value, String classification, String readingDate});

CryptoSourceException _unexpected() =>
    const CryptoSourceException(_source, 'unexpected response');

String _utcDay(int seconds) {
  final date = DateTime.fromMillisecondsSinceEpoch(seconds * 1000, isUtc: true);
  String pad(int n, int width) => n.toString().padLeft(width, '0');
  return '${pad(date.year, 4)}-${pad(date.month, 2)}-${pad(date.day, 2)}';
}

/// Leitura mais recente: a primeira entrada de `data`, com tudo como texto.
/// O valor precisa ser inteiro de 0 a 100; fora disso a fonte provavelmente
/// mudou de escala, e é melhor falhar do que classificar um número sem sentido.
FearGreed parseFearGreedLatest(String body) {
  final json = decodeSourceJson(_source, body);
  final data = json is Map ? json['data'] : null;
  if (data is! List || data.isEmpty) throw _unexpected();
  final entry = data.first;
  if (entry is! Map) throw _unexpected();

  final rawValue = entry['value'];
  final classification = entry['value_classification'];
  final rawTimestamp = entry['timestamp'];
  if (rawValue is! String ||
      classification is! String ||
      rawTimestamp is! String) {
    throw _unexpected();
  }
  final value = int.tryParse(rawValue);
  if (value == null || value < 0 || value > 100) {
    throw CryptoSourceException(_source, "invalid value '$rawValue'");
  }
  final timestamp = int.tryParse(rawTimestamp);
  if (timestamp == null || timestamp < 0 || timestamp > _maxTimestampSeconds) {
    throw CryptoSourceException(_source, "invalid timestamp '$rawTimestamp'");
  }
  return (
    value: value,
    classification: classification,
    readingDate: _utcDay(timestamp),
  );
}

/// Crypto Fear & Greed Index (alternative.me, público e sem chave). É um índice
/// do mercado todo, o mesmo para qualquer moeda do score.
class FearGreedService {
  final http.Client _client;

  FearGreedService({http.Client? client}) : _client = client ?? http.Client();

  Future<FearGreed> latest() async {
    final body = await cryptoGet(
      _client,
      Uri.https('api.alternative.me', '/fng/', {'limit': '1'}),
      source: _source,
    );
    return parseFearGreedLatest(body);
  }
}

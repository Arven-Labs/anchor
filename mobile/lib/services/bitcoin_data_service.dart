import 'package:http/http.dart' as http;

import '../domain/crypto_score.dart' show isValidReadingDate;
import 'crypto_source_http.dart';

const _source = 'bitcoin-data.com';

/// Faixa em que o valor é plausível; fora dela a fonte provavelmente mudou de
/// formato. SOPR é uma razão de preços (0 exclusivo) e NUPL fica em [-1, 1].
const _ranges = {'sopr': (0.0, 10.0), 'nupl': (-1.0, 1.0)};

/// Idade máxima do dado. O plano grátis documenta 7 dias de atraso e hoje (medido
/// em 2026-10-31) o dado vem com 1 dia; 10 cobre os dois e recusa a fonte parada.
const _maxAgeDays = 10;

CryptoSourceException _unexpected([String detail = 'unexpected response']) =>
    CryptoSourceException(_source, detail);

/// Valor e data do dado (`YYYY-MM-DD`); a data não é a da coleta.
typedef DatedValue = ({double value, String date});

/// Resposta de `/v1/{field}/last`, ex.:
/// `{"d":"2026-10-30","unixTs":1793318400,"sopr":0.9933}` (o formato antigo trazia
/// também `delayed` e `message`). [field] é `sopr` ou `nupl`; outro é erro de uso
/// ([ArgumentError]). Data ausente, ilegível, do futuro ou com mais de 10 dias, e
/// valor ausente, não numérico ou fora da faixa, são erro da fonte.
DatedValue parseBitcoinData(
  String body,
  String field, {
  required DateTime today,
}) {
  final range = _ranges[field];
  if (range == null) {
    throw ArgumentError('bitcoin-data.com só atende sopr e nupl, não $field');
  }
  final json = decodeSourceJson(_source, body);
  if (json is! Map) throw _unexpected();

  final value = json[field];
  if (value is! num || !value.isFinite) {
    throw _unexpected("missing numeric '$field'");
  }
  final (min, max) = range;
  if (value < min || value > max || (field == 'sopr' && value == min)) {
    throw _unexpected('implausible $field $value');
  }

  final date = json['d'];
  if (date is! String || !isValidReadingDate(date)) {
    throw _unexpected("missing or invalid 'd' date");
  }
  final utc = today.toUtc();
  final age = DateTime.utc(
    utc.year,
    utc.month,
    utc.day,
  ).difference(DateTime.parse('${date}T00:00:00Z')).inDays;
  if (age < 0) throw _unexpected('data from the future: $date');
  if (age > _maxAgeDays) {
    throw _unexpected('stale data, latest value is $date ($age days old)');
  }
  return (value: value.toDouble(), date: date);
}

/// SOPR e NUPL do BTC no bitcoin-data.com (público, sem chave). **Cota do plano
/// grátis: 10 chamadas por hora e 15 por dia, por IP** (o desktop usa a mesma):
/// quem chama é o coletor, que só vem aqui depois de checar que não há leitura de
/// hoje.
class BitcoinDataService {
  final http.Client _client;
  final DateTime Function() _now;

  /// [now] é o relógio (injetável nos testes) da checagem de dado velho.
  BitcoinDataService({http.Client? client, DateTime Function()? now})
    : _client = client ?? http.Client(),
      _now = now ?? DateTime.now;

  Future<DatedValue> sopr() => _fetch('sopr');

  Future<DatedValue> nupl() => _fetch('nupl');

  Future<DatedValue> _fetch(String field) async => parseBitcoinData(
    await cryptoGet(
      _client,
      Uri.https('bitcoin-data.com', '/v1/$field/last'),
      source: _source,
    ),
    field,
    today: _now(),
  );
}

import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';

const _source = 'DefiLlama yields';

/// Pool stETH (Lido, Ethereum) na DefiLlama; o id é estável. Conferido em
/// 2026-10-31 em `yields.llama.fi/pools` (projeto `lido`, símbolo `STETH`).
const _stethPoolId = '747c1d2a-c668-4682-b9f9-296708a3dd90';

/// Quantos pontos da série entram na mediana. O Lido reportou 15,077 % num único
/// dia (2026-10-30; a média de 7 dias dele é 4,16 %) enquanto o normal é ~2,25 %:
/// o último ponto sozinho, como o desktop usa, gravaria um pico como se fosse o
/// yield. A mediana de 7 aguenta até 3 pontos ruins.
const _window = 7;

/// Idade máxima do último ponto. O normal é menos de 1 dia.
const _maxAgeDays = 7;

CryptoSourceException _unexpected([String detail = 'unexpected response']) =>
    CryptoSourceException(_source, detail);

/// Mediana do `apy` (em %) dos 7 últimos pontos da série do pool, do mais antigo
/// ao mais novo. Série com menos de 7 pontos, `apy` nulo, texto ou negativo em
/// qualquer um dos 7, ou último ponto velho demais é erro (em 1.643 pontos reais
/// não há nenhum `apy` nulo: se aparecer, a série não é confiável).
double parseStethApy(String body, {required DateTime today}) {
  final json = decodeSourceJson(_source, body);
  final data = json is Map ? json['data'] : null;
  if (data is! List) throw _unexpected();
  if (data.length < _window) throw _unexpected('not enough history');

  final apys = <double>[];
  String? lastDay;
  for (final point in data.sublist(data.length - _window)) {
    final apy = point is Map ? point['apy'] : null;
    final time = point is Map ? point['timestamp'] : null;
    if (apy is! num || !apy.isFinite || apy < 0) throw _unexpected();
    if (time is! String || time.length < 10) throw _unexpected();
    apys.add(apy.toDouble());
    lastDay = time.substring(0, 10);
  }

  final date = DateTime.tryParse('${lastDay}T00:00:00Z');
  if (date == null) throw _unexpected("last point has no valid 'timestamp'");
  final utc = today.toUtc();
  final age = DateTime.utc(utc.year, utc.month, utc.day).difference(date).inDays;
  if (age > _maxAgeDays) {
    throw _unexpected('stale data, latest point is $lastDay ($age days old)');
  }

  apys.sort();
  return apys[_window ~/ 2];
}

/// Yield real do staking: APY nominal menos a inflação bruta anual do ETH, tudo
/// em %. Pode ser negativo (APY abaixo da inflação), e isso é um valor válido.
double realStakingYield(double apyPct, double grossInflationPct) =>
    apyPct - grossInflationPct;

/// APY do stETH na DefiLlama yields (público, sem chave).
class DefiLlamaYieldsService {
  final http.Client _client;
  final DateTime Function() _now;

  /// [now] é o relógio (injetável nos testes) da checagem de dado velho.
  DefiLlamaYieldsService({http.Client? client, DateTime Function()? now})
    : _client = client ?? http.Client(),
      _now = now ?? DateTime.now;

  /// Mediana do APY dos 7 últimos pontos do pool stETH, em %. A resposta traz a
  /// série inteira (~254 KB, desde 2022).
  Future<double> stethApy() async => parseStethApy(
    await cryptoGet(
      _client,
      Uri.https('yields.llama.fi', '/chart/$_stethPoolId'),
      source: _source,
    ),
    today: _now(),
  );
}

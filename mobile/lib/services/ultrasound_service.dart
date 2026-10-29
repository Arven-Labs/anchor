import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';

const _supplySource = 'ultrasound.money supply';
const _burnSource = 'ultrasound.money burn';
const _source = 'ultrasound.money';

const _windowDays = 30.0;
const _daysInYear = 365.0;

CryptoSourceException _unexpected(String source) =>
    CryptoSourceException(source, 'unexpected response');

/// Primeira e última oferta da janela `d30` (de hora em hora, ~30 dias). Janela
/// com menos de 2 pontos não é uma variação de 30 dias, e oferta que não seja
/// número finito positivo é dado inválido.
(double, double) _supplyWindow(String supplyBody) {
  final json = decodeSourceJson(_supplySource, supplyBody);
  final window = json is Map ? json['d30'] : null;
  if (window is! List) throw _unexpected(_supplySource);
  if (window.length < 2) {
    throw const CryptoSourceException(
      _supplySource,
      'd30 window needs 2 points',
    );
  }
  double supplyOf(Object? point) {
    final supply = point is Map ? point['supply'] : null;
    if (supply is! num || !supply.isFinite || supply <= 0) {
      throw _unexpected(_supplySource);
    }
    return supply.toDouble();
  }

  return (supplyOf(window.first), supplyOf(window.last));
}

/// Variação % da oferta de ETH em 30 dias, anualizada. Positivo: oferta
/// crescendo (emissão > queima); negativo: "ultra sound money".
double parseNetIssuanceAnnualizedPct(String supplyBody) {
  final (start, end) = _supplyWindow(supplyBody);
  return (end - start) / start * 100 * (_daysInYear / _windowDays);
}

/// Queima (taxas) ÷ emissão bruta de ETH em 30 dias. A API só dá a oferta
/// líquida (emissão − queima) e a queima, então emissão = variação líquida +
/// queima. Emissão bruta não positiva é incoerente e é recusada.
double parseFeesVsEmissionRatio(String supplyBody, String burnBody) {
  final (start, end) = _supplyWindow(supplyBody);
  final json = decodeSourceJson(_burnSource, burnBody);
  final window = json is Map ? json['d30'] : null;
  final sum = window is Map ? window['sum'] : null;
  final burn = sum is Map ? sum['eth'] : null;
  if (burn is! num || !burn.isFinite || burn < 0) {
    throw _unexpected(_burnSource);
  }
  final gross = (end - start) + burn;
  if (gross <= 0) {
    throw const CryptoSourceException(_source, 'non-positive gross issuance');
  }
  return burn / gross;
}

/// Oferta e queima de ETH no ultrasound.money (público, sem chave). O serviço só
/// busca: a oferta (431 KB, quase tudo histórico descartado) serve aos dois
/// indicadores, então quem os calcula a baixa uma vez e passa o texto aos
/// parsers.
class UltrasoundService {
  static const _host = 'ultrasound.money';

  final http.Client _client;

  UltrasoundService({http.Client? client}) : _client = client ?? http.Client();

  /// Corpo de `supply-over-time`.
  Future<String> fetchSupply() => cryptoGet(
    _client,
    Uri.https(_host, '/api/v2/fees/supply-over-time'),
    source: _supplySource,
  );

  /// Corpo de `burn-sums`.
  Future<String> fetchBurn() => cryptoGet(
    _client,
    Uri.https(_host, '/api/v2/fees/burn-sums'),
    source: _burnSource,
  );
}

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'quote_exceptions.dart';
import 'quote_http.dart';

/// Uma fonte do score cripto não deu um valor utilizável: rede, limite, HTTP
/// diferente de 200 ou resposta fora do formato esperado. A mensagem sempre
/// começa pelo nome da fonte, porque a tela mostra uma linha por indicador que
/// falhou (como o desktop), e o coletor segue com as demais.
class CryptoSourceException implements Exception {
  final String source;
  final String detail;

  const CryptoSourceException(this.source, this.detail);

  @override
  String toString() => '$source: $detail';
}

/// Valor e a fonte que de fato respondeu (gravada em `crypto_indicators.source`).
typedef Sourced = ({double value, String source});

/// GET que devolve o corpo como texto. Rede, timeout, 429 e 5xx (tratados por
/// `quoteGet`) e qualquer status diferente de 200 viram [CryptoSourceException].
/// Diferente das cotações, aqui 404 e 451 (bloqueio regional) também são falha
/// da fonte: não existe "ativo não encontrado" nesses endpoints.
Future<String> cryptoGet(
  http.Client client,
  Uri uri, {
  required String source,
  Duration timeout = const Duration(seconds: 20),
}) async {
  final http.Response response;
  try {
    response = await quoteGet(client, uri, source: source, timeout: timeout);
  } on QuoteUnavailableException {
    throw CryptoSourceException(
      source,
      'unavailable (rate limit, server error, timeout or network)',
    );
  }
  if (response.statusCode != 200) {
    throw CryptoSourceException(source, 'HTTP ${response.statusCode}');
  }
  // `Response.body` cai em Latin-1 quando o servidor não manda charset.
  return utf8.decode(response.bodyBytes, allowMalformed: true);
}

/// JSON decodificado, ou [CryptoSourceException] se o corpo não for JSON (em
/// vez de deixar vazar um `FormatException`).
Object? decodeSourceJson(String source, String body) {
  try {
    return jsonDecode(body);
  } on FormatException {
    throw CryptoSourceException(source, 'invalid JSON');
  }
}

/// Número que a fonte manda como texto. `double.tryParse` aceita `NaN` e
/// `Infinity`, que o `classify` trataria como neutro sem avisar; aqui são
/// recusados.
double parseSourceNumber(String source, String field, String text) {
  final value = double.tryParse(text);
  if (value == null || !value.isFinite) {
    throw CryptoSourceException(source, "invalid number in '$field': $text");
  }
  return value;
}

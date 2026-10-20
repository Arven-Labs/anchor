import 'dart:async';
import 'dart:io';

import 'package:http/http.dart' as http;

import 'quote_exceptions.dart';

/// GET dos serviços de cotação que separa "fonte indisponível" de "ativo não
/// existe": 429 (limite de requisições), 5xx, timeout e rede fora viram
/// `QuoteUnavailableException`; os demais status voltam pro chamador, que
/// decide (404 e afins = não encontrado).
Future<http.Response> quoteGet(
  http.Client client,
  Uri uri, {
  required String source,
  Map<String, String>? headers,
}) async {
  final http.Response response;
  try {
    response = await client
        .get(uri, headers: headers)
        .timeout(const Duration(seconds: 15));
  } on TimeoutException {
    throw QuoteUnavailableException(source);
  } on SocketException {
    throw QuoteUnavailableException(source);
  } on http.ClientException {
    throw QuoteUnavailableException(source);
  }
  if (response.statusCode == 429 || response.statusCode >= 500) {
    throw QuoteUnavailableException(source);
  }
  return response;
}

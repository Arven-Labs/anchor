import 'dart:convert';

import 'package:http/http.dart' as http;

import '../domain/ptax_series.dart';
import 'quote_exceptions.dart';
import 'quote_http.dart';

/// PTAX do Banco Central (Olinda, OData), a cotação que a Lei 14.754/2023 manda
/// usar na conversão de moeda estrangeira. Pública, sem chave; porta de
/// `desktop/src-tauri/src/finance_core/bcb_ptax.rs`. Datas da consulta em
/// `'MM-DD-YYYY'`.
///
/// - Dólar: `CotacaoDolarPeriodo` devolve uma linha por dia útil (o fechamento).
/// - Outras moedas: `CotacaoMoedaPeriodo` devolve todos os boletins do dia
///   (`Abertura`, `Intermediário`, `Fechamento`); vale o `Fechamento`.
/// Os `cotacao*` já vêm em reais por unidade da moeda.
class BcbPtaxService {
  static const _baseUrl =
      'https://olinda.bcb.gov.br/olinda/servico/PTAX/versao/v1/odata';

  /// Moedas que a PTAX cota (`/Moedas` da Olinda).
  static const supportedCurrencies = {
    'AUD',
    'CAD',
    'CHF',
    'DKK',
    'EUR',
    'GBP',
    'JPY',
    'NOK',
    'SEK',
    'USD',
  };

  static bool isSupported(String currency) =>
      supportedCurrencies.contains(currency.trim().toUpperCase());

  final http.Client _client;

  BcbPtaxService({http.Client? client}) : _client = client ?? http.Client();

  static String _date(DateTime date) =>
      '${date.month.toString().padLeft(2, '0')}-'
      '${date.day.toString().padLeft(2, '0')}-${date.year}';

  /// URL da consulta de `currency` entre `from` e `to`.
  static Uri buildUri(String currency, DateTime from, DateTime to) {
    final range =
        '@dataInicial=\'${_date(from)}\'&@dataFinalCotacao=\'${_date(to)}\''
        '&\$top=10000&\$format=json';
    if (currency == 'USD') {
      return Uri.parse(
        '$_baseUrl/CotacaoDolarPeriodo(dataInicial=@dataInicial,'
        'dataFinalCotacao=@dataFinalCotacao)?$range',
      );
    }
    return Uri.parse(
      '$_baseUrl/CotacaoMoedaPeriodo(moeda=@moeda,dataInicial=@dataInicial,'
      'dataFinalCotacao=@dataFinalCotacao)?@moeda=\'$currency\'&$range',
    );
  }

  /// Fechamentos PTAX de `currency` desde `from` até hoje. Moeda que a PTAX não
  /// cota ou série vazia é `QuoteNotFoundException`; limite de requisições,
  /// erro do servidor, timeout, rede fora ou corpo ilegível (o BCB às vezes
  /// devolve HTML com status 200) é `QuoteUnavailableException`.
  Future<PtaxSeries> fetchPtax({
    required String currency,
    required DateTime from,
  }) async {
    final code = currency.trim().toUpperCase();
    if (!isSupported(code)) throw QuoteNotFoundException(code);

    final end = DateTime.now().add(const Duration(days: 1));
    final response = await quoteGet(
      _client,
      buildUri(code, from, end),
      source: 'bcb',
      timeout: const Duration(seconds: 45),
    );
    if (response.statusCode != 200) throw QuoteNotFoundException(code);

    final series = parsePtax(response.body, dollar: code == 'USD');
    if (series.isEmpty) throw QuoteNotFoundException(code);
    return series;
  }

  /// Interpreta a resposta da Olinda. `dollar` = uma linha por dia (se houver
  /// mais de uma, vale a mais tardia); senão só o boletim `Fechamento`.
  /// Cotação não positiva é descartada.
  static PtaxSeries parsePtax(String body, {required bool dollar}) {
    final List<dynamic> items;
    try {
      items = (jsonDecode(body) as Map<String, dynamic>)['value'] as List;
    } catch (_) {
      throw QuoteUnavailableException('bcb');
    }

    final latestAt = <DateTime, String>{};
    final quotes = <DateTime, PtaxQuote>{};
    for (final item in items) {
      if (item is! Map<String, dynamic>) continue;
      final buy = item['cotacaoCompra'];
      final sell = item['cotacaoVenda'];
      final at = item['dataHoraCotacao'];
      if (buy is! num || sell is! num || at is! String || at.length < 10) {
        continue;
      }
      if (buy <= 0 || sell <= 0) continue;
      if (!dollar && item['tipoBoletim'] != 'Fechamento') continue;

      final day = DateTime.tryParse(at.substring(0, 10));
      if (day == null) continue;
      final key = DateTime.utc(day.year, day.month, day.day);
      final previous = latestAt[key];
      if (previous != null && previous.compareTo(at) >= 0) continue;
      latestAt[key] = at;
      quotes[key] = PtaxQuote(buy: buy.toDouble(), sell: sell.toDouble());
    }
    return PtaxSeries([for (final e in quotes.entries) (e.key, e.value)]);
  }
}

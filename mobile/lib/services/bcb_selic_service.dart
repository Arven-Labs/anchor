import 'dart:collection';
import 'dart:convert';

import 'package:http/http.dart' as http;

import '../domain/porquinho_yield.dart';
import 'quote_exceptions.dart';
import 'quote_http.dart';

/// Meta Selic diária do Banco Central (SGS, série 432). Pública, sem chave; porta de
/// `desktop/src-tauri/src/finance_core/bcb_sgs.rs`. O BCB recusa (HTTP 406) uma janela longa de
/// série diária, por isso a busca vai em janelas de 9 anos. Sem cache: a estimativa do Porquinho é
/// manual e rara, então a série é pedida inteira do trecho que ela precisa a cada vez.
class BcbSelicService {
  static const _baseUrl = 'https://api.bcb.gov.br/dados/serie/bcdata.sgs.432/dados';
  static const _windowYears = 9;

  final http.Client _client;

  BcbSelicService({http.Client? client}) : _client = client ?? http.Client();

  static DateTime _day(DateTime date) => DateTime.utc(date.year, date.month, date.day);

  static String _date(DateTime date) =>
      '${date.day.toString().padLeft(2, '0')}/${date.month.toString().padLeft(2, '0')}/${date.year}';

  /// URL da consulta entre [from] e [to] (datas em `dd/MM/yyyy`).
  static Uri buildUri(DateTime from, DateTime to) =>
      Uri.parse('$_baseUrl?formato=json&dataInicial=${_date(from)}&dataFinal=${_date(to)}');

  /// Janelas de [from] até [today] que terminam em 31/12 do 9º ano (a última termina em [today]).
  static List<(DateTime, DateTime)> windows(DateTime from, DateTime today) {
    final windows = <(DateTime, DateTime)>[];
    var start = _day(from);
    final last = _day(today);
    while (!start.isAfter(last)) {
      final nextStart = DateTime.utc(start.year + _windowYears, 1, 1);
      final naturalEnd = nextStart.subtract(const Duration(days: 1));
      final end = naturalEnd.isAfter(last) ? last : naturalEnd;
      windows.add((start, end));
      start = end.add(const Duration(days: 1));
    }
    return windows;
  }

  /// Interpreta uma resposta do SGS: lista de `{"data":"dd/MM/yyyy","valor":"14.00"}`. Linha com data
  /// inexistente ou valor que não é número positivo é descartada; corpo que não é lista JSON (o BCB às
  /// vezes devolve HTML com status 200) é `QuoteUnavailableException`.
  static List<SelicPoint> parseSelic(String body) {
    final List<dynamic> items;
    try {
      items = jsonDecode(body) as List;
    } catch (_) {
      throw QuoteUnavailableException('bcb');
    }
    final points = <SelicPoint>[];
    for (final item in items) {
      if (item is! Map<String, dynamic>) continue;
      final date = _parseDate(item['data']);
      final meta = double.tryParse('${item['valor']}');
      if (date == null || meta == null || !meta.isFinite || meta <= 0) continue;
      points.add(SelicPoint(date, meta));
    }
    points.sort((a, b) => a.date.compareTo(b.date));
    return points;
  }

  static DateTime? _parseDate(Object? text) {
    if (text is! String) return null;
    final parts = text.split('/');
    if (parts.length != 3) return null;
    final day = int.tryParse(parts[0]);
    final month = int.tryParse(parts[1]);
    final year = int.tryParse(parts[2]);
    if (day == null || month == null || year == null) return null;
    final date = DateTime.utc(year, month, day);
    // `DateTime` normaliza 31/02 para março; só vale a data que volta igual.
    return date.year == year && date.month == month && date.day == day ? date : null;
  }

  /// Meta Selic diária de [from] até hoje, em ordem de data e sem dia repetido. Erro do servidor, limite
  /// de requisições, HTTP diferente de 200, corpo ilegível ou série vazia é `QuoteUnavailableException`.
  Future<List<SelicPoint>> fetchSince(DateTime from, {DateTime? today}) async {
    final byDay = SplayTreeMap<DateTime, SelicPoint>();
    for (final (start, end) in windows(from, today ?? DateTime.now())) {
      final response = await quoteGet(
        _client,
        buildUri(start, end),
        source: 'bcb',
        timeout: const Duration(seconds: 45),
      );
      if (response.statusCode != 200) throw QuoteUnavailableException('bcb');
      for (final point in parseSelic(response.body)) {
        byDay[point.date] = point;
      }
    }
    if (byDay.isEmpty) throw QuoteUnavailableException('bcb');
    return byDay.values.toList();
  }
}

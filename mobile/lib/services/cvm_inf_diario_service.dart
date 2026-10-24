import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:archive/archive_io.dart';
import 'package:http/http.dart' as http;
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

import '../domain/cvm_inf_diario.dart';

/// A CVM não respondeu de forma utilizável (limite, erro do servidor, timeout, rede
/// fora) ou mandou um arquivo ilegível. O mês pode existir; não deu para ler agora.
class CvmUnavailableException implements Exception {
  final String detail;

  CvmUnavailableException(this.detail);

  @override
  String toString() => 'CVM informe diário: $detail';
}

/// Baixa o zip mensal do informe diário da CVM e lê as cotas dos fundos pedidos.
/// Porta de `CvmDailyZips` e `extract_month` do desktop, com o cuidado extra do
/// celular: o zip (~12 MB num mês fechado) vai para o disco em fluxo e o CSV (~35 MB) é
/// descomprimido para um arquivo temporário e lido linha a linha, nunca inteiro na
/// memória.
class CvmInfDiarioService {
  static const baseUrl = 'https://dados.cvm.gov.br/dados/FI/DOC/INF_DIARIO/DADOS';

  /// O mês corrente e o anterior ainda recebem dias novos e retificações: o zip em
  /// cache vale 12 horas. Os fechados, 30 dias.
  static const recentMaxAge = Duration(hours: 12);
  static const closedMaxAge = Duration(days: 30);

  /// O zip é grande e a CVM oscila: timeout bem maior que o das cotações.
  static const downloadTimeout = Duration(minutes: 2);

  final http.Client _client;
  final Future<Directory> Function() _cacheDir;
  final DateTime Function() _now;

  CvmInfDiarioService({
    http.Client? client,
    Future<Directory> Function()? cacheDir,
    DateTime Function()? now,
  }) : _client = client ?? http.Client(),
       _cacheDir = cacheDir ?? getApplicationCacheDirectory,
       _now = now ?? DateTime.now;

  static String zipName(YearMonth month) =>
      'inf_diario_fi_${month.$1}${month.$2.toString().padLeft(2, '0')}.zip';

  /// As cotas de [targets] no zip do mês, ou `null` se a CVM ainda não publicou o mês
  /// (404: o mês corrente só aparece depois do primeiro dia útil).
  Future<CvmExtract?> extractMonth(
    YearMonth month,
    List<CvmTarget> targets,
  ) async {
    final zip = await _zipFile(month);
    if (zip == null) return null;
    try {
      return await _extract(zip, targets);
    } on CvmUnavailableException {
      rethrow;
    } on FormatException catch (e) {
      // Inclui `ArchiveException`. Zip truncado, corrompido ou sem o CSV do informe
      // (o `archive` devolve um zip vazio para lixo) e CSV sem as colunas: o arquivo
      // não serve e não fica no cache para a próxima tentativa.
      await _discard(zip);
      throw CvmUnavailableException('arquivo ilegível (${e.message})');
    } on FileSystemException catch (e) {
      throw CvmUnavailableException('disco: ${e.message}');
    }
  }

  Future<void> _discard(File file) async {
    try {
      if (await file.exists()) await file.delete();
    } on FileSystemException {
      // nada a fazer: a próxima leitura tenta de novo
    }
  }

  /// O zip do mês no cache (baixa se faltar ou estiver velho), ou `null` no 404.
  Future<File?> _zipFile(YearMonth month) async {
    final root = await _cacheDir();
    final dir = Directory(p.join(root.path, 'cvm_inf_diario'));
    await dir.create(recursive: true);
    final file = File(p.join(dir.path, zipName(month)));

    final now = _now();
    final maxAge = isRecentMonth(month, now) ? recentMaxAge : closedMaxAge;
    if (await file.exists()) {
      final age = now.difference(await file.lastModified());
      if (age < maxAge) return file;
    }

    final part = File('${file.path}.part');
    final request = http.Request('GET', Uri.parse('$baseUrl/${zipName(month)}'));
    try {
      final response = await _client.send(request).timeout(downloadTimeout);
      if (response.statusCode == 404) {
        await response.stream.drain<void>();
        return null;
      }
      if (response.statusCode != 200) {
        await response.stream.drain<void>();
        throw CvmUnavailableException('HTTP ${response.statusCode}');
      }
      final sink = part.openWrite();
      try {
        await response.stream.timeout(downloadTimeout).pipe(sink);
      } catch (_) {
        await _discard(part);
        rethrow;
      }
    } on TimeoutException {
      await _discard(part);
      throw CvmUnavailableException('tempo esgotado');
    } on SocketException {
      await _discard(part);
      throw CvmUnavailableException('sem rede');
    } on http.ClientException catch (e) {
      await _discard(part);
      throw CvmUnavailableException('sem rede (${e.message})');
    }
    // Só vira cache depois de baixado inteiro.
    await part.rename(file.path);
    return file;
  }

  Future<CvmExtract> _extract(File zip, List<CvmTarget> targets) async {
    final csv = File('${zip.path}.csv');
    final input = InputFileStream(zip.path);
    try {
      // Só lê o diretório do zip; o conteúdo é descomprimido sob demanda.
      final archive = ZipDecoder().decodeStream(input);
      final entry = archive.files.firstWhere(
        (f) =>
            f.isFile &&
            f.name.startsWith('inf_diario_fi_') &&
            f.name.endsWith('.csv'),
        orElse: () =>
            throw const FormatException('sem inf_diario_fi_*.csv no zip'),
      );
      final output = OutputFileStream(csv.path);
      try {
        entry.writeContent(output);
      } finally {
        output.closeSync();
      }

      final extractor = CvmQuotaExtractor(targets);
      await csv
          .openRead()
          .transform(latin1.decoder)
          .transform(const LineSplitter())
          .forEach(extractor.addLine);
      return extractor.result;
    } finally {
      input.closeSync();
      await _discard(csv);
    }
  }
}

// Import do CSV "Anchor CSV" (o mesmo que o desktop exporta e que `transactions_csv.dart`
// gera). Espelha o importador do desktop (`commands/portfolio_io.rs`): só acrescenta,
// nunca apaga, e deduplica pela mesma chave (ticker, data, tipo, quantidade, total).
// Tudo aqui é puro, sem I/O: a gravação fica em `PortfolioRepository.applyTransactionsImport`.
//
// Ativo e custódia vêm por ticker e por nome, nunca por id (os ids dos dois bancos não
// casam). `transferencia` é pulada: o CSV tem uma coluna de custódia só, sem o destino.

import '../models/asset.dart';
import '../models/custodia.dart';
import '../models/portfolio_transaction.dart';

class ImportRow {
  final int line;
  final DateTime date;
  final TransactionType type;
  final String ticker;
  final AssetClass? assetClass;
  final String? currency;
  final double? quantity;
  final double? unitPrice;
  final double totalValue;
  final double? fee;
  final String? custody;
  final String? notes;
  final double? lossCreditUsed;

  const ImportRow({
    required this.line,
    required this.date,
    required this.type,
    required this.ticker,
    required this.assetClass,
    required this.currency,
    required this.quantity,
    required this.unitPrice,
    required this.totalValue,
    required this.fee,
    required this.custody,
    required this.notes,
    required this.lossCreditUsed,
  });
}

class ImportIssue {
  /// 1 = primeira linha de dados; 0 = o cabeçalho ou o arquivo todo.
  final int line;
  final String message;

  const ImportIssue(this.line, this.message);
}

class ParsedTransactionsCsv {
  final List<ImportRow> rows;
  final List<ImportIssue> issues;

  const ParsedTransactionsCsv(this.rows, this.issues);
}

class NewAssetPlan {
  final String ticker;
  final AssetClass assetClass;
  final String currency;

  const NewAssetPlan({
    required this.ticker,
    required this.assetClass,
    required this.currency,
  });
}

class ImportPlan {
  /// Linhas que serão gravadas.
  final List<ImportRow> rows;

  /// Linhas que já existem no celular.
  final int duplicates;

  /// Linhas que o CSV não consegue representar (transferência).
  final int skipped;
  final List<ImportIssue> issues;
  final List<NewAssetPlan> newAssets;

  /// Nomes de instituição a criar (com titular "Importado").
  final List<String> newCustodias;

  const ImportPlan({
    required this.rows,
    required this.duplicates,
    required this.skipped,
    required this.issues,
    required this.newAssets,
    required this.newCustodias,
  });
}

const _required = ['date', 'type', 'total_value'];

ParsedTransactionsCsv parseTransactionsCsv(String text) {
  final records = _readRecords(text);
  if (records.isEmpty) {
    return const ParsedTransactionsCsv([], [ImportIssue(0, 'arquivo vazio')]);
  }
  final columns = <String, int>{};
  for (var i = 0; i < records.first.length; i++) {
    columns[records.first[i].trim().toLowerCase()] = i;
  }
  for (final name in _required) {
    if (!columns.containsKey(name)) {
      return ParsedTransactionsCsv([], [
        ImportIssue(0, "coluna '$name' ausente no cabeçalho"),
      ]);
    }
  }

  final rows = <ImportRow>[];
  final issues = <ImportIssue>[];
  for (var i = 1; i < records.length; i++) {
    final line = i;
    final record = records[i];
    String cell(String name) {
      final index = columns[name];
      return index == null || index >= record.length
          ? ''
          : record[index].trim();
    }

    final date = _parseDate(cell('date'));
    if (date == null) {
      issues.add(
        ImportIssue(line, "data inválida: '${cell('date')}' (esperado AAAA-MM-DD ou DD/MM/AAAA)"),
      );
      continue;
    }
    final type = TransactionType.tryParse(cell('type').toLowerCase());
    if (type == null) {
      issues.add(ImportIssue(line, "tipo desconhecido: '${cell('type')}'"));
      continue;
    }

    final numbers = <String, double?>{};
    String? numberError;
    for (final name in [
      'quantity',
      'unit_price',
      'total_value',
      'fee',
      'loss_credit_used',
    ]) {
      final parsed = _parseNumber(cell(name));
      if (parsed.error) {
        numberError = "número inválido em '$name': '${cell(name)}'";
        break;
      }
      numbers[name] = parsed.value;
    }
    if (numberError != null) {
      issues.add(ImportIssue(line, numberError));
      continue;
    }
    final total = numbers['total_value'];
    if (total == null) {
      issues.add(ImportIssue(line, 'total_value vazio'));
      continue;
    }

    final classText = cell('asset_class').toLowerCase();
    AssetClass? assetClass;
    if (classText.isNotEmpty) {
      assetClass = _assetClass(classText);
      if (assetClass == null) {
        issues.add(ImportIssue(line, "classe desconhecida: '$classText'"));
        continue;
      }
    }

    rows.add(
      ImportRow(
        line: line,
        date: date,
        type: type,
        ticker: _normalizeTicker(cell('ticker')),
        assetClass: assetClass,
        currency: cell('currency').isEmpty ? null : cell('currency').toUpperCase(),
        quantity: numbers['quantity'],
        unitPrice: numbers['unit_price'],
        totalValue: total,
        fee: numbers['fee'],
        custody: cell('custody').isEmpty ? null : cell('custody'),
        notes: cell('notes').isEmpty ? null : cell('notes'),
        lossCreditUsed: numbers['loss_credit_used'],
      ),
    );
  }
  return ParsedTransactionsCsv(rows, issues);
}

/// Decide, sem gravar nada, o que o import faria contra o que já está no celular.
ImportPlan planTransactionsImport(
  ParsedTransactionsCsv parsed, {
  required List<Asset> assets,
  required List<Custodia> custodias,
  required List<PortfolioTransaction> transactions,
}) {
  final assetsByTicker = {
    for (final asset in assets) asset.ticker.toUpperCase(): asset,
  };
  final assetsById = {
    for (final asset in assets)
      if (asset.id != null) asset.id!: asset,
  };

  // Cada lançamento já gravado cobre uma linha do arquivo: dois negócios iguais
  // no mesmo dia continuam sendo dois.
  final remaining = <String, int>{};
  for (final t in transactions) {
    final ticker = t.assetId == null ? '' : assetsById[t.assetId]?.ticker ?? '';
    final key = _dedupKey(
      ticker.toUpperCase(),
      t.date,
      t.type,
      t.quantity,
      t.totalValue,
    );
    remaining[key] = (remaining[key] ?? 0) + 1;
  }

  final knownCustodias = {
    for (final custodia in custodias)
      custodia.instituicao.toLowerCase(): custodia.instituicao,
  };
  final newAssets = <String, NewAssetPlan>{};
  final newCustodias = <String, String>{};
  final rows = <ImportRow>[];
  final issues = [...parsed.issues];
  var duplicates = 0;
  var skipped = 0;

  for (final row in parsed.rows) {
    if (row.type == TransactionType.transferencia) {
      skipped++;
      continue;
    }

    final problem = _validate(row, assetsByTicker, newAssets);
    if (problem != null) {
      issues.add(ImportIssue(row.line, problem));
      continue;
    }

    final key = _dedupKey(
      row.ticker,
      row.date,
      row.type,
      row.quantity,
      row.totalValue,
    );
    final left = remaining[key] ?? 0;
    if (left > 0) {
      remaining[key] = left - 1;
      duplicates++;
      continue;
    }

    if (row.type.needsAsset &&
        !assetsByTicker.containsKey(row.ticker) &&
        !newAssets.containsKey(row.ticker)) {
      final assetClass = row.assetClass!;
      newAssets[row.ticker] = NewAssetPlan(
        ticker: row.ticker,
        assetClass: assetClass,
        currency: row.currency ?? assetClass.defaultCurrency,
      );
    }
    final custody = row.custody;
    if (custody != null) {
      final name = custody.toLowerCase();
      if (!knownCustodias.containsKey(name)) {
        newCustodias.putIfAbsent(name, () => custody);
      }
    }
    rows.add(row);
  }

  return ImportPlan(
    rows: rows,
    duplicates: duplicates,
    skipped: skipped,
    issues: issues,
    newAssets: newAssets.values.toList(),
    newCustodias: newCustodias.values.toList(),
  );
}

String? _validate(
  ImportRow row,
  Map<String, Asset> assetsByTicker,
  Map<String, NewAssetPlan> newAssets,
) {
  if (row.type.needsAsset) {
    if (row.ticker.isEmpty) return 'ticker obrigatório para ${row.type.value}';
    final existing = assetsByTicker[row.ticker];
    final planned = newAssets[row.ticker];
    if (existing == null && planned == null && row.assetClass == null) {
      return "ativo novo '${row.ticker}': informe a asset_class";
    }
    final assetClass =
        existing?.assetClass ?? planned?.assetClass ?? row.assetClass;
    if (row.type.isFundOnly && assetClass != AssetClass.fundo) {
      return '${row.type.value} só vale para ativo da classe fundo';
    }
  }
  if (row.type.needsQuantity && row.quantity == null) {
    return 'quantity obrigatória para ${row.type.value}';
  }
  if (row.type.needsUnitPrice && row.unitPrice == null) {
    return 'unit_price obrigatório para ${row.type.value}';
  }
  return null;
}

/// Mesma chave do desktop (`dedup_key`): a data é comparada como AAAA-MM-DD, a
/// quantidade com 6 casas e o total em centavos, para o ruído de float não
/// transformar uma duplicata em linha nova.
String _dedupKey(
  String ticker,
  DateTime date,
  TransactionType type,
  double? quantity,
  double total,
) =>
    '$ticker|${_day(date)}|${type.value}|${((quantity ?? 0) * 1e6).round()}|${(total * 100).round()}';

String _day(DateTime date) {
  final month = date.month.toString().padLeft(2, '0');
  final day = date.day.toString().padLeft(2, '0');
  return '${date.year}-$month-$day';
}

AssetClass? _assetClass(String value) {
  for (final assetClass in AssetClass.values) {
    if (assetClass.value == value) return assetClass;
  }
  return null;
}

/// Ticker em caixa alta e sem o `F` do fracionário (`PETR4F` → `PETR4`).
String _normalizeTicker(String raw) {
  final ticker = raw.trim().toUpperCase();
  final isFractional =
      ticker.length >= 6 &&
      ticker.endsWith('F') &&
      RegExp(r'^[A-Z]{4}.*\d$').hasMatch(ticker.substring(0, ticker.length - 1));
  return isFractional ? ticker.substring(0, ticker.length - 1) : ticker;
}

/// `AAAA-MM-DD` ou `DD/MM/AAAA`. O `DateTime` do Dart normaliza `31/02` para março
/// sem reclamar, então a data montada é conferida de volta.
DateTime? _parseDate(String raw) {
  if (raw.length != 10) return null;
  int? year, month, day;
  final iso = RegExp(r'^(\d{4})-(\d{2})-(\d{2})$').firstMatch(raw);
  final br = RegExp(r'^(\d{2})/(\d{2})/(\d{4})$').firstMatch(raw);
  if (iso != null) {
    year = int.parse(iso.group(1)!);
    month = int.parse(iso.group(2)!);
    day = int.parse(iso.group(3)!);
  } else if (br != null) {
    day = int.parse(br.group(1)!);
    month = int.parse(br.group(2)!);
    year = int.parse(br.group(3)!);
  } else {
    return null;
  }
  final date = DateTime(year, month, day);
  if (date.year != year || date.month != month || date.day != day) return null;
  return year >= 1900 ? date : null;
}

/// Número com ponto (formato do Anchor) ou com vírgula decimal (BR, com ou sem
/// milhar). Vazio é `null` sem erro; `NaN` e infinito são erro.
({double? value, bool error}) _parseNumber(String raw) {
  final cleaned = raw.replaceAll(RegExp(r'[\sR$]'), '');
  if (cleaned.isEmpty || cleaned == '-') return (value: null, error: false);
  final normalized = cleaned.contains(',')
      ? cleaned.replaceAll('.', '').replaceAll(',', '.')
      : cleaned;
  final value = double.tryParse(normalized);
  if (value == null || !value.isFinite) return (value: null, error: true);
  return (value: value, error: false);
}

/// Leitor de CSV: campos entre aspas, `""` como aspas literal, vírgula e quebra de
/// linha dentro de aspas, CRLF, BOM. Linhas totalmente vazias são ignoradas.
List<List<String>> _readRecords(String input) {
  final text = input.startsWith('\u{feff}') ? input.substring(1) : input;
  final records = <List<String>>[];
  var record = <String>[];
  final field = StringBuffer();
  var quoted = false;

  void endField() {
    record.add(field.toString());
    field.clear();
  }

  void endRecord() {
    endField();
    if (record.length > 1 || record.first.trim().isNotEmpty) {
      records.add(record);
    }
    record = <String>[];
  }

  for (var i = 0; i < text.length; i++) {
    final c = text[i];
    if (quoted) {
      if (c == '"') {
        if (i + 1 < text.length && text[i + 1] == '"') {
          field.write('"');
          i++;
        } else {
          quoted = false;
        }
      } else {
        field.write(c);
      }
    } else if (c == '"') {
      quoted = true;
    } else if (c == ',') {
      endField();
    } else if (c == '\n') {
      endRecord();
    } else if (c != '\r') {
      field.write(c);
    }
  }
  if (field.isNotEmpty || record.isNotEmpty) endRecord();
  return records;
}

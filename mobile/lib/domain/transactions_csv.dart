// CSV das transações com as mesmas 16 colunas do desktop (`CSV_HEADER` em
// `commands/portfolio_io.rs`), para o desktop importar pelo formato "Anchor CSV".
// O ativo vai pelo ticker e a custódia pelo nome, nunca pelo id: os ids do celular
// e do desktop são independentes e não casam. A data sai só como AAAA-MM-DD porque
// o desktop deduplica comparando o texto da data.
//
// Só exporta o que o importador do desktop sabe ler. `transferencia` (o CSV não tem
// custódia de destino) e `amortizacao` ficam de fora, e entram em [skipped] para a
// tela avisar em vez de descartar em silêncio.

import '../models/asset.dart';
import '../models/custodia.dart';
import '../models/portfolio_transaction.dart';

const transactionsCsvHeader = [
  'date',
  'type',
  'ticker',
  'asset_class',
  'currency',
  'quantity',
  'unit_price',
  'total_value',
  'fee',
  'custody',
  'payment_type',
  'notes',
  'loss_credit_used',
  'fund_term',
  'fund_structure',
  'fund_estoque_option',
];

const _exportedTypes = {
  TransactionType.compra,
  TransactionType.venda,
  TransactionType.aporte,
  TransactionType.retirada,
  TransactionType.provento,
  TransactionType.comeCotas,
};

class TransactionsCsv {
  final String csv;
  final int exported;

  /// Transações que o CSV não carrega (tipo sem suporte no importador do desktop ou
  /// ativo que não existe mais).
  final int skipped;

  const TransactionsCsv({
    required this.csv,
    required this.exported,
    required this.skipped,
  });
}

String _field(String value) {
  if (!value.contains(RegExp(r'[",\r\n]'))) return value;
  return '"${value.replaceAll('"', '""')}"';
}

/// Número sem zeros à direita e com ponto decimal (`10`, `2.5`, `0.00123456`).
String _number(double? value) {
  if (value == null) return '';
  final text = value.toStringAsFixed(8);
  return text.replaceFirst(RegExp(r'0+$'), '').replaceFirst(RegExp(r'\.$'), '');
}

String _day(DateTime date) {
  final month = date.month.toString().padLeft(2, '0');
  final day = date.day.toString().padLeft(2, '0');
  return '${date.year}-$month-$day';
}

/// Uma linha por transação, em ordem cronológica (a ordem de entrada desempata o
/// mesmo dia). A custódia sai pelo nome da instituição, que é o que o desktop casa:
/// o `Custodia.label` ("XP (Fulano)") é rótulo de tela e criaria uma custódia
/// duplicada lá.
TransactionsCsv transactionsCsv(
  List<Asset> assets,
  List<Custodia> custodias,
  List<PortfolioTransaction> transactions,
) {
  final assetsById = {
    for (final asset in assets)
      if (asset.id != null) asset.id!: asset,
  };
  final custodiaNames = {
    for (final custodia in custodias)
      if (custodia.id != null) custodia.id!: custodia.instituicao,
  };
  final ordered = [
    for (var i = 0; i < transactions.length; i++) (i, transactions[i]),
  ]..sort((a, b) {
      final byDate = a.$2.date.compareTo(b.$2.date);
      return byDate != 0 ? byDate : a.$1.compareTo(b.$1);
    });

  final lines = [transactionsCsvHeader.join(',')];
  var skipped = 0;
  for (final (_, t) in ordered) {
    final asset = t.assetId == null ? null : assetsById[t.assetId];
    if (!_exportedTypes.contains(t.type) ||
        (t.type.needsAsset && asset == null)) {
      skipped++;
      continue;
    }
    lines.add(
      [
        _day(t.date),
        t.type.value,
        asset?.ticker ?? '',
        asset?.assetClass.value ?? '',
        asset?.currency ?? '',
        _number(t.quantity),
        _number(t.unitPrice),
        _number(t.totalValue),
        _number(t.fee),
        t.custodiaId == null ? '' : custodiaNames[t.custodiaId] ?? '',
        '', // payment_type: o mobile não guarda
        t.notes ?? '',
        _number(t.lossCreditUsed),
        '', // fund_*: configuração do fundo só existe no desktop
        '',
        '',
      ].map(_field).join(','),
    );
  }
  return TransactionsCsv(
    csv: '${lines.join('\n')}\n',
    exported: lines.length - 1,
    skipped: skipped,
  );
}

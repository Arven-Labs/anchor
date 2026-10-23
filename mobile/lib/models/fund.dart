/// Tipo tributário do fundo aberto, escolhido pelo usuário (o app não lê isso de
/// lugar nenhum). Valores string iguais ao desktop (`fund_details.term`).
enum FundTerm {
  longTerm('longo_prazo', 'Longo prazo'),
  shortTerm('curto_prazo', 'Curto prazo'),
  equity('acoes', 'Ações');

  final String value;
  final String label;

  const FundTerm(this.value, this.label);

  /// `null` para texto desconhecido, como `FundTerm::parse` do desktop.
  static FundTerm? parse(String? value) {
    for (final term in FundTerm.values) {
      if (term.value == value) return term;
    }
    return null;
  }
}

/// Valor de uma cota numa data, em BRL, lançado pelo usuário. `source` já existe
/// para a leitura da CVM entrar depois; hoje só há `manual`.
class FundQuota {
  final int? id;
  final int assetId;
  final DateTime date;
  final double value;
  final String source;
  final String? notes;
  final DateTime createdAt;

  const FundQuota({
    this.id,
    required this.assetId,
    required this.date,
    required this.value,
    this.source = 'manual',
    this.notes,
    required this.createdAt,
  });

  Map<String, Object?> toMap() => {
    'id': id,
    'asset_id': assetId,
    'quota_date': _dateOnly(date),
    'value': value,
    'source': source,
    'notes': notes,
    'created_at': createdAt.toIso8601String(),
  };

  factory FundQuota.fromMap(Map<String, Object?> map) => FundQuota(
    id: map['id'] as int,
    assetId: map['asset_id'] as int,
    date: DateTime.parse('${map['quota_date']}T00:00:00Z'),
    value: (map['value'] as num).toDouble(),
    source: map['source'] as String,
    notes: map['notes'] as String?,
    createdAt: DateTime.parse(map['created_at'] as String),
  );
}

String _dateOnly(DateTime date) =>
    '${date.year.toString().padLeft(4, '0')}-'
    '${date.month.toString().padLeft(2, '0')}-'
    '${date.day.toString().padLeft(2, '0')}';

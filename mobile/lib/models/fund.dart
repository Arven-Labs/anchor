/// Tipo tributário do fundo aberto, escolhido pelo usuário (o app não lê isso de
/// lugar nenhum). Valores string iguais ao desktop (`fund_details.term`).
enum FundTerm {
  longTerm('longo_prazo', 'Longo prazo'),
  shortTerm('curto_prazo', 'Curto prazo'),
  equity('acoes', 'Ações'),

  /// Entidade de investimento (FIP, FIDC, ETF que não é de renda fixa; Lei 14.754,
  /// arts. 18 e 24): 15% fixo, sem IOF e sem come-cotas.
  investmentEntity('entidade_investimento', 'Entidade de investimento');

  final String value;
  final String label;

  const FundTerm(this.value, this.label);

  /// Alíquota fixa, sem IOF nem come-cotas (ações e entidade de investimento).
  bool get isFlatRate => this == equity || this == investmentEntity;

  /// `null` para texto desconhecido, como `FundTerm::parse` do desktop.
  static FundTerm? parse(String? value) {
    for (final term in FundTerm.values) {
      if (term.value == value) return term;
    }
    return null;
  }
}

/// Estrutura do fundo (Lei 14.754). O IR do art. 17 é o mesmo nas três, mas só o fechado
/// e o exclusivo têm estoque de 2023. Valores string iguais ao desktop
/// (`fund_details.structure`).
enum FundStructure {
  open('aberto', 'Aberto'),
  closed('fechado', 'Fechado'),
  exclusive('exclusivo', 'Exclusivo');

  final String value;
  final String label;

  const FundStructure(this.value, this.label);

  /// `null` para texto desconhecido.
  static FundStructure? parse(String? value) {
    for (final structure in FundStructure.values) {
      if (structure.value == value) return structure;
    }
    return null;
  }
}

/// Como o estoque de rendimento até 31/12/2023 foi tributado (arts. 27 e 28). Valores
/// string iguais ao desktop (`fund_details.estoque_option`).
enum EstoqueOption {
  general('geral', 'Regra geral (15%)'),
  option8('opcao_8', 'Opção (8%)');

  final String value;
  final String label;

  const EstoqueOption(this.value, this.label);

  /// `null` para texto desconhecido.
  static EstoqueOption? parse(String? value) {
    for (final option in EstoqueOption.values) {
      if (option.value == value) return option;
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

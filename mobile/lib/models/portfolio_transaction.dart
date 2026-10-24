/// Os tipos do desktop que o mobile usa. Valores string iguais ao desktop
/// (`desktop/src/portfolio/types.ts`).
enum TransactionType {
  compra('compra', 'Compra'),
  venda('venda', 'Venda'),
  aporte('aporte', 'Aporte'),
  retirada('retirada', 'Retirada'),
  provento('provento', 'Provento'),
  transferencia('transferencia', 'Transferência'),

  /// Desconto real do come-cotas de um fundo: cotas retiradas, valor da cota do dia e
  /// total = imposto em reais. Só vale para ativo `fundo`, sem taxa e sem caixa.
  comeCotas('come_cotas', 'Come-cotas'),

  /// Amortização de fundo fechado ou exclusivo (Lei 14.754, art. 17): valor recebido
  /// em `totalValue` e valor da cota do dia em `unitPrice`, sem quantidade nem taxa. As
  /// cotas ficam e o custo cai na fração `valor ÷ (cota × cotas)`. Só vale para ativo
  /// `fundo`.
  amortizacao('amortizacao', 'Amortização');

  final String value;
  final String label;

  const TransactionType(this.value, this.label);

  static TransactionType fromValue(String value) {
    return TransactionType.values.firstWhere((t) => t.value == value);
  }

  /// `null` para valor desconhecido (um tipo que só o desktop tem, por exemplo), para
  /// quem lê o banco poder pular a linha em vez de quebrar a listagem inteira.
  static TransactionType? tryParse(String? value) {
    for (final type in TransactionType.values) {
      if (type.value == value) return type;
    }
    return null;
  }
}

/// Quais campos cada tipo exige — espelha `needsAsset`/`needsQuantity`/
/// `needsUnitPrice`/`needsTransferDestination` de
/// `desktop/src/portfolio/TransactionSection.tsx:65-79`. `aporte`/`retirada`
/// são fluxo de caixa puro (sem ativo); `provento` tem ativo (qual pagou)
/// mas não quantidade/preço unitário, só um valor total digitado direto;
/// `transferencia` tem quantidade mas não preço unitário (não tem "preço",
/// só move o que já existe) e exige custódia de origem e destino.
extension TransactionTypeMeta on TransactionType {
  bool get needsAsset =>
      this != TransactionType.aporte && this != TransactionType.retirada;

  bool get needsQuantity =>
      this == TransactionType.compra ||
      this == TransactionType.venda ||
      this == TransactionType.transferencia ||
      this == TransactionType.comeCotas;

  bool get needsUnitPrice =>
      this == TransactionType.compra ||
      this == TransactionType.venda ||
      this == TransactionType.comeCotas ||
      this == TransactionType.amortizacao;

  /// Tipos que só valem para ativo da classe `fundo`.
  bool get isFundOnly =>
      this == TransactionType.comeCotas || this == TransactionType.amortizacao;

  bool get needsTransferDestination => this == TransactionType.transferencia;
}

/// Mesmos valores de `FI_INDEXADORES`/`FI_LIQUIDEZ_OPTIONS`
/// (`desktop/src/portfolio/types.ts:242-243`) — usados no bloco de campos
/// `fi_*` da transação de compra de renda fixa (`AssetClassMeta.isFixedIncome`).
const fiIndexadores = ['CDI', 'IPCA', 'SELIC', 'PREFIXADO', 'OUTRO'];
const fiLiquidezOptions = ['diaria', 'no_vencimento', 'outro'];

class PortfolioTransaction {
  final int? id;

  /// `null` pra `aporte`/`retirada` (fluxo de caixa puro, sem ativo).
  final int? assetId;
  final TransactionType type;

  /// `null` pra tipos que não usam quantidade (ver `TransactionTypeMeta`).
  final double? quantity;

  /// `null` pra tipos que não usam preço unitário (ver `TransactionTypeMeta`).
  final double? unitPrice;
  final double totalValue;

  /// Taxa da corretora/exchange — só em `compra`/`venda`, `null` = sem taxa.
  /// Na compra soma ao custo; na venda reduz o ganho (como no desktop).
  final double? fee;

  /// Imposto retido no exterior sobre o provento, na moeda do ativo — só em
  /// `provento` de ativo fora de BRL, `null` = sem retenção informada. É o
  /// crédito da apuração da Lei 14.754 (`foreign_tax.dart`), como no desktop.
  final double? foreignTaxWithheld;

  /// Custódia da transação — opcional em todos os tipos. Pra
  /// `transferencia` é a origem.
  final int? custodiaId;

  /// Só preenchido em `transferencia` (destino) — `null` nos outros tipos.
  final int? transferToCustodiaId;
  final DateTime date;
  final String? notes;
  final DateTime createdAt;

  /// Campos de renda fixa (`showFixedIncomeFields` no desktop) — só
  /// preenchidos numa `compra` de ativo `tesouroDireto`/`rendaFixa`, `null`
  /// em qualquer outro caso.
  final String? fiEmissor;
  final String? fiIndexador;
  final double? fiTaxaPercentual;
  final DateTime? fiDataVencimento;
  final String? fiLiquidez;

  const PortfolioTransaction({
    this.id,
    this.assetId,
    required this.type,
    this.quantity,
    this.unitPrice,
    required this.totalValue,
    this.fee,
    this.foreignTaxWithheld,
    this.custodiaId,
    this.transferToCustodiaId,
    required this.date,
    this.notes,
    required this.createdAt,
    this.fiEmissor,
    this.fiIndexador,
    this.fiTaxaPercentual,
    this.fiDataVencimento,
    this.fiLiquidez,
  });

  Map<String, Object?> toMap() {
    return {
      'id': id,
      'asset_id': assetId,
      'transaction_type': type.value,
      'quantity': quantity,
      'unit_price': unitPrice,
      'total_value': totalValue,
      'fee': fee,
      'foreign_tax_withheld': foreignTaxWithheld,
      'custodia_id': custodiaId,
      'transfer_to_custodia_id': transferToCustodiaId,
      'transaction_date': date.toIso8601String(),
      'notes': notes,
      'created_at': createdAt.toIso8601String(),
      'fi_emissor': fiEmissor,
      'fi_indexador': fiIndexador,
      'fi_taxa_percentual': fiTaxaPercentual,
      'fi_data_vencimento': fiDataVencimento?.toIso8601String(),
      'fi_liquidez': fiLiquidez,
    };
  }

  factory PortfolioTransaction.fromMap(Map<String, Object?> map) {
    return PortfolioTransaction(
      id: map['id'] as int,
      assetId: map['asset_id'] as int?,
      type: TransactionType.fromValue(map['transaction_type'] as String),
      quantity: map['quantity'] as double?,
      unitPrice: map['unit_price'] as double?,
      totalValue: map['total_value'] as double,
      fee: map['fee'] as double?,
      foreignTaxWithheld: map['foreign_tax_withheld'] as double?,
      custodiaId: map['custodia_id'] as int?,
      transferToCustodiaId: map['transfer_to_custodia_id'] as int?,
      date: DateTime.parse(map['transaction_date'] as String),
      notes: map['notes'] as String?,
      createdAt: DateTime.parse(map['created_at'] as String),
      fiEmissor: map['fi_emissor'] as String?,
      fiIndexador: map['fi_indexador'] as String?,
      fiTaxaPercentual: map['fi_taxa_percentual'] as double?,
      fiDataVencimento: map['fi_data_vencimento'] != null
          ? DateTime.parse(map['fi_data_vencimento'] as String)
          : null,
      fiLiquidez: map['fi_liquidez'] as String?,
    );
  }
}

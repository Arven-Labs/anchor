/// `receita` e `despesa` são lançadas à mão; as duas pernas de transferência
/// nascem juntas em `GeneralFinanceRepository.createTransfer` e as parcelas de
/// dívida em `createLiability`. Empréstimos do desktop ficam fora do mobile.
enum GeneralTransactionType {
  receita('receita', 'Receita'),
  despesa('despesa', 'Despesa'),
  transferenciaSaida('transferencia_saida', 'Transferência (saída)'),
  transferenciaEntrada('transferencia_entrada', 'Transferência (entrada)'),
  parcelaDivida('parcela_divida', 'Parcela de dívida');

  /// Texto gravado em `transaction_type`, o mesmo do desktop.
  final String value;
  final String label;

  const GeneralTransactionType(this.value, this.label);

  /// O que o usuário escolhe ao lançar um movimento.
  static const manual = [receita, despesa];

  bool get isTransfer => this == transferenciaSaida || this == transferenciaEntrada;

  static GeneralTransactionType fromValue(String value) =>
      values.firstWhere((t) => t.value == value);
}

/// Lançamento de uma conta — espelha
/// `desktop/src-tauri/src/entity/general_transaction.rs`. `valor` é sempre
/// positivo; o sinal vem do tipo (`signedValor` em
/// `domain/general_finance.dart`). A categoria só vale para despesa; as duas
/// pernas de uma transferência dividem o mesmo `transferId`.
class GeneralTransaction {
  final int? id;
  final int bankAccountId;
  final GeneralTransactionType type;
  final int? categoryId;
  final int? transferId;
  final double valor;

  /// `YYYY-MM-DD`; a comparação com "hoje" é de texto, como no desktop.
  final String date;
  final String? notes;
  final DateTime createdAt;

  const GeneralTransaction({
    this.id,
    required this.bankAccountId,
    required this.type,
    this.categoryId,
    this.transferId,
    required this.valor,
    required this.date,
    this.notes,
    required this.createdAt,
  });

  Map<String, Object?> toMap() {
    return {
      'id': id,
      'bank_account_id': bankAccountId,
      'transaction_type': type.value,
      'category_id': categoryId,
      'transfer_id': transferId,
      'valor': valor,
      'transaction_date': date,
      'notes': notes,
      'created_at': createdAt.toIso8601String(),
    };
  }

  factory GeneralTransaction.fromMap(Map<String, Object?> map) {
    return GeneralTransaction(
      id: map['id'] as int,
      bankAccountId: map['bank_account_id'] as int,
      type: GeneralTransactionType.fromValue(map['transaction_type'] as String),
      categoryId: map['category_id'] as int?,
      transferId: map['transfer_id'] as int?,
      valor: (map['valor'] as num).toDouble(),
      date: map['transaction_date'] as String,
      notes: map['notes'] as String?,
      createdAt: DateTime.parse(map['created_at'] as String),
    );
  }
}

/// Só `receita` e `despesa` são lançados à mão; transferências, parcelas de
/// dívida e empréstimos do desktop ficam fora do mobile.
enum GeneralTransactionType {
  receita,
  despesa;

  String get label => this == receita ? 'Receita' : 'Despesa';
}

/// Lançamento de uma conta — espelha
/// `desktop/src-tauri/src/entity/general_transaction.rs`, sem categoria nem
/// `transfer_id`. `valor` é sempre positivo; o sinal vem do tipo
/// (`signedValor` em `domain/general_finance.dart`).
class GeneralTransaction {
  final int? id;
  final int bankAccountId;
  final GeneralTransactionType type;
  final double valor;

  /// `YYYY-MM-DD`; a comparação com "hoje" é de texto, como no desktop.
  final String date;
  final String? notes;
  final DateTime createdAt;

  const GeneralTransaction({
    this.id,
    required this.bankAccountId,
    required this.type,
    required this.valor,
    required this.date,
    this.notes,
    required this.createdAt,
  });

  Map<String, Object?> toMap() {
    return {
      'id': id,
      'bank_account_id': bankAccountId,
      'transaction_type': type.name,
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
      type: GeneralTransactionType.values.byName(
        map['transaction_type'] as String,
      ),
      valor: (map['valor'] as num).toDouble(),
      date: map['transaction_date'] as String,
      notes: map['notes'] as String?,
      createdAt: DateTime.parse(map['created_at'] as String),
    );
  }
}

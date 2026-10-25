/// Categoria de lançamento — espelha
/// `desktop/src-tauri/src/entity/general_transaction_category.rs`, sem
/// `workspace_id`. `limiteMensal` é o teto de despesa do mês, na moeda base.
class GeneralTransactionCategory {
  final int? id;
  final String nome;
  final double? limiteMensal;
  final DateTime createdAt;

  const GeneralTransactionCategory({
    this.id,
    required this.nome,
    this.limiteMensal,
    required this.createdAt,
  });

  Map<String, Object?> toMap() {
    return {
      'id': id,
      'nome': nome,
      'limite_mensal': limiteMensal,
      'created_at': createdAt.toIso8601String(),
    };
  }

  factory GeneralTransactionCategory.fromMap(Map<String, Object?> map) {
    return GeneralTransactionCategory(
      id: map['id'] as int,
      nome: map['nome'] as String,
      limiteMensal: (map['limite_mensal'] as num?)?.toDouble(),
      createdAt: DateTime.parse(map['created_at'] as String),
    );
  }
}

/// Conta bancária em qualquer moeda — espelha
/// `desktop/src-tauri/src/entity/bank_account.rs`, sem `workspace_id` (o
/// mobile não tem Workspace) e sem os campos de carteira espelhada e
/// rendimento automático. O saldo nunca é gravado: sai dos lançamentos.
class BankAccount {
  final int? id;
  final String nome;
  final String titular;

  /// Código ISO do catálogo (`currency_catalog.dart`), em maiúsculas.
  final String currency;
  final DateTime createdAt;

  const BankAccount({
    this.id,
    required this.nome,
    required this.titular,
    required this.currency,
    required this.createdAt,
  });

  /// Nome com a moeda quando não é o real, como a lista de contas do desktop.
  String get label => currency == 'BRL' ? nome : '$nome ($currency)';

  Map<String, Object?> toMap() {
    return {
      'id': id,
      'nome': nome,
      'titular': titular,
      'currency': currency,
      'created_at': createdAt.toIso8601String(),
    };
  }

  factory BankAccount.fromMap(Map<String, Object?> map) {
    return BankAccount(
      id: map['id'] as int,
      nome: map['nome'] as String,
      titular: map['titular'] as String,
      currency: map['currency'] as String,
      createdAt: DateTime.parse(map['created_at'] as String),
    );
  }
}

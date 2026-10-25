import '../domain/loan.dart';

/// Devedor de empréstimo concedido — espelha
/// `desktop/src-tauri/src/entity/borrower.rs`, sem `workspace_id`.
class Borrower {
  final int? id;
  final String nome;
  final String? notes;
  final DateTime createdAt;

  const Borrower({
    this.id,
    required this.nome,
    this.notes,
    required this.createdAt,
  });

  Map<String, Object?> toMap() => {
    'id': id,
    'nome': nome,
    'notes': notes,
    'created_at': createdAt.toIso8601String(),
  };

  factory Borrower.fromMap(Map<String, Object?> map) => Borrower(
    id: map['id'] as int,
    nome: map['nome'] as String,
    notes: map['notes'] as String?,
    createdAt: DateTime.parse(map['created_at'] as String),
  );
}

/// Empréstimo concedido — espelha `desktop/src-tauri/src/entity/loan.rs`. Com
/// conta, a saída do principal vira o lançamento `emprestimo_concedido`
/// (`generalTransactionId`) e a moeda é a da conta; sem conta, a moeda é
/// escolhida e nada mexe em saldo.
class Loan {
  final int? id;
  final int borrowerId;
  final int? bankAccountId;
  final int? generalTransactionId;
  final double principal;

  /// Percentual mensal como digitado (1.5 = 1,5% a.m.); `null` = sem juros.
  final double? taxaMensalPercentual;

  /// `YYYY-MM-DD`.
  final String dataInicio;
  final bool includeInNetWorth;
  final String currency;
  final String? notes;
  final DateTime createdAt;

  const Loan({
    this.id,
    required this.borrowerId,
    this.bankAccountId,
    this.generalTransactionId,
    required this.principal,
    this.taxaMensalPercentual,
    required this.dataInicio,
    required this.includeInNetWorth,
    required this.currency,
    this.notes,
    required this.createdAt,
  });

  Map<String, Object?> toMap() => {
    'id': id,
    'borrower_id': borrowerId,
    'bank_account_id': bankAccountId,
    'general_transaction_id': generalTransactionId,
    'principal': principal,
    'taxa_mensal_percentual': taxaMensalPercentual,
    'data_inicio': dataInicio,
    'include_in_net_worth': includeInNetWorth ? 1 : 0,
    'currency': currency,
    'notes': notes,
    'created_at': createdAt.toIso8601String(),
  };

  factory Loan.fromMap(Map<String, Object?> map) => Loan(
    id: map['id'] as int,
    borrowerId: map['borrower_id'] as int,
    bankAccountId: map['bank_account_id'] as int?,
    generalTransactionId: map['general_transaction_id'] as int?,
    principal: (map['principal'] as num).toDouble(),
    taxaMensalPercentual: (map['taxa_mensal_percentual'] as num?)?.toDouble(),
    dataInicio: map['data_inicio'] as String,
    includeInNetWorth: (map['include_in_net_worth'] as int) != 0,
    currency: map['currency'] as String,
    notes: map['notes'] as String?,
    createdAt: DateTime.parse(map['created_at'] as String),
  );
}

/// Baixa manual — espelha `desktop/src-tauri/src/entity/loan_repayment.rs`.
/// `valorRecebido` é o dinheiro que entrou (0 = perdão) e `valorBaixado` o
/// que abate do saldo, independente do recebido.
class LoanRepayment {
  final int? id;
  final int loanId;
  final int? bankAccountId;
  final int? generalTransactionId;
  final String data;
  final double valorRecebido;
  final double valorBaixado;
  final String? notes;
  final DateTime createdAt;

  const LoanRepayment({
    this.id,
    required this.loanId,
    this.bankAccountId,
    this.generalTransactionId,
    required this.data,
    required this.valorRecebido,
    required this.valorBaixado,
    this.notes,
    required this.createdAt,
  });

  Map<String, Object?> toMap() => {
    'id': id,
    'loan_id': loanId,
    'bank_account_id': bankAccountId,
    'general_transaction_id': generalTransactionId,
    'data': data,
    'valor_recebido': valorRecebido,
    'valor_baixado': valorBaixado,
    'notes': notes,
    'created_at': createdAt.toIso8601String(),
  };

  factory LoanRepayment.fromMap(Map<String, Object?> map) => LoanRepayment(
    id: map['id'] as int,
    loanId: map['loan_id'] as int,
    bankAccountId: map['bank_account_id'] as int?,
    generalTransactionId: map['general_transaction_id'] as int?,
    data: map['data'] as String,
    valorRecebido: (map['valor_recebido'] as num).toDouble(),
    valorBaixado: (map['valor_baixado'] as num).toDouble(),
    notes: map['notes'] as String?,
    createdAt: DateTime.parse(map['created_at'] as String),
  );
}

/// Empréstimo com o devedor e as baixas; os números derivados saem de `hoje`.
class LoanWithRepayments {
  final Loan loan;
  final Borrower borrower;
  final List<LoanRepayment> repayments;

  const LoanWithRepayments(this.loan, this.borrower, this.repayments);

  double interestOn(DateTime hoje) {
    final inicio = DateTime.tryParse(loan.dataInicio);
    // Data corrompida: sem juros, em vez de derrubar a lista.
    if (inicio == null) return 0;
    return accruedInterest(
      principal: loan.principal,
      taxaMensalPercentual: loan.taxaMensalPercentual,
      dataInicio: inicio,
      hoje: DateTime(hoje.year, hoje.month, hoje.day),
    );
  }

  double get totalWrittenOff =>
      repayments.fold<double>(0, (sum, r) => sum + r.valorBaixado);

  double get extra => extraReceived([
    for (final r in repayments)
      (recebido: r.valorRecebido, baixado: r.valorBaixado),
  ]);

  double balanceOn(DateTime hoje) => loanOutstandingBalance(
    loan.principal,
    interestOn(hoje),
    repayments.map((r) => r.valorBaixado),
  );
}

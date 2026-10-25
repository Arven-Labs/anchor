import '../domain/liability.dart';

/// Dívida com parcelas — espelha `desktop/src-tauri/src/entity/liability.rs`,
/// sem `workspace_id`, ativo vinculado nem indexador. As parcelas viram
/// lançamentos `parcela_divida` na conta (todos de uma vez, com data de
/// vencimento) e o detalhe de juros e amortização fica em `LiabilityInstallment`.
class Liability {
  final int? id;
  final int bankAccountId;
  final String nome;
  final String titular;
  final double principal;

  /// Percentual mensal como digitado (1.5 = 1,5% a.m.).
  final double taxaPercentual;
  final int prazoMeses;
  final AmortizationSystem sistema;

  /// `YYYY-MM-DD`; a parcela 1 vence um mês depois.
  final String dataInicio;
  final DateTime createdAt;

  const Liability({
    this.id,
    required this.bankAccountId,
    required this.nome,
    required this.titular,
    required this.principal,
    required this.taxaPercentual,
    required this.prazoMeses,
    required this.sistema,
    required this.dataInicio,
    required this.createdAt,
  });

  Map<String, Object?> toMap() {
    return {
      'id': id,
      'bank_account_id': bankAccountId,
      'nome': nome,
      'titular': titular,
      'principal': principal,
      'taxa_percentual': taxaPercentual,
      'prazo_meses': prazoMeses,
      'sistema_amortizacao': sistema.value,
      'data_inicio': dataInicio,
      'created_at': createdAt.toIso8601String(),
    };
  }

  factory Liability.fromMap(Map<String, Object?> map) {
    return Liability(
      id: map['id'] as int,
      bankAccountId: map['bank_account_id'] as int,
      nome: map['nome'] as String,
      titular: map['titular'] as String,
      principal: (map['principal'] as num).toDouble(),
      taxaPercentual: (map['taxa_percentual'] as num).toDouble(),
      prazoMeses: map['prazo_meses'] as int,
      sistema: AmortizationSystem.fromValue(
        map['sistema_amortizacao'] as String,
      ),
      dataInicio: map['data_inicio'] as String,
      createdAt: DateTime.parse(map['created_at'] as String),
    );
  }
}

/// Dívida com todas as suas parcelas, ordenadas pelo número.
class LiabilityWithInstallments {
  final Liability liability;
  final List<LiabilityInstallment> installments;

  const LiabilityWithInstallments(this.liability, this.installments);

  /// Saldo devedor de `hoje` (`YYYY-MM-DD`).
  double balanceOn(String hoje) => outstandingBalance(
    liability.principal,
    [
      for (final i in installments)
        (
          dataVencimento: i.dataVencimento,
          valorAmortizacao: i.valorAmortizacao,
        ),
    ],
    hoje,
  );

  /// Parcelas já vencidas em `hoje`.
  int paidOn(String hoje) =>
      installments.where((i) => i.dataVencimento.compareTo(hoje) <= 0).length;

  /// Próxima parcela a vencer depois de `hoje`; `null` quando quitada.
  LiabilityInstallment? nextAfter(String hoje) {
    for (final i in installments) {
      if (i.dataVencimento.compareTo(hoje) > 0) return i;
    }
    return null;
  }
}

/// Uma parcela gerada: o lançamento de caixa é o `general_transactions`
/// apontado por `generalTransactionId`; aqui fica a divisão em juros e
/// amortização.
class LiabilityInstallment {
  final int? id;
  final int liabilityId;
  final int generalTransactionId;
  final int numeroParcela;
  final double valorParcela;
  final double valorJuros;
  final double valorAmortizacao;
  final double saldoDevedorApos;

  /// `YYYY-MM-DD`.
  final String dataVencimento;

  const LiabilityInstallment({
    this.id,
    required this.liabilityId,
    required this.generalTransactionId,
    required this.numeroParcela,
    required this.valorParcela,
    required this.valorJuros,
    required this.valorAmortizacao,
    required this.saldoDevedorApos,
    required this.dataVencimento,
  });

  Map<String, Object?> toMap() {
    return {
      'id': id,
      'liability_id': liabilityId,
      'general_transaction_id': generalTransactionId,
      'numero_parcela': numeroParcela,
      'valor_parcela': valorParcela,
      'valor_juros': valorJuros,
      'valor_amortizacao': valorAmortizacao,
      'saldo_devedor_apos': saldoDevedorApos,
      'data_vencimento': dataVencimento,
    };
  }

  factory LiabilityInstallment.fromMap(Map<String, Object?> map) {
    return LiabilityInstallment(
      id: map['id'] as int,
      liabilityId: map['liability_id'] as int,
      generalTransactionId: map['general_transaction_id'] as int,
      numeroParcela: map['numero_parcela'] as int,
      valorParcela: (map['valor_parcela'] as num).toDouble(),
      valorJuros: (map['valor_juros'] as num).toDouble(),
      valorAmortizacao: (map['valor_amortizacao'] as num).toDouble(),
      saldoDevedorApos: (map['saldo_devedor_apos'] as num).toDouble(),
      dataVencimento: map['data_vencimento'] as String,
    );
  }
}

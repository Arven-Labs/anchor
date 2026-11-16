// Cobertura de vendas do histórico de um ativo, porta mínima de `cost_basis::replay`
// (`desktop/src-tauri/src/domain/cost_basis.rs`) e de `ensure_history_not_worse`
// (`desktop/src-tauri/src/commands/transaction.rs`, ADR 0004).
//
// O mobile não tem desdobramento nem bonificação, então a cobertura é só a quantidade em
// carteira: compra soma, venda e come-cotas baixam. Transferência de custódia, provento,
// aporte, retirada e amortização não mexem na quantidade total do ativo.

import '../models/portfolio_transaction.dart';
import 'cost_basis.dart';

/// A mudança deixaria uma venda acima da quantidade em carteira na data dela.
class HistoryCoverageException implements Exception {
  const HistoryCoverageException();

  @override
  String toString() => 'HistoryCoverageException: the change would leave a sale uncovered';
}

/// Lançamento ainda sem id (o que está sendo criado) ordena depois de todos do mesmo dia.
const _newRowId = 1 << 62;

int _byDayThenId(PortfolioTransaction a, PortfolioTransaction b) {
  final dayA = DateTime.utc(a.date.year, a.date.month, a.date.day);
  final dayB = DateTime.utc(b.date.year, b.date.month, b.date.day);
  final byDay = dayA.compareTo(dayB);
  return byDay != 0 ? byDay : (a.id ?? _newRowId).compareTo(b.id ?? _newRowId);
}

/// `true` se alguma venda (ou come-cotas) passa da quantidade em carteira na data dela.
/// Ordena por dia e, no mesmo dia, pelo id, como o desktop. A posição nunca fica negativa
/// (a venda descoberta baixa só o que havia), igual a `CostPosition.take`.
bool hasUncoveredSale(Iterable<PortfolioTransaction> transactions) {
  // Desempate pela posição de entrada (o `List.sort` do Dart não é estável): linhas novas, sem
  // id, no mesmo dia, valem na ordem em que vieram (a ordem do arquivo no import).
  final indexed = transactions.toList().asMap().entries.toList()
    ..sort((a, b) {
      final byDayThenId = _byDayThenId(a.value, b.value);
      return byDayThenId != 0 ? byDayThenId : a.key.compareTo(b.key);
    });
  var held = 0.0;
  for (final tx in indexed.map((e) => e.value)) {
    final quantity = tx.quantity ?? 0;
    switch (tx.type) {
      case TransactionType.compra:
        held += quantity;
      case TransactionType.venda:
      case TransactionType.comeCotas:
        if (held + costEps < quantity) return true;
        held -= quantity;
        if (held <= costEps) held = 0;
      default:
        break;
    }
  }
  return false;
}

/// A mudança piora a cobertura: o histórico de depois tem venda descoberta e o de antes
/// não tinha. Histórico que já estava descoberto antes não bloqueia nada.
bool worsensCoverage({
  required Iterable<PortfolioTransaction> before,
  required Iterable<PortfolioTransaction> after,
}) => !hasUncoveredSale(before) && hasUncoveredSale(after);

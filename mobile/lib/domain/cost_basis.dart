// Custo médio corrente de uma posição, porta de `take_from_position` de
// `desktop/src-tauri/src/domain/cost_basis.rs`. Compartilhado pelo IR de cripto
// e pelo de exterior: a venda baixa o custo médio (o "preço médio" da tela de
// posições não baixa, por isso o cálculo do IR é próprio).

const costEps = 1e-9;

class CostPosition {
  double quantity = 0;
  double cost = 0;

  /// Compra: soma a quantidade e o custo (valor + taxa, já na moeda da apuração).
  void add(double quantity, double cost) {
    this.quantity += quantity;
    this.cost += cost;
  }

  /// Baixa `quantity` ao custo médio. Devolve (custo, quantidade realmente
  /// coberta pelo histórico). Posição zerada recomeça do zero na próxima compra.
  (double, double) take(double quantity) {
    final available = this.quantity > 0 ? this.quantity : 0.0;
    final covered = quantity < available ? quantity : available;
    final average = this.quantity > costEps ? cost / this.quantity : 0.0;
    final taken = average * covered;
    this.quantity -= covered;
    cost -= taken;
    if (this.quantity <= costEps) {
      this.quantity = 0;
      cost = 0;
    }
    return (taken, covered);
  }
}

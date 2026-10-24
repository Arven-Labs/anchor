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

  /// Amortização de fundo (Lei 14.754, art. 17): as cotas ficam e o custo cai na
  /// fração `valor ÷ (cota × cotas)`, no máximo todo ele. Posição vazia ou sem valor
  /// não muda. Devolve o custo devolvido (porta do `Amortize` de `cost_basis.rs`).
  double amortize(double value, double quotaValue) {
    final marketValue = quantity * quotaValue;
    if (quantity <= costEps || marketValue <= costEps) return 0;
    final fraction = value / marketValue < 1 ? value / marketValue : 1.0;
    final removed = cost * fraction;
    cost -= removed;
    return removed;
  }
}

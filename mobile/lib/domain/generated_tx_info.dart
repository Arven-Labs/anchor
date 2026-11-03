/// Quem gerou um lançamento da conta (ADR 0001): o repositório não grava a frase no
/// banco, só o vínculo; a tela monta o texto no idioma do aparelho.
enum GeneratedTxKind { installment, loanGranted, loanRepayment }

class GeneratedTxInfo {
  final GeneratedTxKind kind;

  /// Nome da dívida (parcela) ou do devedor (empréstimo e baixa), como o usuário digitou.
  final String name;
  final int? installmentNumber;
  final int? installmentTotal;

  const GeneratedTxInfo({
    required this.kind,
    required this.name,
    this.installmentNumber,
    this.installmentTotal,
  });
}

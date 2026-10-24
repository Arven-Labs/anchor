// Compensação de prejuízo entre fundos abertos. Porta de
// `desktop/src-tauri/src/domain/tax/fund_compensation.rs`. A perda apurada num resgate
// abate os rendimentos de resgates e de come-cotas POSTERIORES, no mesmo fundo ou em
// outro da mesma administradora (ou do mesmo intermediário por conta e ordem), desde
// que da mesma classificação (longo prazo, curto prazo; fundos de ações à parte). Não é
// retroativa (IN RFB 1.585/2015, art. 15).
//
// Premissas (todas a conferir na Receita; as regras vêm de fontes secundárias):
// - Grupo = (administradora normalizada, tipo do fundo). Fundo sem administradora
//   informada fica isolado: compensa só consigo.
// - Perda de um resgate = soma, por lote, da perda legal medida contra o custo
//   atualizado (cotas × max(0, custo atualizado − cota do resgate), IN 1.585 art. 15
//   §1) ou de `max(0, IOF − rendimento nominal)`, o que for maior (ver
//   `fund_redemption`). Os lotes seguem independentes, como em
//   `estimateRedemptions`: ganho e perda do mesmo resgate não se anulam.
// - O crédito de um resgate é o das perdas de datas ESTRITAMENTE anteriores do grupo
//   ("posteriores", na lei); no mesmo dia não compensa.
// - O crédito abate o rendimento dos lotes do resgate, do mais antigo ao mais novo, mas
//   só até zerar o IR a pagar do lote: o que o come-cotas já cobriu não gasta crédito.
//   O IOF não é abatido.
// - O saldo do grupo abate o ganho da estimativa do próximo come-cotas (fundos em ordem
//   de ticker, sem usar o mesmo crédito duas vezes).
// - Descontos de come-cotas JÁ registrados: o usuário informa (campo opcional da
//   transação) quanto rendimento o crédito de prejuízo cobriu ali, e isso abate o saldo
//   do grupo na data (no máximo o saldo). Sem o valor (nulo), o app não sabe quanto a
//   administradora abateu e só avisa (`CreditNote`, `informed == null`) quando o grupo
//   tinha saldo de perdas de datas estritamente anteriores: o saldo mostrado pode estar
//   acima do real. Valor informado acima do saldo também avisa (`informed` com o
//   valor). Valor informado dentro do saldo, inclusive 0, não avisa.
// - O crédito não tem validade modelada (as fontes divergem sobre o prazo).

import '../models/fund.dart';
import 'come_cotas.dart';
import 'cost_basis.dart' show costEps;
import 'fund_redemption.dart';

/// Um fundo na compensação: quem é, a que grupo pertence e os resgates já estimados.
class CompFund {
  final int assetId;
  final String ticker;
  final FundTerm term;
  final String? administrator;
  final List<RedemptionEstimate> redemptions;

  /// Os descontos de come-cotas já registrados do fundo.
  final List<ComeCotasMark> comeCotas;

  const CompFund({
    required this.assetId,
    required this.ticker,
    required this.term,
    required this.administrator,
    required this.redemptions,
    this.comeCotas = const [],
  });
}

/// Um desconto de come-cotas registrado: a data e o crédito de prejuízo que o usuário
/// disse ter sido usado nele.
class ComeCotasMark {
  final DateTime date;

  /// Rendimento coberto pelo crédito, em BRL; `null` = não informado (0 = nenhum
  /// crédito usado).
  final double? lossCreditUsed;

  const ComeCotasMark(this.date, [this.lossCreditUsed]);
}

/// Aviso de um come-cotas registrado em que o crédito de prejuízo do grupo pode não
/// estar certo.
class CreditNote {
  final int assetId;

  /// Data do come-cotas registrado.
  final DateTime date;

  /// Saldo do grupo imediatamente antes da data (perdas de datas anteriores, já
  /// descontado o que resgates gastaram), em BRL.
  final double balance;

  /// `null`: o usuário não informou o crédito usado e o grupo tinha saldo (a
  /// administradora pode ter usado parte). Valor: o que ele informou, acima do saldo.
  final double? informed;

  const CreditNote({
    required this.assetId,
    required this.date,
    required this.balance,
    this.informed,
  });
}

/// Saldo abaixo disto não gera aviso (ruído de arredondamento).
const _noteMinBalance = 0.005;

/// O saldo de prejuízo a compensar de um grupo.
class LossCredit {
  /// A administradora como o usuário escreveu; `null` = fundo sem administradora
  /// (grupo de um fundo só).
  final String? administrator;
  final FundTerm term;
  final List<String> tickers;
  final List<int> assetIds;

  /// Rendimento ainda abatível, em BRL.
  final double amount;

  const LossCredit({
    required this.administrator,
    required this.term,
    required this.tickers,
    required this.assetIds,
    required this.amount,
  });
}

/// O resultado de [compensate]: os resgates de cada fundo (na ordem de entrada) já
/// abatidos, o saldo de cada grupo e os avisos de come-cotas registrados feitos com
/// saldo no grupo (em ordem de data).
class CompensationResult {
  final List<List<RedemptionEstimate>> redemptions;
  final List<LossCredit> credits;
  final List<CreditNote> notes;

  const CompensationResult({
    required this.redemptions,
    required this.credits,
    this.notes = const [],
  });
}

/// Administradora sem espaços sobrando e em minúsculas; vazia vira `null`.
String? normalizeAdministrator(String? raw) {
  final collapsed = (raw ?? '').trim().split(RegExp(r'\s+')).join(' ');
  return collapsed.isEmpty ? null : collapsed.toLowerCase();
}

/// Administradora como se grava: só os espaços colapsados, sem mudar a caixa.
String? collapseAdministrator(String? raw) {
  final collapsed = (raw ?? '').trim().split(RegExp(r'\s+')).join(' ');
  return collapsed.isEmpty ? null : collapsed;
}

String _groupKey(CompFund fund) {
  final administrator = normalizeAdministrator(fund.administrator);
  return administrator != null
      ? 'adm:$administrator|${fund.term.name}'
      : 'asset:${fund.assetId}|${fund.term.name}';
}

class _Pool {
  final String? administrator;
  final FundTerm term;
  final tickers = <String>{};
  final assetIds = <int>[];
  double amount = 0;

  _Pool(this.administrator, this.term);
}

class _Item {
  final DateTime date;
  final int txId;
  final int fund;
  final int index;

  const _Item(this.date, this.txId, this.fund, this.index);
}

int _compareItems(_Item a, _Item b) {
  final byDate = a.date.compareTo(b.date);
  if (byDate != 0) return byDate;
  final byTx = a.txId.compareTo(b.txId);
  if (byTx != 0) return byTx;
  final byFund = a.fund.compareTo(b.fund);
  return byFund != 0 ? byFund : a.index.compareTo(b.index);
}

/// Abate o IR a pagar dos lotes do resgate com o crédito disponível. Devolve o resgate
/// novo e quanto usou (o original, se nada foi usado).
(RedemptionEstimate, double) _applyCredit(
  RedemptionEstimate redemption,
  double available,
) {
  var left = available;
  var used = 0.0;
  final lots = <LotPart>[];
  for (final lot in redemption.lots) {
    if (left <= costEps || lot.rate <= 0 || lot.ir <= costEps) {
      lots.add(lot);
      continue;
    }
    // Rendimento que ainda gera IR a pagar; abater mais que isso não reduz nada.
    final needed = lot.ir / lot.rate;
    final cut = needed < left ? needed : left;
    final ir = lot.ir - cut * lot.rate;
    lots.add(lot.copyWith(ir: ir > 0 ? ir : 0));
    left -= cut;
    used += cut;
  }
  if (used <= costEps) return (redemption, 0);
  return (redemption.withLossCredit(lots: lots, lossCreditUsed: used), used);
}

/// Compensa os prejuízos entre os resgates de [funds] (todos os fundos que têm tipo,
/// mesmo sem resgate) e devolve os resgates abatidos e o saldo de cada grupo.
CompensationResult compensate(List<CompFund> funds) {
  final keys = [for (final fund in funds) _groupKey(fund)];
  final pools = <String, _Pool>{};
  for (var i = 0; i < funds.length; i++) {
    final fund = funds[i];
    final pool = pools.putIfAbsent(
      keys[i],
      () => _Pool(collapseAdministrator(fund.administrator), fund.term),
    );
    pool.tickers.add(fund.ticker);
    pool.assetIds.add(fund.assetId);
  }

  final out = [for (final fund in funds) List.of(fund.redemptions)];
  // (data, id da transação, fundo, resgate): o crédito de um dia só vale a partir do
  // dia seguinte.
  final order = <_Item>[
    for (var f = 0; f < funds.length; f++)
      for (var r = 0; r < funds[f].redemptions.length; r++)
        _Item(funds[f].redemptions[r].date, funds[f].redemptions[r].txId, f, r),
  ]..sort(_compareItems);

  // (data, fundo, crédito informado) de cada come-cotas registrado. O saldo de uma data
  // é o de antes do bloco dessa data: só perdas de datas estritamente anteriores
  // contam, como no crédito dos resgates. O crédito informado sai do saldo (no máximo o
  // saldo) e um valor acima do saldo, ou nenhum valor com saldo no grupo, gera aviso.
  final cc = <(DateTime, int, double?)>[
    for (var f = 0; f < funds.length; f++)
      for (final mark in funds[f].comeCotas) (mark.date, f, mark.lossCreditUsed),
  ]..sort((a, b) {
      final byDate = a.$1.compareTo(b.$1);
      return byDate != 0 ? byDate : a.$2.compareTo(b.$2);
    });
  final notes = <CreditNote>[];
  var nextCc = 0;
  void noteUpTo(DateTime? limit) {
    while (nextCc < cc.length &&
        (limit == null || !cc[nextCc].$1.isAfter(limit))) {
      final (date, f, used) = cc[nextCc];
      final pool = pools[keys[f]]!;
      final balance = pool.amount;
      if (used == null) {
        if (balance > _noteMinBalance) {
          notes.add(
            CreditNote(assetId: funds[f].assetId, date: date, balance: balance),
          );
        }
      } else {
        if (used > balance + _noteMinBalance) {
          notes.add(
            CreditNote(
              assetId: funds[f].assetId,
              date: date,
              balance: balance,
              informed: used,
            ),
          );
        }
        pool.amount = balance - used > 0 ? balance - used : 0;
      }
      nextCc++;
    }
  }

  var start = 0;
  while (start < order.length) {
    final date = order[start].date;
    var end = start;
    while (end < order.length && order[end].date == date) {
      end++;
    }
    noteUpTo(date);
    for (final item in order.sublist(start, end)) {
      final pool = pools[keys[item.fund]]!;
      final (updated, used) = _applyCredit(
        out[item.fund][item.index],
        pool.amount,
      );
      out[item.fund][item.index] = updated;
      pool.amount -= used;
    }
    for (final item in order.sublist(start, end)) {
      pools[keys[item.fund]]!.amount += out[item.fund][item.index].lossCreated;
    }
    start = end;
  }
  noteUpTo(null);

  return CompensationResult(
    notes: notes,
    redemptions: out,
    credits: [
      for (final pool in pools.values)
        if (pool.amount > costEps)
          LossCredit(
            administrator: pool.administrator,
            term: pool.term,
            tickers: (pool.tickers.toList()..sort()),
            assetIds: pool.assetIds,
            amount: pool.amount,
          ),
    ],
  );
}

/// Abate o ganho das estimativas de come-cotas com o saldo dos grupos em [credits].
/// Estimativas sem imposto, sem grupo com saldo ou de fundo de ações (isento) ficam
/// como estão.
void applyLossCredit(
  List<ComeCotasEstimate> estimates,
  List<LossCredit> credits,
) {
  final remaining = [for (final credit in credits) credit.amount];
  final order = [for (var i = 0; i < estimates.length; i++) i]
    ..sort((a, b) => estimates[a].ticker.compareTo(estimates[b].ticker));
  for (final i in order) {
    final estimate = estimates[i];
    if (estimate.status != EstimateStatus.estimated ||
        estimate.tax <= costEps ||
        estimate.rate <= 0) {
      continue;
    }
    final group = credits.indexWhere(
      (credit) => credit.assetIds.contains(estimate.assetId),
    );
    if (group < 0) continue;
    final cut = estimate.gain < remaining[group]
        ? estimate.gain
        : remaining[group];
    if (cut <= costEps) continue;
    remaining[group] -= cut;
    estimate.lossCreditUsed = cut;
    estimate.gain -= cut;
    estimate.tax = estimate.gain * estimate.rate;
    final quota = estimate.quotaValue;
    estimate.quotasDeducted = quota != null && quota > 0
        ? estimate.tax / quota
        : 0;
  }
}

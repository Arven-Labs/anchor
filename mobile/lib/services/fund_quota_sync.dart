// Valor da cota dos fundos lido do informe diário da CVM e gravado em
// `fund_quota_values` (`source = 'cvm'`). Porta de `finance_api/fund_quota_store.rs` do
// desktop, com duas diferenças pensadas no celular: só roda quando o usuário aperta o
// botão (sem espera nem intervalo entre buscas) e só olha a janela necessária à
// estimativa do come-cotas, não o histórico desde a 1ª compra. Só vão à rede os meses
// que faltam: o corrente e o anterior sempre, os fechados só se o fundo não tem cota da
// CVM naquele mês. Fundo fechado ou exclusivo com estoque de 2023 pede também dez/2023
// (a cota de 31/12/2023), um zip só. Uma cota lançada à mão nunca é sobrescrita.

import '../domain/come_cotas.dart' show lastReferenceDate;
import '../domain/cvm_inf_diario.dart';
import '../models/asset.dart';
import '../models/fund.dart';
import '../models/portfolio_transaction.dart';
import 'cvm_inf_diario_service.dart';
import 'portfolio_repository.dart';

enum CvmStatusKind {
  /// O fundo não tem CNPJ cadastrado.
  noCnpj,

  /// Ainda não buscou (nada rodou desde que o app abriu) e não há cota da CVM gravada.
  pending,
  ok,

  /// O informe não traz o CNPJ, ou traz sem cota usável para a subclasse escolhida.
  notFound,

  /// O CNPJ só tem subclasses no informe: o usuário escolhe uma.
  needsSubclass,
  error,
}

/// Como está a leitura da CVM de um fundo.
class CvmStatus {
  final CvmStatusKind kind;

  /// As subclasses do informe (só em [CvmStatusKind.needsSubclass]).
  final List<String> options;
  final String? message;

  const CvmStatus(this.kind, {this.options = const [], this.message});
}

DateTime _day(DateTime date) => DateTime.utc(date.year, date.month, date.day);

/// Os meses que a estimativa do come-cotas precisa, dadas as datas de cota da CVM já
/// gravadas do fundo: o corrente e o anterior (a cota mais nova) e os que cobrem os 45
/// dias antes da última data de come-cotas (a base do ganho). Os meses de permeio
/// ficam de fora: são ~12 MB cada e nada da estimativa os usa.
///
/// [extra] são meses de que o fundo ainda precisa fora dessa janela (hoje: dez/2023, a
/// cota do estoque). Entram só se já fecharam, caírem a partir da primeira publicação
/// mensal e o fundo ainda não tiver cota da CVM naquele mês.
List<YearMonth> cvmMonthsToRead(
  DateTime today,
  Iterable<DateTime> storedCvmDates, {
  List<YearMonth> extra = const [],
}) {
  final now = _day(today);
  final lastReference = lastReferenceDate(now);
  final baseSince = lastReference.subtract(
    const Duration(days: cvmLookbackDaysBeforeComeCotas),
  );
  // `DateTime.utc` normaliza o mês 0 para dezembro do ano anterior.
  final previousMonth = DateTime.utc(now.year, now.month - 1, 1);
  final stored = {for (final d in storedCvmDates) (d.year, d.month)};
  final months = {
    // A base: fechado só se faltar cota (a recência é de hoje, não do come-cotas).
    for (final month in monthsBetween(baseSince, lastReference))
      if (isRecentMonth(month, now) || !stored.contains(month)) month,
    ...planMonths(previousMonth, now, storedCvmDates),
    for (final month in extra)
      if (!_before(month, cvmFirstMonth) &&
          !_before((now.year, now.month), month) &&
          !stored.contains(month))
        month,
  }.toList()
    ..sort((a, b) => a.$1 != b.$1 ? a.$1.compareTo(b.$1) : a.$2.compareTo(b.$2));
  return months;
}

bool _before(YearMonth a, YearMonth b) =>
    a.$1 < b.$1 || (a.$1 == b.$1 && a.$2 < b.$2);

/// O mês da cota do estoque de 2023 (31/12/2023, ou até 10 dias antes).
const _estoqueMonth = (2023, 12);

/// O fundo tem estoque de 2023 a estimar (fechado ou exclusivo, de longo ou curto prazo,
/// com a opção de estoque informada) e já tinha cotas em 31/12/2023 (primeira compra até
/// essa data, ou sem compra): então precisa da cota de dez/2023, que fica fora da janela
/// do come-cotas. Mesma regra de `wants_estoque_quota` do desktop.
bool wantsEstoqueQuota({
  required FundTerm? term,
  required FundStructure? structure,
  required EstoqueOption? estoqueOption,
  required DateTime? firstBuy,
}) {
  final hasEstoque =
      structure != null &&
      structure != FundStructure.open &&
      (term == FundTerm.longTerm || term == FundTerm.shortTerm);
  final heldIn2023 = firstBuy == null || !_day(firstBuy).isAfter(_estoqueDate);
  return hasEstoque && estoqueOption != null && heldIn2023;
}

final _estoqueDate = DateTime.utc(2023, 12, 31);

class FundQuotaSync {
  final PortfolioRepository _repository;
  final CvmInfDiarioService _service;
  final DateTime Function() _now;

  FundQuotaSync({
    PortfolioRepository? repository,
    CvmInfDiarioService? service,
    DateTime Function()? now,
  }) : _repository = repository ?? PortfolioRepository(),
       _service = service ?? CvmInfDiarioService(),
       _now = now ?? DateTime.now;

  /// Lê a CVM para todos os fundos com CNPJ e devolve o estado de cada um (por id do
  /// ativo). Falha de rede não lança: vira [CvmStatusKind.error] nos fundos que
  /// ficaram sem leitura, e o que já foi lido fica gravado. [onProgress] recebe
  /// (meses lidos, meses a ler).
  Future<Map<int, CvmStatus>> refresh({
    void Function(int done, int total)? onProgress,
  }) async {
    final today = _day(_now());
    final subclasses = await _repository.listFundSubclasses();
    final terms = await _repository.listFundTerms();
    final structures = await _repository.listFundStructures();
    final estoqueOptions = await _repository.listFundEstoqueOptions();
    final firstBuys = <int, DateTime>{};
    for (final tx in await _repository.listAllTransactions()) {
      final assetId = tx.assetId;
      if (assetId == null || tx.type != TransactionType.compra) continue;
      final current = firstBuys[assetId];
      if (current == null || tx.date.isBefore(current)) firstBuys[assetId] = tx.date;
    }
    final funds = [
      for (final asset in await _repository.listAssets())
        if (asset.assetClass == AssetClass.fundo && asset.id != null) asset,
    ];

    final statuses = <int, CvmStatus>{};
    final targets = <(Asset, CvmTarget)>[];
    final monthsByFund = <int, List<YearMonth>>{};
    for (final fund in funds) {
      final digits = normalizeCnpj(fund.cnpj ?? '');
      if (digits.length != 14) {
        statuses[fund.id!] = const CvmStatus(CvmStatusKind.noCnpj);
        continue;
      }
      targets.add((fund, CvmTarget(digits, subclasses[fund.id])));
      monthsByFund[fund.id!] = cvmMonthsToRead(
        today,
        await _repository.listCvmQuotaDates(fund.id!),
        extra: wantsEstoqueQuota(
              term: terms[fund.id],
              structure: structures[fund.id],
              estoqueOption: estoqueOptions[fund.id],
              firstBuy: firstBuys[fund.id],
            )
            ? const [_estoqueMonth]
            : const [],
      );
    }
    if (targets.isEmpty) return statuses;

    final months = {...monthsByFund.values.expand((m) => m)}.toList()
      ..sort((a, b) => a.$1 != b.$1 ? a.$1.compareTo(b.$1) : a.$2.compareTo(b.$2));
    final quotas = {for (final t in targets) t.$1.id!: <DateTime, double>{}};
    final seenSubclasses = <String, Set<String>>{};
    String? failure;

    onProgress?.call(0, months.length);
    for (var i = 0; i < months.length; i++) {
      final month = months[i];
      // Só os fundos que precisam deste mês.
      final needing = [
        for (var t = 0; t < targets.length; t++)
          if (monthsByFund[targets[t].$1.id!]!.contains(month)) t,
      ];
      try {
        final extract = await _service.extractMonth(month, [
          for (final t in needing) targets[t].$2,
        ]);
        if (extract != null) {
          for (var n = 0; n < needing.length; n++) {
            final (fund, target) = targets[needing[n]];
            quotas[fund.id!]!.addAll(extract.quotas[n]);
            final seen = extract.subclasses[target.cnpj];
            if (seen != null) {
              seenSubclasses.putIfAbsent(target.cnpj, () => {}).addAll(seen);
            }
          }
        }
      } on CvmUnavailableException catch (e) {
        failure = e.detail;
        break;
      }
      onProgress?.call(i + 1, months.length);
    }

    for (final (fund, target) in targets) {
      final id = fund.id!;
      await _repository.upsertCvmQuotas(id, quotas[id]!);
      final hasStored = (await _repository.listCvmQuotaDates(id)).isNotEmpty;
      final options = (seenSubclasses[target.cnpj] ?? {}).toList()..sort();
      if (failure != null) {
        // O que foi lido antes da falha já está gravado; o erro avisa que faltou.
        statuses[id] = CvmStatus(CvmStatusKind.error, message: failure);
      } else if (hasStored) {
        statuses[id] = const CvmStatus(CvmStatusKind.ok);
      } else if (target.subclass == null && options.isNotEmpty) {
        statuses[id] = CvmStatus(
          CvmStatusKind.needsSubclass,
          options: options,
        );
      } else {
        statuses[id] = const CvmStatus(CvmStatusKind.notFound);
      }
    }
    return statuses;
  }
}

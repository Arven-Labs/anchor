import '../domain/come_cotas.dart';
import '../domain/fund_compensation.dart' show LossCredit, RecordNote;
import '../domain/fund_redemption.dart';
import '../domain/fund_tax.dart';
import '../domain/number_locale.dart';
import '../domain/tax_format.dart';
import '../models/fund.dart';
import '../services/fund_quota_sync.dart';
import 'app_localizations.dart';

// Textos do IR de fundos (ADR 0001). As frases do domínio ficam aqui, não na tela; um
// `switch` sem `default` nos enums faz o analisador acusar um valor novo sem texto.

String _brl(AppLocalizations l, double value) => formatBrl(value, locale: l.localeName);
String _day(AppLocalizations l, DateTime date) => formatDay(date, locale: l.localeName);

/// Situação do come-cotas de um fundo numa frase.
String comeCotasStatusText(AppLocalizations l, ComeCotasEstimate estimate) =>
    switch (estimate.status) {
      EstimateStatus.noPosition => l.fundStatusNoPosition,
      EstimateStatus.noTerm => l.fundStatusNoTerm,
      EstimateStatus.noQuota => l.fundStatusNoQuota,
      EstimateStatus.exempt => l.fundStatusExempt,
      EstimateStatus.estimated => l.fundStatusEstimated(
        _day(l, estimate.referenceDate),
        _brl(l, estimate.tax),
        formatFixed(estimate.quotasDeducted, 4, locale: l.localeName),
      ),
    };

/// Como a base do ganho foi escolhida; vazio fora da estimativa.
String comeCotasBasisText(AppLocalizations l, ComeCotasEstimate estimate) {
  final basis = estimate.basisPerQuota;
  if (estimate.status != EstimateStatus.estimated || basis == null) return '';
  final origin = switch (estimate.basisSource) {
    BasisSource.lastComeCotas => l.fundBasisLastComeCotas,
    BasisSource.estoque => l.fundBasisEstoque,
    _ => l.fundBasisAverageCost,
  };
  return l.fundBasis(
    _brl(l, basis),
    origin,
    _brl(l, estimate.gain),
    formatFixed(estimate.rate * 100, 1, locale: l.localeName),
  );
}

/// O estoque de 2023 (Lei 14.754, arts. 27 e 28) numa frase; vazio quando o fundo não tem
/// estoque.
String estoqueText(AppLocalizations l, FundReport report) {
  final estoque = report.estoque;
  switch (report.estoqueStatus) {
    case EstoqueStatus.notApplicable:
      return '';
    case EstoqueStatus.notTaxed:
      return l.fundEstoqueNotTaxed;
    case EstoqueStatus.needsQuota:
      return l.fundEstoqueNeedsQuota;
    case EstoqueStatus.estimated:
      if (estoque == null) return '';
      return l.fundEstoqueEstimated(
        _brl(l, estoque.base),
        _day(l, estoque.date),
        _brl(l, estoque.quotaValue),
        formatFixed(estoque.rate * 100, 0, locale: l.localeName),
        _brl(l, estoque.tax),
      );
  }
}

/// O come-cotas pago acima do IR devido, somado; vazio sem excesso. A lei não prevê
/// restituição (IN RFB 1.585/2015, art. 15; Lei 14.754/2023, art. 17, §6): é imposto pago
/// sobre rendimento que depois sumiu e volta como perda a compensar.
String refundableText(AppLocalizations l, FundReport report) {
  final total = report.refundableComeCotas;
  if (total <= 0.005) return '';
  return l.fundRefundable(_brl(l, total));
}

/// Avisos do cartão do fundo, em frases.
List<String> fundWarnings(AppLocalizations l, FundReport report) {
  final estimate = report.estimate;
  return [
    if (estimate.staleQuota) l.fundWarnStaleQuota(staleQuotaDays),
    if (estimate.lastReferenceUnrecorded != null)
      l.fundWarnUnrecordedDeduction(_day(l, estimate.lastReferenceUnrecorded!)),
    for (final note in report.creditNotes)
      if (note.informed == null)
        l.fundWarnCreditNotInformed(_day(l, note.date), _brl(l, note.balance))
      else
        l.fundWarnCreditExceedsBalance(
          _day(l, note.date),
          _brl(l, note.informed!),
          _brl(l, note.balance),
        ),
    for (final redemption in report.redemptions) ...[
      if (redemption.uncoveredQuotas > 1e-9)
        l.fundWarnRedemptionUncovered(_day(l, redemption.date)),
      if (redemption.excessComeCotas > 0.005)
        l.fundWarnRedemptionExcess(
          _day(l, redemption.date),
          _brl(l, redemption.excessComeCotas),
        ),
    ],
  ];
}

/// Quanto do ganho da estimativa o crédito de prejuízo já abateu; vazio sem crédito.
String comeCotasCreditText(AppLocalizations l, ComeCotasEstimate estimate) {
  if (estimate.lossCreditUsed <= 0.005) return '';
  return l.fundCreditReduced(
    _brl(l, estimate.lossCreditUsed),
    _brl(l, estimate.taxBeforeCompensation),
  );
}

/// O que o resgate tem de compensação, uma frase por linha; vazio sem prejuízo nem crédito.
List<String> redemptionCompensationLines(AppLocalizations l, RedemptionEstimate redemption) {
  return [
    if (redemption.lossCreditUsed > 0.005)
      l.fundRedemptionCreditLine(
        _brl(l, redemption.lossCreditUsed),
        _brl(l, redemption.ir),
        _brl(l, redemption.irBeforeCompensation),
      ),
    if (redemption.lossCreated > 0.005) l.fundRedemptionLossLine(_brl(l, redemption.lossCreated)),
  ];
}

/// Uma linha por grupo com prejuízo ainda a compensar.
String lossCreditText(AppLocalizations l, LossCredit credit) => l.fundLossCreditLine(
  _brl(l, credit.amount),
  credit.tickers.join(', '),
  credit.administrator ?? l.fundNoAdministrator,
  fundTermName(l, credit.term),
);

/// O aviso de guarda do registro das perdas de uma administradora que o usuário zerou: a lei
/// só obriga a administradora a guardar o registro até o fim do ano seguinte (IN RFB 1.585,
/// art. 15 §2) e não limita o direito de compensar.
String recordNoteText(AppLocalizations l, RecordNote note) {
  final head = l.fundRecordHead(note.administrator, _day(l, note.redeemedOn));
  final until = _day(l, note.guaranteedUntil);
  final balance = _brl(l, note.balance);
  return note.expired
      ? l.fundRecordExpired(head, balance, until)
      : l.fundRecordActive(head, balance, until);
}

/// Estado da leitura da CVM de um fundo numa frase; vazio se não há o que dizer.
String cvmStatusText(AppLocalizations l, CvmStatus? status, DateTime? lastCvmDate) {
  final latest = lastCvmDate == null ? '' : l.fundCvmLatest(_day(l, lastCvmDate));
  if (status == null) {
    return lastCvmDate == null ? '' : l.fundCvmRead(latest);
  }
  return switch (status.kind) {
    CvmStatusKind.noCnpj => l.fundCvmNoCnpj,
    CvmStatusKind.pending => l.fundCvmPending(latest),
    CvmStatusKind.ok => l.fundCvmRead(latest),
    CvmStatusKind.notFound => l.fundCvmNotFound,
    CvmStatusKind.needsSubclass => l.fundCvmNeedsSubclass,
    CvmStatusKind.error => l.fundCvmError(status.message ?? l.fundCvmErrorDefault, latest),
  };
}

String fundTermName(AppLocalizations l, FundTerm term) => switch (term) {
  FundTerm.longTerm => l.fundTermLongTerm,
  FundTerm.shortTerm => l.fundTermShortTerm,
  FundTerm.equity => l.fundTermEquity,
  FundTerm.investmentEntity => l.fundTermInvestmentEntity,
};

String fundStructureName(AppLocalizations l, FundStructure structure) => switch (structure) {
  FundStructure.open => l.fundStructureOpen,
  FundStructure.closed => l.fundStructureClosed,
  FundStructure.exclusive => l.fundStructureExclusive,
};

String estoqueOptionName(AppLocalizations l, EstoqueOption option) => switch (option) {
  EstoqueOption.general => l.fundEstoqueGeneral,
  EstoqueOption.option8 => l.fundEstoqueOption8,
};

String fundSettingErrorText(AppLocalizations l, FundSettingException error) => switch (error.code) {
  FundSettingErrorCode.needTermBeforeStructure => l.fundErrNeedTermBeforeStructure,
  FundSettingErrorCode.needTermBeforeEstoque => l.fundErrNeedTermBeforeEstoque,
  FundSettingErrorCode.needTermBeforeAdministrator => l.fundErrNeedTermBeforeAdministrator,
  FundSettingErrorCode.needTermBeforeSubclass => l.fundErrNeedTermBeforeSubclass,
  FundSettingErrorCode.cnpjMustHave14Digits => l.fundErrCnpjMustHave14Digits,
};

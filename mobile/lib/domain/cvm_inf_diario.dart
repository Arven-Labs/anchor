// CVM, informe diário de fundos (`dados.cvm.gov.br/dados/FI/DOC/INF_DIARIO`): o valor
// da cota de cada fundo em cada dia útil. Porta de `finance_core/cvm_inf_diario.rs` e
// de `plan_months` de `finance_api/fund_quota_store.rs` do desktop; aqui só a parte
// pura (meses, CNPJ e leitura do CSV). O download está em
// `services/cvm_inf_diario_service.dart`.
//
// Conferido nos arquivos reais (2026-10-23):
// - Um zip por mês (`inf_diario_fi_YYYYMM.zip`, de 202101 em diante, ~12 MB e ~35 MB
//   de CSV). Antes de 2021 só há zips anuais em `HIST/`, que não são lidos.
// - CSV `;` com CRLF e dois cabeçalhos: o antigo (`TP_FUNDO;CNPJ_FUNDO;DT_COMPTC;...`)
//   e, desde a Resolução CVM 175, `TP_FUNDO_CLASSE;CNPJ_FUNDO_CLASSE;ID_SUBCLASSE;
//   DT_COMPTC;...`. As colunas são lidas por nome.
// - Um mesmo CNPJ tem uma linha por subclasse em cada dia, cada uma com a sua cota; a
//   linha da classe tem `ID_SUBCLASSE` vazio. Quem chama escolhe a subclasse (ou a
//   classe) por fundo.

/// O primeiro mês publicado em arquivo mensal.
const cvmFirstMonth = (2021, 1);

/// Janela do mobile: o necessário para a estimativa do come-cotas é a cota mais nova e
/// a cota até tantos dias antes da última data de come-cotas (mesma janela de
/// `come_cotas.dart`).
const cvmLookbackDaysBeforeComeCotas = 45;

/// CNPJ só com os dígitos.
String normalizeCnpj(String cnpj) => cnpj.replaceAll(RegExp(r'\D'), '');

/// `00.017.024/0001-53` a partir de 14 dígitos.
String formatCnpj(String digits) =>
    '${digits.substring(0, 2)}.${digits.substring(2, 5)}.${digits.substring(5, 8)}'
    '/${digits.substring(8, 12)}-${digits.substring(12, 14)}';

typedef YearMonth = (int, int);

bool _after(YearMonth a, YearMonth b) =>
    a.$1 > b.$1 || (a.$1 == b.$1 && a.$2 > b.$2);

YearMonth _next(YearMonth month) =>
    month.$2 == 12 ? (month.$1 + 1, 1) : (month.$1, month.$2 + 1);

/// Os meses de [from] a [to], sem passar do primeiro mês publicado.
List<YearMonth> monthsBetween(DateTime from, DateTime to) {
  var current = (from.year, from.month);
  if (_after(cvmFirstMonth, current)) current = cvmFirstMonth;
  final last = (to.year, to.month);
  final out = <YearMonth>[];
  while (!_after(current, last)) {
    out.add(current);
    current = _next(current);
  }
  return out;
}

/// O mês é o corrente ou o anterior a [today]? Esses ainda recebem dias novos e
/// retificações.
bool isRecentMonth(YearMonth month, DateTime today) {
  final previous = today.month == 1
      ? (today.year - 1, 12)
      : (today.year, today.month - 1);
  return !_after(previous, month);
}

/// Os meses a ler para cobrir [since]..[today], dadas as datas de cota da CVM já
/// gravadas do fundo: o corrente e o anterior sempre, os fechados só se o fundo não
/// tem nenhuma cota da CVM naquele mês.
List<YearMonth> planMonths(
  DateTime since,
  DateTime today,
  Iterable<DateTime> storedCvmDates,
) {
  final stored = {for (final d in storedCvmDates) (d.year, d.month)};
  return [
    for (final month in monthsBetween(since, today))
      if (isRecentMonth(month, today) || !stored.contains(month)) month,
  ];
}

/// O que procurar no informe: um fundo (CNPJ só com dígitos) e a subclasse (`null` = a
/// linha da classe).
class CvmTarget {
  final String cnpj;
  final String? subclass;

  const CvmTarget(this.cnpj, [this.subclass]);
}

/// O que um mês do informe tem para os alvos pedidos.
class CvmExtract {
  /// Por alvo (na mesma ordem): as cotas positivas do mês.
  final List<Map<DateTime, double>> quotas;

  /// Por CNPJ: as subclasses (não vazias) que o informe traz no mês.
  final Map<String, Set<String>> subclasses = {};

  /// Os CNPJs que têm alguma linha no mês, com ou sem cota usável.
  final Set<String> found = {};

  CvmExtract(int targets) : quotas = [for (var i = 0; i < targets; i++) {}];
}

DateTime? _parseDate(String text) {
  final match = RegExp(r'^(\d{4})-(\d{2})-(\d{2})$').firstMatch(text.trim());
  if (match == null) return null;
  return DateTime.utc(
    int.parse(match[1]!),
    int.parse(match[2]!),
    int.parse(match[3]!),
  );
}

/// Lê o CSV do informe uma linha por vez (o arquivo tem ~35 MB e não cabe inteiro numa
/// lista de linhas no celular) e guarda só as linhas dos CNPJs pedidos. Lança
/// [FormatException] se faltar uma coluna essencial no cabeçalho.
class CvmQuotaExtractor {
  final List<CvmTarget> targets;
  final CvmExtract result;
  final Map<String, List<int>> _byCnpj = {};

  bool _headerSeen = false;
  int _cnpjCol = -1, _dateCol = -1, _quotaCol = -1, _subclassCol = -1;

  CvmQuotaExtractor(this.targets) : result = CvmExtract(targets.length) {
    for (var i = 0; i < targets.length; i++) {
      _byCnpj.putIfAbsent(targets[i].cnpj, () => []).add(i);
    }
  }

  void _readHeader(List<String> fields) {
    // Um BOM UTF-8 lido como latin-1 vira "ï»¿" e grudaria na primeira coluna.
    final headers = [
      for (final h in fields)
        h.replaceFirst('\u{ef}\u{bb}\u{bf}', '').replaceFirst('\u{feff}', '').trim(),
    ];
    int column(List<String> names) => headers.indexWhere(names.contains);

    _cnpjCol = column(['CNPJ_FUNDO_CLASSE', 'CNPJ_FUNDO']);
    _dateCol = column(['DT_COMPTC']);
    _quotaCol = column(['VL_QUOTA']);
    _subclassCol = column(['ID_SUBCLASSE']);
    if (_cnpjCol < 0) {
      throw const FormatException(
        'CVM informe diário: falta a coluna CNPJ_FUNDO_CLASSE',
      );
    }
    if (_dateCol < 0) {
      throw const FormatException('CVM informe diário: falta a coluna DT_COMPTC');
    }
    if (_quotaCol < 0) {
      throw const FormatException('CVM informe diário: falta a coluna VL_QUOTA');
    }
  }

  /// Uma linha do CSV, sem o CRLF.
  void addLine(String line) {
    if (line.isEmpty) return;
    final fields = line.split(';');
    if (!_headerSeen) {
      _headerSeen = true;
      _readHeader(fields);
      return;
    }

    String? at(int column) =>
        column >= 0 && column < fields.length ? fields[column] : null;

    final rawCnpj = at(_cnpjCol);
    if (rawCnpj == null) return;
    final cnpj = normalizeCnpj(rawCnpj);
    final indices = _byCnpj[cnpj];
    if (indices == null) return;

    final subclass = at(_subclassCol)?.trim() ?? '';
    if (subclass.isNotEmpty) {
      result.subclasses.putIfAbsent(cnpj, () => {}).add(subclass);
    }
    result.found.add(cnpj);

    final date = _parseDate(at(_dateCol) ?? '');
    final value = double.tryParse((at(_quotaCol) ?? '').trim());
    if (date == null || value == null || !value.isFinite || value <= 0) return;
    for (final i in indices) {
      if ((targets[i].subclass ?? '') == subclass) result.quotas[i][date] = value;
    }
  }
}

/// [CvmQuotaExtractor] sobre linhas já em memória (testes e arquivos pequenos).
CvmExtract extractQuotas(Iterable<String> lines, List<CvmTarget> targets) {
  final extractor = CvmQuotaExtractor(targets);
  if (targets.isEmpty) return extractor.result;
  lines.forEach(extractor.addLine);
  return extractor.result;
}

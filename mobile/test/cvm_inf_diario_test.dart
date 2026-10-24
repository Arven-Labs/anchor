import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/cvm_inf_diario.dart';

// Porta dos testes de `finance_core/cvm_inf_diario.rs` e `plan_months` do desktop.
void main() {
  DateTime d(String s) => DateTime.parse('${s}T00:00:00Z');

  const newHeader =
      'TP_FUNDO_CLASSE;CNPJ_FUNDO_CLASSE;ID_SUBCLASSE;DT_COMPTC;VL_TOTAL;VL_QUOTA;VL_PATRIM_LIQ;CAPTC_DIA;RESG_DIA;NR_COTST';
  const oldHeader =
      'TP_FUNDO;CNPJ_FUNDO;DT_COMPTC;VL_TOTAL;VL_QUOTA;VL_PATRIM_LIQ;CAPTC_DIA;RESG_DIA;NR_COTST';
  const classCnpj = '00017024000153';
  const subclassCnpj = '00888897000131';

  // Linhas copiadas do informe de 2026-10 (`inf_diario_fi_202610.csv`).
  final newCsv = [
    newHeader,
    'CLASSES - FIF;00.017.024/0001-53;;2026-10-01;1119426.37;44.990762600000;1221524.87;0.00;2902.97;1',
    'CLASSES - FIF;00.017.024/0001-53;;2026-10-02;1120004.10;45.009590600000;1222036.06;0.00;0.00;1',
    'CLASSES - FIF;00.888.897/0001-31;RBMFN1747320951;2026-10-01;47114895.69;74.383553275471;47091275.92;74000.00;17000.00;1',
    'CLASSES - FIF;00.888.897/0001-31;MZMRC1747322915;2026-10-01;700260258.28;74.040455402933;700168039.98;971319.87;6732106.10;10909',
    'CLASSES - FIF;00.888.897/0001-31;RBMFN1747320951;2026-10-02;47315013.59;74.510349769592;47288549.16;165000.00;48000.00;1',
    'CLASSES - FIF;97.929.213/0001-34;;2026-10-20;70175226.87;15.909504423400;70122822.66;0.00;0.00;7',
  ];

  group('extractQuotas', () {
    test('lê a linha da classe pelo CNPJ e ignora os outros fundos', () {
      final out = extractQuotas(newCsv, [const CvmTarget(classCnpj)]);

      expect(out.quotas.single, {
        d('2026-10-01'): 44.9907626,
        d('2026-10-02'): 45.0095906,
      });
      expect(out.found, {classCnpj});
      expect(out.subclasses, isEmpty);
    });

    test('cada subclasse tem a sua cota e a classe não pega a de nenhuma', () {
      final out = extractQuotas(newCsv, [
        const CvmTarget(subclassCnpj, 'RBMFN1747320951'),
        const CvmTarget(subclassCnpj, 'MZMRC1747322915'),
        const CvmTarget(subclassCnpj),
      ]);

      expect(out.quotas[0].keys, [d('2026-10-01'), d('2026-10-02')]);
      expect(out.quotas[0][d('2026-10-01')], 74.383553275471);
      expect(out.quotas[1].keys, [d('2026-10-01')]);
      expect(out.quotas[2], isEmpty, reason: 'só tem subclasses');
      expect(out.subclasses[subclassCnpj], {
        'RBMFN1747320951',
        'MZMRC1747322915',
      });
      expect(out.found, {subclassCnpj});
    });

    test('lê o cabeçalho antigo (CNPJ_FUNDO, sem subclasse)', () {
      final out = extractQuotas([
        oldHeader,
        'FI;00.017.024/0001-53;2020-12-30;1119426.37;44.5;1221524.87;0.00;2902.97;1',
      ], [const CvmTarget(classCnpj)]);

      expect(out.quotas.single, {d('2020-12-30'): 44.5});
    });

    test('tira o BOM, mesmo lido como latin-1', () {
      for (final bom in ['\u{feff}', '\u{ef}\u{bb}\u{bf}']) {
        final out = extractQuotas([
          '$bom$newHeader',
          newCsv[1],
        ], [const CvmTarget(classCnpj)]);
        expect(out.quotas.single, hasLength(1), reason: 'BOM ${bom.runes}');
      }
    });

    test('cota zero, negativa, vazia ou ilegível e data ruim ficam de fora', () {
      final out = extractQuotas([
        newHeader,
        'CLASSES - FIF;00.017.024/0001-53;;2026-10-01;1;0;1;0;0;1',
        'CLASSES - FIF;00.017.024/0001-53;;2026-10-02;1;-3.5;1;0;0;1',
        'CLASSES - FIF;00.017.024/0001-53;;2026-10-05;1;;1;0;0;1',
        'CLASSES - FIF;00.017.024/0001-53;;2026-10-06;1;abc;1;0;0;1',
        'CLASSES - FIF;00.017.024/0001-53;;06/10/2026;1;12.5;1;0;0;1',
        'CLASSES - FIF;00.017.024/0001-53;;2026-10-07;1;12.5;1;0;0;1',
      ], [const CvmTarget(classCnpj)]);

      expect(out.quotas.single, {d('2026-10-07'): 12.5});
      expect(out.found, {classCnpj}, reason: 'o CNPJ existe, só as cotas não servem');
    });

    test('CNPJ ausente do mês não aparece em found', () {
      final out = extractQuotas(newCsv, [const CvmTarget('11111111000111')]);

      expect(out.quotas.single, isEmpty);
      expect(out.found, isEmpty);
    });

    test('sem alvo não lê nada, nem o cabeçalho', () {
      expect(extractQuotas(['lixo'], const []).quotas, isEmpty);
    });

    test('linhas curtas, em branco e CRLF sobrando não quebram', () {
      final out = extractQuotas([
        newHeader,
        '',
        'CLASSES - FIF;00.017.024/0001-53',
        'CLASSES - FIF;00.017.024/0001-53;;2026-10-01;1;45.5;1;0;0;1',
      ], [const CvmTarget(classCnpj)]);

      expect(out.quotas.single, {d('2026-10-01'): 45.5});
    });

    test('cabeçalho sem as colunas essenciais é erro', () {
      expect(
        () => extractQuotas(['A;B;C', '1;2;3'], [const CvmTarget(classCnpj)]),
        throwsFormatException,
      );
      expect(
        () => extractQuotas(
          ['CNPJ_FUNDO;DT_COMPTC;OUTRA', '1;2;3'],
          [const CvmTarget(classCnpj)],
        ),
        throwsFormatException,
      );
    });
  });

  group('meses', () {
    test('monthsBetween vai do mês inicial ao final, virando o ano', () {
      expect(monthsBetween(d('2026-11-15'), d('2027-02-03')), [
        (2026, 11),
        (2026, 12),
        (2027, 1),
        (2027, 2),
      ]);
      expect(monthsBetween(d('2026-10-31'), d('2026-10-01')), [(2026, 10)],
          reason: 'é por mês, não por dia');
      expect(monthsBetween(d('2026-11-01'), d('2026-10-31')), isEmpty);
    });

    test('monthsBetween não passa do primeiro mês publicado', () {
      final months = monthsBetween(d('2019-05-01'), d('2021-03-10'));

      expect(months.first, (2021, 1));
      expect(months.last, (2021, 3));
    });

    test('o mês corrente e o anterior são os recentes', () {
      final today = d('2026-10-23');
      expect(isRecentMonth((2026, 10), today), isTrue);
      expect(isRecentMonth((2026, 9), today), isTrue);
      expect(isRecentMonth((2026, 8), today), isFalse);
      // em janeiro, o anterior é dezembro
      expect(isRecentMonth((2026, 12), d('2027-01-05')), isTrue);
      expect(isRecentMonth((2026, 11), d('2027-01-05')), isFalse);
    });

    test('planMonths relê sempre os recentes e os fechados só se faltar cota', () {
      final today = d('2026-10-23');
      final since = d('2026-07-01');

      expect(planMonths(since, today, const []), [
        (2026, 7),
        (2026, 8),
        (2026, 9),
        (2026, 10),
      ]);
      // julho e setembro já têm cota da CVM: julho (fechado) sai, setembro (anterior) fica
      expect(
        planMonths(since, today, [d('2026-07-10'), d('2026-09-10')]),
        [(2026, 8), (2026, 9), (2026, 10)],
      );
    });
  });

  group('CNPJ', () {
    test('normaliza para os dígitos e formata de volta', () {
      expect(normalizeCnpj('00.017.024/0001-53'), classCnpj);
      expect(normalizeCnpj(' 00017024000153 '), classCnpj);
      expect(formatCnpj(classCnpj), '00.017.024/0001-53');
    });
  });
}

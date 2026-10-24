import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/cvm_inf_diario.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/fund.dart';
import 'package:anchor_mobile/services/cvm_inf_diario_service.dart';
import 'package:anchor_mobile/services/fund_quota_sync.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

/// Serviço da CVM sem rede: devolve o CSV combinado de cada mês (ou `null` = 404) e
/// anota os meses pedidos.
class FakeCvm extends CvmInfDiarioService {
  final Map<YearMonth, List<String>> csvByMonth;
  final Set<YearMonth> failing;
  final requested = <YearMonth>[];

  FakeCvm(this.csvByMonth, {this.failing = const {}});

  @override
  Future<CvmExtract?> extractMonth(
    YearMonth month,
    List<CvmTarget> targets,
  ) async {
    requested.add(month);
    if (failing.contains(month)) throw CvmUnavailableException('HTTP 503');
    final lines = csvByMonth[month];
    return lines == null ? null : extractQuotas(lines, targets);
  }
}

void main() {
  DateTime d(String s) => DateTime.parse('${s}T00:00:00Z');

  const header =
      'TP_FUNDO_CLASSE;CNPJ_FUNDO_CLASSE;ID_SUBCLASSE;DT_COMPTC;VL_TOTAL;VL_QUOTA;VL_PATRIM_LIQ;CAPTC_DIA;RESG_DIA;NR_COTST';
  String row(String cnpj, String sub, String date, String quota) =>
      'CLASSES - FIF;$cnpj;$sub;$date;1;$quota;1;0;0;1';
  const cnpj = '00.017.024/0001-53';
  const subCnpj = '00.888.897/0001-31';

  final repository = PortfolioRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-db-test-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<int> addFund({String? fundCnpj = cnpj, FundTerm? term}) async {
    final id = await repository.insertAsset(
      Asset(
        ticker: 'FUNDO',
        name: 'Fundo Teste',
        assetClass: AssetClass.fundo,
        currency: 'BRL',
        createdAt: DateTime(2026, 1, 1),
        cnpj: fundCnpj,
      ),
    );
    if (term != null) await repository.setFundTerm(id, term);
    return id;
  }

  FundQuotaSync sync(FakeCvm cvm) => FundQuotaSync(
    repository: repository,
    service: cvm,
    now: () => DateTime(2026, 10, 23, 15),
  );

  Future<Map<String, double>> quotasOf(int id, {String? source}) async {
    final all = await repository.listFundQuotas(id);
    return {
      for (final q in all)
        if (source == null || q.source == source)
          '${q.date.year}-${q.date.month.toString().padLeft(2, '0')}-'
                  '${q.date.day.toString().padLeft(2, '0')}':
              q.value,
    };
  }

  group('meses a ler', () {
    test('o corrente, o anterior e os 45 dias antes do último come-cotas', () {
      // hoje 23/10/2026: último come-cotas 29/05/2026; base desde 14/04
      expect(cvmMonthsToRead(d('2026-10-23'), const []), [
        (2026, 4),
        (2026, 5),
        (2026, 9),
        (2026, 10),
      ]);
    });

    test('mês fechado que já tem cota da CVM não é lido de novo, os recentes sim', () {
      expect(
        cvmMonthsToRead(d('2026-10-23'), [d('2026-04-20'), d('2026-09-10')]),
        [(2026, 5), (2026, 9), (2026, 10)],
      );
    });

    test('em janeiro o mês anterior é dezembro', () {
      // último come-cotas 30/11/2026; base desde 16/10
      expect(cvmMonthsToRead(d('2027-01-10'), const []), [
        (2026, 10),
        (2026, 11),
        (2026, 12),
        (2027, 1),
      ]);
    });
  });

  group('banco', () {
    test('uma instalação nova já nasce com as colunas da CVM', () async {
      final id = await addFund();
      await repository.setFundTerm(id, FundTerm.longTerm);
      await repository.setFundCvm(id, cnpj: cnpj, subclass: 'ABC');

      final asset = (await repository.listAssets()).single;
      expect(asset.cnpj, cnpj);
      expect(await repository.listFundSubclasses(), {id: 'ABC'});
    });

    test('migra do v9 sem perder o fundo, o tipo e as cotas', () async {
      final path = '${dir.path}/anchor_mobile.db';
      final old = await openDatabase(
        path,
        version: 9,
        onCreate: (db, _) async {
          await db.execute('''
            CREATE TABLE assets (
              id INTEGER PRIMARY KEY AUTOINCREMENT, ticker TEXT NOT NULL,
              name TEXT NOT NULL, asset_class TEXT NOT NULL, currency TEXT NOT NULL,
              created_at TEXT NOT NULL, external_id TEXT, equity_shares_owned REAL,
              equity_total_shares REAL, equity_company_valuation REAL)''');
          await db.execute('''
            CREATE TABLE fund_details (
              asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
              term TEXT NOT NULL)''');
          await db.execute('''
            CREATE TABLE fund_quota_values (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
              quota_date TEXT NOT NULL, value REAL NOT NULL,
              source TEXT NOT NULL DEFAULT 'manual', notes TEXT,
              created_at TEXT NOT NULL)''');
          await db.insert('assets', {
            'ticker': 'FUNDO',
            'name': 'Fundo Velho',
            'asset_class': 'fundo',
            'currency': 'BRL',
            'created_at': '2026-01-01T00:00:00.000',
          });
          await db.insert('fund_details', {'asset_id': 1, 'term': 'longo_prazo'});
          await db.insert('fund_quota_values', {
            'asset_id': 1,
            'quota_date': '2026-09-30',
            'value': 12.5,
            'created_at': '2026-10-01T00:00:00.000',
          });
        },
      );
      await old.close();

      final asset = (await repository.listAssets()).single;
      expect(asset.name, 'Fundo Velho');
      expect(asset.cnpj, isNull);
      expect(await repository.getFundTerm(1), FundTerm.longTerm);
      expect(await quotasOf(1), {'2026-09-30': 12.5});
      // as colunas novas funcionam
      await repository.setFundCvm(1, cnpj: cnpj, subclass: 'ABC');
      expect((await repository.listAssets()).single.cnpj, cnpj);
      expect(await repository.listFundSubclasses(), {1: 'ABC'});
      await repository.setFundAdministrator(1, 'XP');
      expect(await repository.listFundAdministrators(), {1: 'XP'});
      // e o tipo e a subclasse seguem lá depois de gravar a administradora
      expect(await repository.getFundTerm(1), FundTerm.longTerm);
      expect(await repository.listFundSubclasses(), {1: 'ABC'});
    });

    test('migra do v10 sem perder o CNPJ, a subclasse e o tipo', () async {
      final old = await openDatabase(
        '${dir.path}/anchor_mobile.db',
        version: 10,
        onCreate: (db, _) async {
          await db.execute('''
            CREATE TABLE assets (
              id INTEGER PRIMARY KEY AUTOINCREMENT, ticker TEXT NOT NULL,
              name TEXT NOT NULL, asset_class TEXT NOT NULL, currency TEXT NOT NULL,
              created_at TEXT NOT NULL, external_id TEXT, equity_shares_owned REAL,
              equity_total_shares REAL, equity_company_valuation REAL, cnpj TEXT)''');
          await db.execute('''
            CREATE TABLE fund_details (
              asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
              term TEXT NOT NULL, cvm_subclass TEXT)''');
          await db.insert('assets', {
            'ticker': 'FUNDO',
            'name': 'Fundo v10',
            'asset_class': 'fundo',
            'currency': 'BRL',
            'created_at': '2026-01-01T00:00:00.000',
            'cnpj': cnpj,
          });
          await db.insert('fund_details', {
            'asset_id': 1,
            'term': 'curto_prazo',
            'cvm_subclass': 'ABC',
          });
        },
      );
      await old.close();

      expect((await repository.listAssets()).single.cnpj, cnpj);
      expect(await repository.getFundTerm(1), FundTerm.shortTerm);
      expect(await repository.listFundSubclasses(), {1: 'ABC'});
      expect(await repository.listFundAdministrators(), isEmpty);
      await repository.setFundAdministrator(1, 'BTG');
      expect(await repository.listFundAdministrators(), {1: 'BTG'});
    });

    test('migra do v8: cria as tabelas de fundo já com a subclasse e o CNPJ', () async {
      final old = await openDatabase(
        '${dir.path}/anchor_mobile.db',
        version: 8,
        onCreate: (db, _) async {
          await db.execute('''
            CREATE TABLE assets (
              id INTEGER PRIMARY KEY AUTOINCREMENT, ticker TEXT NOT NULL,
              name TEXT NOT NULL, asset_class TEXT NOT NULL, currency TEXT NOT NULL,
              created_at TEXT NOT NULL, external_id TEXT, equity_shares_owned REAL,
              equity_total_shares REAL, equity_company_valuation REAL)''');
        },
      );
      await old.close();

      final id = await addFund(term: FundTerm.shortTerm);
      await repository.setFundCvm(id, cnpj: cnpj, subclass: 'XYZ');
      await repository.setFundAdministrator(id, 'XP');
      expect(await repository.listFundAdministrators(), {id: 'XP'});
      expect(await repository.listFundSubclasses(), {id: 'XYZ'});
      expect((await repository.listAssets()).single.cnpj, cnpj);
    });
  });

  group('setFundAdministrator', () {
    test('exige o tipo do fundo antes', () async {
      final id = await addFund();

      await expectLater(
        repository.setFundAdministrator(id, 'XP'),
        throwsA(isA<FormatException>()),
      );
      await repository.setFundTerm(id, FundTerm.longTerm);
      await repository.setFundAdministrator(id, 'XP');
      expect(await repository.listFundAdministrators(), {id: 'XP'});
    });

    test('guarda o texto com os espaços colapsados e vazio apaga', () async {
      final id = await addFund(term: FundTerm.longTerm);

      await repository.setFundAdministrator(id, '  XP   Investimentos ');
      expect(await repository.listFundAdministrators(), {id: 'XP Investimentos'});

      await repository.setFundAdministrator(id, '   ');
      expect(await repository.listFundAdministrators(), isEmpty);
    });

    test('trocar o tipo do fundo não apaga a administradora nem a subclasse', () async {
      final id = await addFund(term: FundTerm.longTerm);
      await repository.setFundAdministrator(id, 'XP');
      await repository.setFundCvm(id, cnpj: cnpj, subclass: 'ABC');

      await repository.setFundTerm(id, FundTerm.shortTerm);

      expect(await repository.listFundAdministrators(), {id: 'XP'});
      expect(await repository.listFundSubclasses(), {id: 'ABC'});
      expect(await repository.getFundTerm(id), FundTerm.shortTerm);
    });
  });

  group('setFundCvm', () {
    test('guarda o CNPJ formatado e aceita só 14 dígitos', () async {
      final id = await addFund(fundCnpj: null);

      await repository.setFundCvm(id, cnpj: '00017024000153');
      expect((await repository.listAssets()).single.cnpj, cnpj);

      expect(
        () => repository.setFundCvm(id, cnpj: '123'),
        throwsA(isA<FormatException>()),
      );
      await repository.setFundCvm(id, cnpj: '  ');
      expect((await repository.listAssets()).single.cnpj, isNull);
    });

    test('a subclasse exige o tipo do fundo escolhido antes', () async {
      final id = await addFund();

      await expectLater(
        repository.setFundCvm(id, cnpj: cnpj, subclass: 'ABC'),
        throwsA(isA<FormatException>()),
      );
      await repository.setFundTerm(id, FundTerm.longTerm);
      await repository.setFundCvm(id, cnpj: cnpj, subclass: 'ABC');
      expect(await repository.listFundSubclasses(), {id: 'ABC'});
    });

    test('trocar o CNPJ ou a subclasse apaga só as cotas da CVM', () async {
      final id = await addFund(term: FundTerm.longTerm);
      await repository.setFundQuota(assetId: id, date: d('2026-10-01'), value: 10);
      await repository.upsertCvmQuotas(id, {d('2026-10-02'): 11});

      // mesmos valores: nada muda
      await repository.setFundCvm(id, cnpj: cnpj);
      expect(await quotasOf(id), {'2026-10-01': 10, '2026-10-02': 11});

      await repository.setFundCvm(id, cnpj: subCnpj);
      expect(await quotasOf(id), {'2026-10-01': 10});

      await repository.upsertCvmQuotas(id, {d('2026-10-03'): 12});
      await repository.setFundCvm(id, cnpj: subCnpj, subclass: 'ABC');
      expect(await quotasOf(id), {'2026-10-01': 10});
    });
  });

  group('upsertCvmQuotas', () {
    test('troca a cota da CVM de uma data e nunca a lançada à mão', () async {
      final id = await addFund();
      await repository.setFundQuota(assetId: id, date: d('2026-10-01'), value: 99);
      await repository.upsertCvmQuotas(id, {
        d('2026-10-01'): 44,
        d('2026-10-02'): 45,
      });
      await repository.upsertCvmQuotas(id, {d('2026-10-02'): 46});

      expect(await quotasOf(id), {'2026-10-01': 99, '2026-10-02': 46});
      expect(await quotasOf(id, source: 'cvm'), {'2026-10-02': 46});
      expect(await repository.listCvmQuotaDates(id), [d('2026-10-02')]);
    });

    test('corrigir à mão uma cota da CVM a torna manual', () async {
      final id = await addFund();
      await repository.upsertCvmQuotas(id, {d('2026-10-02'): 45});

      await repository.setFundQuota(assetId: id, date: d('2026-10-02'), value: 47);

      expect(await quotasOf(id, source: 'manual'), {'2026-10-02': 47});
      expect(await repository.listCvmQuotaDates(id), isEmpty);
    });
  });

  group('refresh', () {
    test('lê só os meses necessários e grava as cotas como CVM', () async {
      final id = await addFund(term: FundTerm.longTerm);
      final cvm = FakeCvm({
        (2026, 5): [header, row(cnpj, '', '2026-05-28', '40.5')],
        (2026, 9): [header, row(cnpj, '', '2026-09-30', '44.1')],
        (2026, 10): [
          header,
          row(cnpj, '', '2026-10-01', '44.99'),
          row(subCnpj, '', '2026-10-01', '74.38'),
          row(cnpj, '', '2026-10-02', '45.01'),
        ],
      });

      final statuses = await sync(cvm).refresh();

      expect(cvm.requested, [(2026, 4), (2026, 5), (2026, 9), (2026, 10)]);
      expect(statuses[id]!.kind, CvmStatusKind.ok);
      expect(await quotasOf(id, source: 'cvm'), {
        '2026-05-28': 40.5,
        '2026-09-30': 44.1,
        '2026-10-01': 44.99,
        '2026-10-02': 45.01,
      });
      final latest = await repository.getLatestFundQuota(
        id,
        today: DateTime(2026, 10, 23),
      );
      expect(latest!.value, 45.01);
    });

    test('numa segunda leitura só os meses recentes voltam à rede', () async {
      final id = await addFund();
      final rows = {
        (2026, 4): [header, row(cnpj, '', '2026-04-20', '39')],
        (2026, 5): [header, row(cnpj, '', '2026-05-28', '40.5')],
        (2026, 9): [header, row(cnpj, '', '2026-09-30', '44.1')],
        (2026, 10): [header, row(cnpj, '', '2026-10-02', '45.01')],
      };
      await sync(FakeCvm(rows)).refresh();

      final second = FakeCvm(rows);
      await sync(second).refresh();

      expect(second.requested, [(2026, 9), (2026, 10)]);
      expect(await quotasOf(id, source: 'cvm'), hasLength(4));
    });

    test('a cota lançada à mão não é sobrescrita', () async {
      final id = await addFund();
      await repository.setFundQuota(assetId: id, date: d('2026-10-01'), value: 99);

      await sync(
        FakeCvm({
          (2026, 10): [
            header,
            row(cnpj, '', '2026-10-01', '44.99'),
            row(cnpj, '', '2026-10-02', '45.01'),
          ],
        }),
      ).refresh();

      expect(await quotasOf(id), {'2026-10-01': 99, '2026-10-02': 45.01});
    });

    test('CNPJ que só tem subclasses pede uma; escolhida, lê a cota dela', () async {
      final id = await addFund(
        fundCnpj: subCnpj,
        term: FundTerm.longTerm,
      );
      final months = {
        (2026, 10): [
          header,
          row(subCnpj, 'AAA', '2026-10-01', '74.38'),
          row(subCnpj, 'BBB', '2026-10-01', '74.04'),
        ],
      };

      final first = (await sync(FakeCvm(months)).refresh())[id]!;
      expect(first.kind, CvmStatusKind.needsSubclass);
      expect(first.options, ['AAA', 'BBB']);
      expect(await quotasOf(id), isEmpty);

      await repository.setFundCvm(id, cnpj: subCnpj, subclass: 'BBB');
      final second = (await sync(FakeCvm(months)).refresh())[id]!;
      expect(second.kind, CvmStatusKind.ok);
      expect(await quotasOf(id, source: 'cvm'), {'2026-10-01': 74.04});
    });

    test('CNPJ que o informe não traz é "não encontrado"', () async {
      final id = await addFund();

      final status = (await sync(
        FakeCvm({
          (2026, 10): [header, row(subCnpj, '', '2026-10-01', '74.38')],
        }),
      ).refresh())[id]!;

      expect(status.kind, CvmStatusKind.notFound);
    });

    test('fundo sem CNPJ não vai à rede', () async {
      final id = await addFund(fundCnpj: null);
      final cvm = FakeCvm({});

      final statuses = await sync(cvm).refresh();

      expect(statuses[id]!.kind, CvmStatusKind.noCnpj);
      expect(cvm.requested, isEmpty);
    });

    test('CVM fora do ar vira erro, sem perder o que já foi lido', () async {
      final id = await addFund();

      final statuses = await sync(
        FakeCvm(
          {
            (2026, 5): [header, row(cnpj, '', '2026-05-28', '40.5')],
          },
          failing: {(2026, 9)},
        ),
      ).refresh();

      expect(statuses[id]!.kind, CvmStatusKind.error);
      expect(statuses[id]!.message, contains('503'));
      expect(await quotasOf(id, source: 'cvm'), {'2026-05-28': 40.5});
    });

    test('informa o progresso mês a mês', () async {
      await addFund();
      final progress = <(int, int)>[];

      await sync(FakeCvm({})).refresh(
        onProgress: (done, total) => progress.add((done, total)),
      );

      expect(progress, [(0, 4), (1, 4), (2, 4), (3, 4), (4, 4)]);
    });
  });
}

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/transactions_csv.dart';
import 'package:anchor_mobile/domain/transactions_csv_import.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/custodia.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

const _header =
    'date,type,ticker,asset_class,currency,quantity,unit_price,total_value,'
    'fee,custody,payment_type,notes,loss_credit_used,fund_term,'
    'fund_structure,fund_estoque_option';

void main() {
  final created = DateTime(2026, 1, 1);

  Asset asset(int id, String ticker, {AssetClass? assetClass}) => Asset(
    id: id,
    ticker: ticker,
    name: ticker,
    assetClass: assetClass ?? AssetClass.acaoBr,
    currency: 'BRL',
    createdAt: created,
  );

  Custodia custodia(int id, String instituicao) => Custodia(
    id: id,
    instituicao: instituicao,
    titular: 'Fulano',
    createdAt: created,
  );

  PortfolioTransaction tx({
    int? assetId,
    TransactionType type = TransactionType.compra,
    double? quantity,
    double? unitPrice,
    double total = 1000,
    int? custodiaId,
    DateTime? date,
  }) => PortfolioTransaction(
    assetId: assetId,
    type: type,
    quantity: quantity,
    unitPrice: unitPrice,
    totalValue: total,
    custodiaId: custodiaId,
    date: date ?? DateTime(2026, 3, 10),
    createdAt: created,
  );

  ImportPlan plan(
    String csv, {
    List<Asset> assets = const [],
    List<Custodia> custodias = const [],
    List<PortfolioTransaction> transactions = const [],
  }) => planTransactionsImport(
    parseTransactionsCsv(csv),
    assets: assets,
    custodias: custodias,
    transactions: transactions,
  );

  group('parseTransactionsCsv', () {
    test('lê uma linha do desktop com todos os campos', () {
      final parsed = parseTransactionsCsv(
        '$_header\n'
        '2026-03-10,compra,PETR4,acao_br,BRL,100,30.5,3050,4.9,XP,,nota,,,,\n',
      );

      expect(parsed.issues, isEmpty);
      final row = parsed.rows.single;
      expect(row.date, DateTime(2026, 3, 10));
      expect(row.type, TransactionType.compra);
      expect(row.ticker, 'PETR4');
      expect(row.assetClass, AssetClass.acaoBr);
      expect(row.currency, 'BRL');
      expect(row.quantity, 100);
      expect(row.unitPrice, 30.5);
      expect(row.totalValue, 3050);
      expect(row.fee, 4.9);
      expect(row.custody, 'XP');
      expect(row.notes, 'nota');
    });

    test('aceita data DD/MM/AAAA, número com vírgula e BOM', () {
      final parsed = parseTransactionsCsv(
        '\u{feff}date,type,ticker,total_value\n'
        '05/01/2026,compra,petr4f,"1.234,56"\n',
      );

      expect(parsed.issues, isEmpty);
      expect(parsed.rows.single.date, DateTime(2026, 1, 5));
      expect(parsed.rows.single.totalValue, 1234.56);
      expect(parsed.rows.single.ticker, 'PETR4', reason: 'sufixo fracionário sai');
    });

    test('campo entre aspas com vírgula, aspas duplas e quebra de linha', () {
      final parsed = parseTransactionsCsv(
        '$_header\n'
        '2026-03-10,compra,PETR4,acao_br,BRL,1,10,10,,"Corretora ""A"", filial",,"linha 1\nlinha 2",,,,\n',
      );

      expect(parsed.issues, isEmpty);
      expect(parsed.rows.single.custody, 'Corretora "A", filial');
      expect(parsed.rows.single.notes, 'linha 1\nlinha 2');
    });

    test('CRLF e CSV com colunas antigas (12) são lidos', () {
      final parsed = parseTransactionsCsv(
        'date,type,ticker,asset_class,currency,quantity,unit_price,total_value,fee,custody,payment_type,notes\r\n'
        '2026-01-05,compra,PETR4,,,100,30.5,3050,4.9,XP,,\r\n',
      );

      expect(parsed.issues, isEmpty);
      expect(parsed.rows.single.assetClass, isNull);
      expect(parsed.rows.single.custody, 'XP');
    });

    test('cabeçalho sem colunas obrigatórias vira um erro na linha 0', () {
      final parsed = parseTransactionsCsv('foo,bar\n1,2\n');

      expect(parsed.rows, isEmpty);
      expect(parsed.issues.single.line, 0);
      expect(parsed.issues.single.message, contains('date'));
    });

    test('data, número e tipo inválidos viram erro com o número da linha', () {
      final parsed = parseTransactionsCsv(
        'date,type,ticker,total_value\n'
        '2026-01-05,compra,PETR4,100\n'
        '05-01-26,compra,PETR4,100\n'
        '2026-01-06,compra,PETR4,abc\n'
        '2026-01-07,inexistente,PETR4,100\n',
      );

      expect(parsed.rows, hasLength(1));
      expect(parsed.issues.map((i) => i.line), [2, 3, 4]);
    });

    test('31/02 não existe: o Dart normalizaria para março, aqui é erro', () {
      final parsed = parseTransactionsCsv(
        'date,type,total_value\n31/02/2026,aporte,100\n',
      );

      expect(parsed.rows, isEmpty);
      expect(parsed.issues.single.line, 1);
    });

    test('arquivo vazio não derruba', () {
      final parsed = parseTransactionsCsv('');

      expect(parsed.rows, isEmpty);
      expect(parsed.issues, isNotEmpty);
    });
  });

  group('planTransactionsImport', () {
    const buy =
        '$_header\n2026-03-10,compra,PETR4,acao_br,BRL,100,30,3000,,XP,,,,,,\n';

    test('ativo e custódia que não existem entram como novos', () {
      final p = plan(buy);

      expect(p.rows, hasLength(1));
      expect(p.newAssets.single.ticker, 'PETR4');
      expect(p.newAssets.single.assetClass, AssetClass.acaoBr);
      expect(p.newAssets.single.currency, 'BRL');
      expect(p.newCustodias, ['XP']);
      expect(p.duplicates, 0);
    });

    test('ativo e custódia existentes são reaproveitados (custódia sem caixa)', () {
      final p = plan(
        buy,
        assets: [asset(1, 'petr4')],
        custodias: [custodia(1, 'xp')],
      );

      expect(p.rows, hasLength(1));
      expect(p.newAssets, isEmpty);
      expect(p.newCustodias, isEmpty);
    });

    test('ativo novo sem classe vira erro, e a linha não entra', () {
      final p = plan(
        'date,type,ticker,quantity,unit_price,total_value\n'
        '2026-03-10,compra,XYZ1,1,10,10\n',
      );

      expect(p.rows, isEmpty);
      expect(p.issues.single.line, 1);
      expect(p.issues.single.message, contains('XYZ1'));
    });

    test('moeda ausente usa a padrão da classe', () {
      final p = plan(
        'date,type,ticker,asset_class,quantity,unit_price,total_value\n'
        '2026-03-10,compra,AAPL,acao_internacional,1,10,10\n',
      );

      expect(p.newAssets.single.currency, 'USD');
    });

    test('mesma compra já gravada é duplicata e não entra', () {
      final p = plan(
        buy,
        assets: [asset(1, 'PETR4')],
        transactions: [
          tx(assetId: 1, quantity: 100, unitPrice: 30, total: 3000),
        ],
      );

      expect(p.rows, isEmpty);
      expect(p.duplicates, 1);
    });

    test('dois negócios iguais no mesmo dia contam como dois', () {
      const twice =
          '$_header\n'
          '2026-03-10,compra,PETR4,acao_br,BRL,100,30,3000,,,,,,,,\n'
          '2026-03-10,compra,PETR4,acao_br,BRL,100,30,3000,,,,,,,,\n';
      final existing = [
        asset(1, 'PETR4'),
      ];
      final one = [tx(assetId: 1, quantity: 100, unitPrice: 30, total: 3000)];

      final onlyOneExists = plan(
        twice,
        assets: existing,
        transactions: one,
      );
      expect(onlyOneExists.duplicates, 1);
      expect(onlyOneExists.rows, hasLength(1));

      final noneExists = plan(twice, assets: existing);
      expect(noneExists.duplicates, 0);
      expect(noneExists.rows, hasLength(2));
    });

    test('total difere em centavos de arredondamento: ainda é duplicata', () {
      final p = plan(
        '$_header\n2026-03-10,compra,PETR4,acao_br,BRL,100,30,3000.001,,,,,,,,\n',
        assets: [asset(1, 'PETR4')],
        transactions: [
          tx(assetId: 1, quantity: 100, unitPrice: 30, total: 3000),
        ],
      );

      expect(p.duplicates, 1);
    });

    test('aporte sem ativo não exige classe nem ticker', () {
      final p = plan('date,type,total_value\n2026-03-10,aporte,5000\n');

      expect(p.rows, hasLength(1));
      expect(p.newAssets, isEmpty);
      expect(p.issues, isEmpty);
    });

    test('transferência é pulada e contada (o CSV não traz o destino)', () {
      final p = plan(
        '$_header\n'
        '2026-03-10,transferencia,PETR4,acao_br,BRL,5,,0,,XP,,,,,,\n',
      );

      expect(p.rows, isEmpty);
      expect(p.skipped, 1);
      expect(p.issues, isEmpty);
    });

    test('compra sem quantidade é erro em vez de gravar posição errada', () {
      final p = plan(
        'date,type,ticker,asset_class,total_value\n'
        '2026-03-10,compra,PETR4,acao_br,3000\n',
      );

      expect(p.rows, isEmpty);
      expect(p.issues.single.line, 1);
    });
  });

  group('ida e volta', () {
    test('o que o mobile exporta, o mobile reimporta igual', () {
      final assets = [
        asset(1, 'PETR4'),
        asset(2, 'BTC', assetClass: AssetClass.cripto),
      ];
      final custodias = [custodia(7, 'Corretora "A", filial')];
      final original = [
        tx(
          assetId: 1,
          quantity: 10,
          unitPrice: 100,
          custodiaId: 7,
          date: DateTime(2026, 3, 10),
        ),
        tx(
          assetId: 1,
          type: TransactionType.venda,
          quantity: 4,
          unitPrice: 120,
          total: 480,
          date: DateTime(2026, 4, 1),
        ),
        tx(assetId: 2, quantity: 0.00123456, unitPrice: 60000, total: 74.07),
        tx(type: TransactionType.aporte, total: 5000),
      ];

      final csv = transactionsCsv(assets, custodias, original).csv;

      // num celular vazio, tudo entra e cria ativos e custódia
      final empty = plan(csv);
      expect(empty.rows, hasLength(4));
      expect(empty.newAssets.map((a) => a.ticker).toSet(), {'PETR4', 'BTC'});
      expect(empty.newCustodias, ['Corretora "A", filial']);

      // no mesmo celular, nada é novo (reimportar o próprio arquivo é seguro)
      final same = plan(
        csv,
        assets: assets,
        custodias: custodias,
        transactions: original,
      );
      expect(same.rows, isEmpty);
      expect(same.duplicates, 4);
    });
  });

  group('applyTransactionsImport', () {
    final repository = PortfolioRepository();
    late Directory dir;

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-import-test-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    Future<ImportPlan> planAgainstDb(String csv) async => planTransactionsImport(
      parseTransactionsCsv(csv),
      assets: await repository.listAssets(),
      custodias: await repository.listCustodias(),
      transactions: await repository.listAllTransactions(),
    );

    const csv =
        '$_header\n'
        '2026-03-10,compra,PETR4,acao_br,BRL,100,30,3000,,XP,,,,,,\n'
        '2026-03-11,venda,PETR4,acao_br,BRL,40,35,1400,,xp,,,,,,\n'
        '2026-03-12,aporte,,,,,,5000,,,,,,,,\n';

    test('grava ativo, custódia e transações, e a posição fecha', () async {
      final written = await repository.applyTransactionsImport(
        await planAgainstDb(csv),
      );

      expect(written, 3);
      final assets = await repository.listAssets();
      final custodias = await repository.listCustodias();
      expect(assets.single.ticker, 'PETR4');
      expect(custodias.single.instituicao, 'XP');
      expect(custodias.single.titular, 'Importado');

      final transactions = await repository.listAllTransactions();
      expect(transactions, hasLength(3));
      final position = PortfolioRepository.computePositions(
        assets,
        transactions,
      ).single;
      expect(position.netQuantity, 60);
    });

    test('reimportar o mesmo arquivo não grava nada', () async {
      await repository.applyTransactionsImport(await planAgainstDb(csv));

      final again = await planAgainstDb(csv);
      expect(again.rows, isEmpty);
      expect(again.duplicates, 3);
      expect(await repository.applyTransactionsImport(again), 0);
      expect(await repository.listAllTransactions(), hasLength(3));
    });

    test('nada fica pela metade se a gravação falhar', () async {
      final good = await planAgainstDb(csv);
      // plano incoerente: as linhas usam PETR4, que não está em newAssets nem no
      // banco. A custódia é criada antes e a gravação das transações quebra depois.
      final broken = ImportPlan(
        rows: good.rows,
        duplicates: 0,
        skipped: 0,
        issues: const [],
        newAssets: const [],
        newCustodias: good.newCustodias,
      );

      await expectLater(
        repository.applyTransactionsImport(broken),
        throwsStateError,
      );

      expect(await repository.listAssets(), isEmpty);
      expect(await repository.listCustodias(), isEmpty);
      expect(await repository.listAllTransactions(), isEmpty);
    });
  });
}

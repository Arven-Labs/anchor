import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/transactions_csv.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/custodia.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';

void main() {
  final created = DateTime(2026, 1, 1);

  // Com titular: o rótulo de tela ("XP (Fulano)") difere do nome que o desktop casa ("XP").
  Custodia custodia(int id, String instituicao) => Custodia(
    id: id,
    instituicao: instituicao,
    titular: 'Fulano',
    createdAt: created,
  );

  Asset asset(
    int id,
    String ticker, {
    AssetClass assetClass = AssetClass.acaoBr,
    String currency = 'BRL',
  }) => Asset(
    id: id,
    ticker: ticker,
    name: ticker,
    assetClass: assetClass,
    currency: currency,
    createdAt: created,
  );

  PortfolioTransaction tx({
    int? assetId,
    TransactionType type = TransactionType.compra,
    double? quantity,
    double? unitPrice,
    double total = 1000,
    double? fee,
    int? custodiaId,
    int? transferTo,
    DateTime? date,
    String? notes,
  }) => PortfolioTransaction(
    assetId: assetId,
    type: type,
    quantity: quantity,
    unitPrice: unitPrice,
    totalValue: total,
    fee: fee,
    custodiaId: custodiaId,
    transferToCustodiaId: transferTo,
    date: date ?? DateTime(2026, 3, 10, 14, 30),
    notes: notes,
    createdAt: created,
  );

  const header =
      'date,type,ticker,asset_class,currency,quantity,unit_price,total_value,'
      'fee,custody,payment_type,notes,loss_credit_used,fund_term,'
      'fund_structure,fund_estoque_option';

  test('o cabeçalho é idêntico ao CSV_HEADER do desktop (16 colunas)', () {
    final result = transactionsCsv([], [], []);

    expect(result.csv, '$header\n');
    expect(result.exported, 0);
    expect(result.skipped, 0);
  });

  test('compra sai com ticker, classe, moeda, custódia por nome e só a data', () {
    final result = transactionsCsv(
      [asset(1, 'PETR4')],
      [custodia(7, 'XP')],
      [
        tx(
          assetId: 1,
          quantity: 10,
          unitPrice: 100,
          fee: 1.5,
          custodiaId: 7,
        ),
      ],
    );

    // data sem hora: o desktop deduplica pelo texto da data
    expect(
      result.csv,
      '$header\n2026-03-10,compra,PETR4,acao_br,BRL,10,100,1000,1.5,XP,,,,,,\n',
    );
    expect(result.exported, 1);
  });

  test('a custódia sai só com o nome da instituição, sem o titular do rótulo de tela', () {
    final result = transactionsCsv(
      [asset(1, 'PETR4')],
      [custodia(7, 'XP')],
      [tx(assetId: 1, quantity: 1, unitPrice: 10, total: 10, custodiaId: 7)],
    );

    // o desktop casa por instituição: "XP (Fulano)" criaria uma custódia duplicada lá
    expect(result.csv, contains(',10,10,,XP,'));
    expect(result.csv, isNot(contains('Fulano')));
  });

  test('aporte e retirada saem sem ticker, classe, quantidade e preço', () {
    final result = transactionsCsv([], [], [
      tx(type: TransactionType.aporte, total: 5000),
      tx(type: TransactionType.retirada, total: 200),
    ]);

    expect(result.csv, contains('\n2026-03-10,aporte,,,,,,5000,,,,,,,,\n'));
    expect(result.csv, contains('\n2026-03-10,retirada,,,,,,200,,,,,,,,\n'));
    expect(result.exported, 2);
  });

  test('provento e come-cotas são exportados', () {
    final result = transactionsCsv(
      [asset(1, 'HGLG11', assetClass: AssetClass.fii)],
      [],
      [
        tx(assetId: 1, type: TransactionType.provento, total: 80),
        tx(
          assetId: 1,
          type: TransactionType.comeCotas,
          quantity: 0.5,
          unitPrice: 120,
          total: 60,
        ),
      ],
    );

    expect(result.csv, contains(',provento,HGLG11,fii,BRL,,,80,'));
    expect(result.csv, contains(',come_cotas,HGLG11,fii,BRL,0.5,120,60,'));
    expect(result.exported, 2);
    expect(result.skipped, 0);
  });

  test('transferência e amortização são puladas e contadas, não somem em silêncio', () {
    final result = transactionsCsv(
      [asset(1, 'PETR4')],
      [custodia(1, 'XP'), custodia(2, 'Rico')],
      [
        tx(assetId: 1, quantity: 10, unitPrice: 100),
        tx(
          assetId: 1,
          type: TransactionType.transferencia,
          quantity: 5,
          total: 0,
          custodiaId: 1,
          transferTo: 2,
        ),
        tx(assetId: 1, type: TransactionType.amortizacao, total: 300),
      ],
    );

    expect(result.exported, 1);
    expect(result.skipped, 2);
    expect(result.csv, isNot(contains('transferencia')));
    expect(result.csv, isNot(contains('amortizacao')));
  });

  test('texto com vírgula, aspas ou quebra de linha é escapado', () {
    final result = transactionsCsv(
      [asset(1, 'PETR4')],
      [custodia(1, 'Corretora "A", filial')],
      [
        tx(
          assetId: 1,
          quantity: 1,
          unitPrice: 10,
          total: 10,
          custodiaId: 1,
          notes: 'linha 1\nlinha 2',
        ),
      ],
    );

    expect(result.csv, contains('"Corretora ""A"", filial"'));
    expect(result.csv, contains('"linha 1\nlinha 2"'));
  });

  test('quantidade fracionada sai sem zeros à direita e com ponto decimal', () {
    final result = transactionsCsv(
      [asset(1, 'BTC', assetClass: AssetClass.cripto, currency: 'USD')],
      [],
      [tx(assetId: 1, quantity: 0.00123456, unitPrice: 60000, total: 74.07)],
    );

    expect(result.csv, contains(',cripto,USD,0.00123456,60000,74.07,'));
  });

  test('as linhas saem em ordem cronológica, e a mesma data mantém a ordem de entrada', () {
    final result = transactionsCsv(
      [asset(1, 'PETR4')],
      [],
      [
        tx(assetId: 1, date: DateTime(2026, 5, 2), total: 3),
        tx(assetId: 1, date: DateTime(2026, 1, 2), total: 1),
        tx(assetId: 1, date: DateTime(2026, 3, 2), total: 2),
      ],
    );

    final dates = result.csv
        .split('\n')
        .skip(1)
        .where((l) => l.isNotEmpty)
        .map((l) => l.substring(0, 10))
        .toList();
    expect(dates, ['2026-01-02', '2026-03-02', '2026-05-02']);
  });

  test('transação de ativo que não existe mais é pulada e contada', () {
    final result = transactionsCsv([], [], [
      tx(assetId: 99, quantity: 1, unitPrice: 10, total: 10),
    ]);

    expect(result.exported, 0);
    expect(result.skipped, 1);
  });
}

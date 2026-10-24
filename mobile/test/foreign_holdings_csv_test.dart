import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/foreign_holdings_csv.dart';
import 'package:anchor_mobile/domain/foreign_tax.dart';

void main() {
  ForeignHolding holding({
    int year = 2025,
    int? custodiaId,
    String ticker = 'AAPL',
    String name = 'Apple',
    double quantity = 10,
    double cost = 5500,
    bool approximate = false,
    bool incomplete = false,
    String? group,
    String? code,
    String? country,
  }) => ForeignHolding(
    year: year,
    assetId: 1,
    custodiaId: custodiaId,
    ticker: ticker,
    name: name,
    currency: 'USD',
    quantity: quantity,
    costBrl: cost,
    fxApproximate: approximate,
    incomplete: incomplete,
    dirpfGroup: group,
    dirpfCode: code,
    dirpfCountry: country,
  );

  test('o cabeçalho tem as mesmas colunas do desktop (13, com a ficha no fim)', () {
    final csv = foreignHoldingsCsv([], {});

    expect(
      csv,
      'year,ticker,name,exchange,custodia,currency,quantity,cost_brl,'
      'fx_approximate,incomplete,dirpf_group,dirpf_code,dirpf_country\n',
    );
  });

  test('uma linha por posição, com custo de duas casas e quantidade limpa', () {
    final csv = foreignHoldingsCsv([
      holding(),
      holding(year: 2026, quantity: 2.5, cost: 1234.5),
    ], {});

    // sem ficha informada, as três colunas finais saem vazias
    expect(csv, contains('\n2025,AAPL,Apple,,,USD,10,5500.00,false,false,,,\n'));
    expect(csv, contains('\n2026,AAPL,Apple,,,USD,2.5,1234.50,false,false,,,\n'));
  });

  test('grupo, código e país da ficha saem nas três últimas colunas', () {
    final csv = foreignHoldingsCsv([
      holding(group: '03', code: '01', country: '249'),
    ], {});

    expect(csv, contains('\n2025,AAPL,Apple,,,USD,10,5500.00,false,false,03,01,249\n'));
  });

  test('o nome da custódia e as marcas saem na linha', () {
    final csv = foreignHoldingsCsv(
      [holding(custodiaId: 3, approximate: true, incomplete: true)],
      {3: 'Avenue (Eu)'},
    );

    expect(
      csv,
      contains('2025,AAPL,Apple,,Avenue (Eu),USD,10,5500.00,true,true,,,'),
    );
  });

  test('nome com vírgula ou aspas vai entre aspas', () {
    final csv = foreignHoldingsCsv([
      holding(ticker: 'BRK.B', name: 'Berkshire Hathaway, Inc. "B"'),
    ], {});

    expect(
      csv,
      contains('2025,BRK.B,"Berkshire Hathaway, Inc. ""B""",,,USD,10,'),
    );
  });

  test('custódia sem nome conhecido sai vazia', () {
    final csv = foreignHoldingsCsv([holding(custodiaId: 9)], {});

    expect(csv, contains('2025,AAPL,Apple,,,USD,10,'));
  });
}

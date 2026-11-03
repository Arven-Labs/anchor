import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/home_layout.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/home_labels.dart';
import 'package:anchor_mobile/models/asset.dart';

// Nomes dos cards do Início e das classes de ativo vivem nos `.arb` (ADR 0001). O português
// é o texto que o app já mostrava, fixado aqui por extenso (o `AssetClass.label` saiu).
Future<AppLocalizations> _load(String code) => AppLocalizations.delegate.load(Locale(code));

void main() {
  late AppLocalizations en;
  late AppLocalizations pt;

  setUpAll(() async {
    en = await _load('en');
    pt = await _load('pt');
  });

  group('cards do Início', () {
    test('pt: o texto de antes', () {
      expect({for (final k in HomeWidgetKey.values) k.id: homeWidgetName(pt, k)}, {
        'total_value': 'Valor total',
        'profit': 'Lucro não realizado',
        'allocation': 'Alocação por classe',
        'net_worth': 'Patrimônio líquido',
      });
    });

    test('en: tradução fixada', () {
      expect({for (final k in HomeWidgetKey.values) k.id: homeWidgetName(en, k)}, {
        'total_value': 'Total value',
        'profit': 'Unrealized profit',
        'allocation': 'Allocation by class',
        'net_worth': 'Net worth',
      });
    });

    test('o identificador gravado no JSON não muda (o rótulo saiu do enum)', () {
      expect(HomeWidgetKey.values.map((k) => k.id), [
        'total_value',
        'profit',
        'allocation',
        'net_worth',
      ]);
    });
  });

  group('classes de ativo', () {
    // Os nomes em português que o `AssetClass.label` guardava, por extenso.
    const ptNames = {
      AssetClass.acaoBr: 'Ação BR',
      AssetClass.fii: 'FII',
      AssetClass.etfBr: 'ETF BR',
      AssetClass.etfRendaFixa: 'ETF Renda Fixa BR',
      AssetClass.bdr: 'BDR',
      AssetClass.acaoInternacional: 'Ação internacional',
      AssetClass.reit: 'REIT',
      AssetClass.etfUs: 'ETF US',
      AssetClass.cripto: 'Cripto',
      AssetClass.metal: 'Metal',
      AssetClass.tesouroDireto: 'Tesouro Direto',
      AssetClass.rendaFixa: 'Renda Fixa',
      AssetClass.imovel: 'Imóvel',
      AssetClass.empresaNaoListada: 'Empresa não listada',
      AssetClass.fundo: 'Fundo',
    };

    test('pt: o nome de antes, para toda classe', () {
      expect(ptNames.keys.toSet(), AssetClass.values.toSet(), reason: 'tabela cobre todas');
      for (final c in AssetClass.values) {
        expect(assetClassName(pt, c), ptNames[c], reason: c.value);
      }
    });

    test('en: toda classe tem nome, e siglas brasileiras ficam como estão', () {
      for (final c in AssetClass.values) {
        expect(assetClassName(en, c), isNotEmpty, reason: c.value);
      }
      expect(assetClassName(en, AssetClass.acaoBr), 'BR stock');
      expect(assetClassName(en, AssetClass.fii), 'FII');
      expect(assetClassName(en, AssetClass.etfBr), 'BR ETF');
      expect(assetClassName(en, AssetClass.etfRendaFixa), 'BR fixed-income ETF');
      expect(assetClassName(en, AssetClass.bdr), 'BDR');
      expect(assetClassName(en, AssetClass.acaoInternacional), 'International stock');
      expect(assetClassName(en, AssetClass.reit), 'REIT');
      expect(assetClassName(en, AssetClass.etfUs), 'US ETF');
      expect(assetClassName(en, AssetClass.cripto), 'Crypto');
      expect(assetClassName(en, AssetClass.metal), 'Metal');
      expect(assetClassName(en, AssetClass.tesouroDireto), 'Tesouro Direto');
      expect(assetClassName(en, AssetClass.rendaFixa), 'Fixed income');
      expect(assetClassName(en, AssetClass.imovel), 'Real estate');
      expect(assetClassName(en, AssetClass.empresaNaoListada), 'Unlisted company');
      expect(assetClassName(en, AssetClass.fundo), 'Fund');
    });

    test('en: nenhum nome fica igual ao português por esquecimento', () {
      // Siglas e nomes próprios iguais nos dois idiomas, de propósito.
      const same = {'fii', 'bdr', 'reit', 'metal', 'tesouro_direto'};
      for (final c in AssetClass.values) {
        if (same.contains(c.value)) continue;
        expect(assetClassName(en, c), isNot(ptNames[c]), reason: c.value);
      }
    });
  });
}

import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/crypto_indicator_info.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';

const _coins = [coinEth, coinBtc, coinAda, coinXrp, coinBnb];

/// Todo indicador de todo conjunto de moeda.
Set<String> _allIndicators() => {
  for (final coin in _coins)
    for (final group in IndicatorGroup.forCoin(coin)) ...group.indicators(coin),
};

// Os textos (nome do indicador, grupo, veredito, sinal) saíram do domínio e vivem nos
// `.arb`; são testados nos dois idiomas em `crypto_labels_test.dart`. O domínio guarda
// só as chaves e a formatação de números por idioma.
void main() {
  group('chaves conhecidas', () {
    test('o conjunto de indicadores conhecidos cobre todo indicador de toda moeda', () {
      final indicators = _allIndicators();
      expect(indicators, hasLength(22)); // se mudar, os `.arb` também
      expect(knownIndicators, equals(indicators));
    });
  });

  group('describeCollectionError', () {
    // A função não conhece idioma: recebe quem dá o nome do indicador (a tela passa
    // o `AppLocalizations`), ou `null` quando a chave é desconhecida.
    String? names(String key) => switch (key) {
      'sopr' => 'SOPR',
      'exchange_netflow' => 'Exchange netflow',
      'mvrv_z_score' => 'MVRV Z-Score',
      _ => null,
    };

    test('troca a chave do banco pelo nome do indicador e mantém o resto', () {
      expect(
        describeCollectionError('BTC sopr: bitcoin-data.com: unavailable (rate limit)', nameOf: names),
        'BTC SOPR: bitcoin-data.com: unavailable (rate limit)',
      );
      expect(
        describeCollectionError('ETH exchange_netflow: CoinMetrics: HTTP 404', nameOf: names),
        'ETH Exchange netflow: CoinMetrics: HTTP 404',
      );
    });

    test('indicador desconhecido e linhas de outro formato passam como vieram', () {
      for (final line in [
        'ETH indicador_novo: x',
        'crypto score: ETH x: unavailable',
        'Erro inesperado: Bad state: bug',
        '',
      ]) {
        expect(describeCollectionError(line, nameOf: names), line, reason: line);
      }
    });

    test('o detalhe pode ter dois-pontos e quebras de linha', () {
      expect(
        describeCollectionError('ADA mvrv_z_score: a: b\nc', nameOf: names),
        'ADA MVRV Z-Score: a: b\nc',
      );
    });
  });

  group('formatIndicatorValue', () {
    test('todo indicador de todo conjunto formata um número comum nos dois idiomas', () {
      for (final indicator in _allIndicators()) {
        final pt = formatIndicatorValue(indicator, 1.5, locale: 'pt');
        final en = formatIndicatorValue(indicator, 1.5, locale: 'en');
        for (final text in [pt, en]) {
          expect(text, isNotEmpty, reason: indicator);
          expect(text, isNot(contains('NaN')), reason: indicator);
        }
        expect(pt, isNot(contains('.')), reason: '$indicator usa vírgula em pt');
        expect(en, isNot(contains(',')), reason: '$indicator usa ponto em en');
      }
    });

    test('pt: percentuais levam " %" e duas casas, com vírgula decimal', () {
      String pt(String i, double v) => formatIndicatorValue(i, v, locale: 'pt');
      expect(pt('staking_yield', 1.360842742486958), '1,36 %');
      expect(pt('tvl_trend', -13.961346656541899), '-13,96 %');
      expect(pt('active_addresses_trend', -25.507515219130166), '-25,51 %');
      expect(pt('btc_dominance', 59.98151875544407), '59,98 %');
      expect(pt('funding_rate_7d', 6.376), '6,38 %');
    });

    test('en: percentuais levam "%" colado e duas casas, com ponto decimal', () {
      String en(String i, double v) => formatIndicatorValue(i, v, locale: 'en');
      expect(en('staking_yield', 1.360842742486958), '1.36%');
      expect(en('tvl_trend', -13.961346656541899), '-13.96%');
      expect(en('btc_dominance', 59.98151875544407), '59.98%');
      expect(en('funding_rate_7d', 6.376), '6.38%');
    });

    test('razões e múltiplos têm duas casas e nenhuma unidade', () {
      String pt(String i, double v) => formatIndicatorValue(i, v, locale: 'pt');
      String en(String i, double v) => formatIndicatorValue(i, v, locale: 'en');
      expect(pt('mvrv_z_score', 1.7888543819998317), '1,79');
      expect(pt('puell_multiple', 0.4), '0,40');
      expect(pt('exchange_netflow', 0.5), '0,50');
      expect(pt('long_short_ratio', 2.639), '2,64');
      expect(pt('nupl', 0.3017), '0,30');
      expect(en('mvrv_z_score', 1.7888543819998317), '1.79');
      expect(en('puell_multiple', 0.4), '0.40');
      expect(en('nupl', 0.3017), '0.30');
    });

    test('SOPR tem três casas (vive perto de 1)', () {
      expect(formatIndicatorValue('sopr', 0.9933, locale: 'pt'), '0,993');
      expect(formatIndicatorValue('sopr', 1.0057, locale: 'pt'), '1,006');
      expect(formatIndicatorValue('sopr', 0.9933, locale: 'en'), '0.993');
    });

    test('Fear & Greed é inteiro', () {
      for (final locale in ['pt', 'en']) {
        expect(formatIndicatorValue('fear_greed_index', 34, locale: locale), '34');
        expect(formatIndicatorValue('fear_greed_index', 33.6, locale: locale), '34');
      }
    });

    test('nunca mostra -0,00 (zero negativo)', () {
      expect(formatIndicatorValue('mvrv_z_score', -0.001, locale: 'pt'), '0,00');
      expect(formatIndicatorValue('mvrv_z_score', -0.0, locale: 'pt'), '0,00');
      expect(formatIndicatorValue('tvl_trend', -0.004, locale: 'pt'), '0,00 %');
      expect(formatIndicatorValue('mvrv_z_score', -0.001, locale: 'en'), '0.00');
      expect(formatIndicatorValue('tvl_trend', -0.004, locale: 'en'), '0.00%');
      // Mas -0,01 continua negativo.
      expect(formatIndicatorValue('mvrv_z_score', -0.01, locale: 'pt'), '-0,01');
      expect(formatIndicatorValue('mvrv_z_score', -0.01, locale: 'en'), '-0.01');
    });

    test('valor não finito vira traço, não "NaN" nem "Infinity"', () {
      for (final locale in ['pt', 'en']) {
        for (final value in [double.nan, double.infinity, double.negativeInfinity]) {
          expect(
            formatIndicatorValue('mvrv_z_score', value, locale: locale),
            '—',
            reason: '$value ($locale)',
          );
        }
      }
    });

    test('indicador desconhecido ainda formata com duas casas', () {
      expect(formatIndicatorValue('indicador_novo', 3.14159, locale: 'pt'), '3,14');
      expect(formatIndicatorValue('indicador_novo', 3.14159, locale: 'en'), '3.14');
    });

    test('número grande no en não ganha separador de milhar (a tela não agrupa)', () {
      expect(formatIndicatorValue('mvrv_z_score', 12345.678, locale: 'en'), '12345.68');
      expect(formatIndicatorValue('mvrv_z_score', 12345.678, locale: 'pt'), '12345,68');
    });
  });
}

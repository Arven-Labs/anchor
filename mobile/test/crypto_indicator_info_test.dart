import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/crypto_indicator_info.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';

const _coins = [coinEth, coinBtc, coinAda, coinXrp, coinBnb];

/// Todo indicador de todo conjunto de moeda.
Set<String> _allIndicators() => {
  for (final coin in _coins)
    for (final group in IndicatorGroup.forCoin(coin)) ...group.indicators(coin),
};

void main() {
  group('rótulos', () {
    test('todo indicador de todo conjunto tem nome legível em português', () {
      final indicators = _allIndicators();
      expect(indicators, hasLength(22)); // se mudar, o mapa de nomes também

      for (final indicator in indicators) {
        final label = indicatorLabel(indicator);
        expect(label, isNotEmpty, reason: indicator);
        // Jargão interno (a chave do banco) nunca aparece na tela.
        expect(label, isNot(contains('_')), reason: indicator);
        expect(label, isNot(equals(indicator)), reason: indicator);
      }
    });

    test('nomes que o dono vai ler (amostra fixada)', () {
      expect(indicatorLabel('mvrv_z_score'), 'MVRV Z-Score');
      expect(indicatorLabel('staking_yield'), 'Staking yield real');
      expect(indicatorLabel('fear_greed_index'), 'Fear & Greed (0–100)');
      expect(indicatorLabel('exchange_netflow'), 'Netflow das exchanges');
      expect(
        indicatorLabel('funding_rate_7d'),
        'Funding dos perpétuos (7d, anualizado)',
      );
      expect(indicatorLabel('sopr'), 'SOPR');
    });

    test('indicador desconhecido não quebra a tela: mostra a chave como veio', () {
      expect(indicatorLabel('indicador_novo'), 'indicador_novo');
    });

    test('grupos em português, um rótulo distinto para cada', () {
      expect({for (final g in IndicatorGroup.values) g: groupLabel(g)}, {
        IndicatorGroup.onChain: 'On-chain',
        IndicatorGroup.fundamentals: 'Fundamentos da rede',
        IndicatorGroup.derivatives: 'Derivativos e opções',
        IndicatorGroup.liquidity: 'Liquidez de mercado',
        IndicatorGroup.sentiment: 'Sentimento de mercado',
      });
    });

    test('veredito e sinal têm texto próprio (a cor nunca é a única pista)', () {
      expect(verdictLabel(ScoreVerdict.intact), 'Tese intacta — manter / seguir aportando');
      expect(verdictLabel(ScoreVerdict.neutral), 'Neutro — acompanhar de perto');
      expect(verdictLabel(ScoreVerdict.reduceRisk), 'Reduzir risco');

      expect(signalLabel(Signal.green.wire), 'Verde');
      expect(signalLabel(Signal.neutral.wire), 'Neutro');
      expect(signalLabel(Signal.red.wire), 'Vermelho');
      expect(signalLabel('OUTRO'), '—');
    });
  });

  group('describeCollectionError', () {
    test('troca a chave do banco pelo nome do indicador e mantém o resto', () {
      expect(
        describeCollectionError('BTC sopr: bitcoin-data.com: unavailable (rate limit)'),
        'BTC SOPR: bitcoin-data.com: unavailable (rate limit)',
      );
      expect(
        describeCollectionError('ETH exchange_netflow: CoinMetrics: HTTP 404'),
        'ETH Netflow das exchanges: CoinMetrics: HTTP 404',
      );
    });

    test('indicador desconhecido e linhas de outro formato passam como vieram', () {
      for (final line in [
        'ETH indicador_novo: x',
        'crypto score: ETH x: unavailable',
        'Erro inesperado: Bad state: bug',
        '',
      ]) {
        expect(describeCollectionError(line), line, reason: line);
      }
    });

    test('o detalhe pode ter dois-pontos e quebras de linha', () {
      expect(
        describeCollectionError('ADA mvrv_z_score: a: b\nc'),
        'ADA MVRV Z-Score: a: b\nc',
      );
    });
  });

  group('formatIndicatorValue', () {
    test('todo indicador de todo conjunto formata um número comum', () {
      for (final indicator in _allIndicators()) {
        final text = formatIndicatorValue(indicator, 1.5);
        expect(text, isNotEmpty, reason: indicator);
        expect(text, isNot(contains('NaN')), reason: indicator);
        expect(text, isNot(contains('.')), reason: '$indicator usa vírgula');
      }
    });

    test('percentuais levam % e duas casas, com vírgula decimal', () {
      expect(formatIndicatorValue('staking_yield', 1.360842742486958), '1,36 %');
      expect(formatIndicatorValue('tvl_trend', -13.961346656541899), '-13,96 %');
      expect(formatIndicatorValue('active_addresses_trend', -25.507515219130166), '-25,51 %');
      expect(formatIndicatorValue('btc_dominance', 59.98151875544407), '59,98 %');
      expect(formatIndicatorValue('funding_rate_7d', 6.376), '6,38 %');
    });

    test('razões e múltiplos têm duas casas e nenhuma unidade', () {
      expect(formatIndicatorValue('mvrv_z_score', 1.7888543819998317), '1,79');
      expect(formatIndicatorValue('puell_multiple', 0.4), '0,40');
      expect(formatIndicatorValue('exchange_netflow', 0.5), '0,50');
      expect(formatIndicatorValue('long_short_ratio', 2.639), '2,64');
      expect(formatIndicatorValue('nupl', 0.3017), '0,30');
    });

    test('SOPR tem três casas (vive perto de 1)', () {
      expect(formatIndicatorValue('sopr', 0.9933), '0,993');
      expect(formatIndicatorValue('sopr', 1.0057), '1,006');
    });

    test('Fear & Greed é inteiro', () {
      expect(formatIndicatorValue('fear_greed_index', 34), '34');
      expect(formatIndicatorValue('fear_greed_index', 33.6), '34');
    });

    test('nunca mostra -0,00 (zero negativo)', () {
      expect(formatIndicatorValue('mvrv_z_score', -0.001), '0,00');
      expect(formatIndicatorValue('mvrv_z_score', -0.0), '0,00');
      expect(formatIndicatorValue('tvl_trend', -0.004), '0,00 %');
      // Mas -0,01 continua negativo.
      expect(formatIndicatorValue('mvrv_z_score', -0.01), '-0,01');
    });

    test('valor não finito vira traço, não "NaN" nem "Infinity"', () {
      for (final value in [double.nan, double.infinity, double.negativeInfinity]) {
        expect(formatIndicatorValue('mvrv_z_score', value), '—', reason: '$value');
      }
    });

    test('indicador desconhecido ainda formata com duas casas', () {
      expect(formatIndicatorValue('indicador_novo', 3.14159), '3,14');
    });
  });
}

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/crypto_history_view.dart';
import 'package:anchor_mobile/domain/crypto_indicator_info.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';
import 'package:anchor_mobile/domain/crypto_threshold_input.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/crypto_labels.dart';

// Os textos da aba Cripto vivem nos `.arb` (ADR 0001). Aqui cada texto é fixado nos dois
// idiomas. O português é exatamente o que o app mostrava antes da migração.
Future<AppLocalizations> _load(String code) => AppLocalizations.delegate.load(Locale(code));

ScoreHistoryPoint _point({
  int green = 5,
  int logged = 10,
  int total = 10,
  ScoreVerdict verdict = ScoreVerdict.neutral,
}) => ScoreHistoryPoint(
  date: '2026-10-29',
  green: green,
  logged: logged,
  total: total,
  verdict: verdict,
);

void main() {
  late AppLocalizations en;
  late AppLocalizations pt;

  setUpAll(() async {
    en = await _load('en');
    pt = await _load('pt');
  });

  group('nome dos indicadores', () {
    test('todo indicador conhecido tem nome nos dois idiomas, sem a chave do banco', () {
      for (final l in [en, pt]) {
        for (final key in knownIndicators) {
          final name = indicatorName(l, key);
          expect(name, isNotEmpty, reason: '$key (${l.localeName})');
          expect(name, isNot(contains('_')), reason: '$key (${l.localeName})');
          expect(name, isNot(equals(key)), reason: '$key (${l.localeName})');
        }
      }
    });

    test('pt: os nomes que o dono lê hoje', () {
      expect(indicatorName(pt, 'mvrv_z_score'), 'MVRV Z-Score');
      expect(indicatorName(pt, 'staking_yield'), 'Staking yield real');
      expect(indicatorName(pt, 'fear_greed_index'), 'Fear & Greed (0–100)');
      expect(indicatorName(pt, 'exchange_netflow'), 'Netflow das exchanges');
      expect(indicatorName(pt, 'funding_rate_7d'), 'Funding dos perpétuos (7d, anualizado)');
      expect(indicatorName(pt, 'net_issuance'), 'Emissão líquida (emissão − queima)');
      expect(indicatorName(pt, 'sopr'), 'SOPR');
    });

    test('en: tradução fixada', () {
      expect(indicatorName(en, 'mvrv_z_score'), 'MVRV Z-Score');
      expect(indicatorName(en, 'staking_yield'), 'Real staking yield');
      expect(indicatorName(en, 'fear_greed_index'), 'Fear & Greed (0–100)');
      expect(indicatorName(en, 'exchange_netflow'), 'Exchange netflow');
      expect(indicatorName(en, 'funding_rate_7d'), 'Perpetual funding (7d, annualized)');
      expect(indicatorName(en, 'net_issuance'), 'Net issuance (issuance − burn)');
      expect(indicatorName(en, 'sopr'), 'SOPR');
    });

    test('indicador desconhecido mostra a chave como veio, em qualquer idioma', () {
      expect(indicatorName(en, 'indicador_novo'), 'indicador_novo');
      expect(indicatorName(pt, 'indicador_novo'), 'indicador_novo');
    });

    test('indicatorNameOrNull devolve null quando a chave é desconhecida', () {
      expect(indicatorNameOrNull(en, 'sopr'), 'SOPR');
      expect(indicatorNameOrNull(en, 'indicador_novo'), isNull);
    });
  });

  group('grupos', () {
    test('pt: um rótulo distinto para cada grupo', () {
      expect({for (final g in IndicatorGroup.values) g: groupName(pt, g)}, {
        IndicatorGroup.onChain: 'On-chain',
        IndicatorGroup.fundamentals: 'Fundamentos da rede',
        IndicatorGroup.derivatives: 'Derivativos e opções',
        IndicatorGroup.liquidity: 'Liquidez de mercado',
        IndicatorGroup.sentiment: 'Sentimento de mercado',
      });
    });

    test('en: um rótulo distinto para cada grupo', () {
      expect({for (final g in IndicatorGroup.values) g: groupName(en, g)}, {
        IndicatorGroup.onChain: 'On-chain',
        IndicatorGroup.fundamentals: 'Network fundamentals',
        IndicatorGroup.derivatives: 'Derivatives & options',
        IndicatorGroup.liquidity: 'Market liquidity',
        IndicatorGroup.sentiment: 'Market sentiment',
      });
    });
  });

  group('veredito e sinal (a cor nunca é a única pista)', () {
    test('pt', () {
      expect(verdictText(pt, ScoreVerdict.intact), 'Tese intacta — manter / seguir aportando');
      expect(verdictText(pt, ScoreVerdict.neutral), 'Neutro — acompanhar de perto');
      expect(verdictText(pt, ScoreVerdict.reduceRisk), 'Reduzir risco');
      expect(signalText(pt, Signal.green.wire), 'Verde');
      expect(signalText(pt, Signal.neutral.wire), 'Neutro');
      expect(signalText(pt, Signal.red.wire), 'Vermelho');
      expect(signalText(pt, 'OUTRO'), '—');
    });

    test('en', () {
      expect(verdictText(en, ScoreVerdict.intact), 'Thesis intact — hold / keep contributing');
      expect(verdictText(en, ScoreVerdict.neutral), 'Neutral — monitor closely');
      expect(verdictText(en, ScoreVerdict.reduceRisk), 'Reduce risk');
      expect(signalText(en, Signal.green.wire), 'Green');
      expect(signalText(en, Signal.neutral.wire), 'Neutral');
      expect(signalText(en, Signal.red.wire), 'Red');
      expect(signalText(en, 'OUTRO'), '—');
    });
  });

  group('ponto do histórico tocado', () {
    test('pt: verdes e registrados sobre o total, com o veredito curto', () {
      expect(
        historyPointText(pt, _point(green: 14, logged: 18, total: 20, verdict: ScoreVerdict.intact)),
        '14 verdes de 20 · 18 registrados · Tese intacta',
      );
      expect(
        historyPointText(pt, _point(green: 8, logged: 20, total: 20)),
        '8 verdes de 20 · 20 registrados · Neutro',
      );
      expect(
        historyPointText(pt, _point(green: 2, logged: 5, total: 20, verdict: ScoreVerdict.reduceRisk)),
        '2 verdes de 20 · 5 registrados · Reduzir risco',
      );
    });

    test('pt: singular com 1 e "0 verdes" no zero', () {
      expect(
        historyPointText(pt, _point(green: 1, logged: 1, total: 8, verdict: ScoreVerdict.reduceRisk)),
        '1 verde de 8 · 1 registrado · Reduzir risco',
      );
      expect(
        historyPointText(pt, _point(green: 0, logged: 0, total: 8, verdict: ScoreVerdict.reduceRisk)),
        '0 verdes de 8 · 0 registrados · Reduzir risco',
      );
    });

    test('en: plural e singular', () {
      expect(
        historyPointText(en, _point(green: 14, logged: 18, total: 20, verdict: ScoreVerdict.intact)),
        '14 greens of 20 · 18 logged · Thesis intact',
      );
      expect(
        historyPointText(en, _point(green: 1, logged: 1, total: 8, verdict: ScoreVerdict.reduceRisk)),
        '1 green of 8 · 1 logged · Reduce risk',
      );
      expect(
        historyPointText(en, _point(green: 0, logged: 0, total: 8, verdict: ScoreVerdict.reduceRisk)),
        '0 greens of 8 · 0 logged · Reduce risk',
      );
    });
  });

  group('períodos do gráfico', () {
    test('só "Tudo/All" muda de texto; 30D, 90D e 12M são iguais nos dois idiomas', () {
      expect(HistoryPeriod.values.map((p) => periodLabel(pt, p)), ['30D', '90D', '12M', 'Tudo']);
      expect(HistoryPeriod.values.map((p) => periodLabel(en, p)), ['30D', '90D', '12M', 'All']);
    });
  });

  group('faixas verde/vermelho', () {
    test('pt: maior é melhor, menor é melhor e vírgula decimal', () {
      expect(bandsText(pt, 2.0, 0.0), 'Verde ≥ 2 · Vermelho ≤ 0');
      expect(bandsText(pt, 0.0, 7.0), 'Verde ≤ 0 · Vermelho ≥ 7');
      expect(bandsText(pt, 0.5, 4.0), 'Verde ≤ 0,5 · Vermelho ≥ 4');
    });

    test('en: maior é melhor, menor é melhor e ponto decimal', () {
      expect(bandsText(en, 2.0, 0.0), 'Green ≥ 2 · Red ≤ 0');
      expect(bandsText(en, 0.0, 7.0), 'Green ≤ 0 · Red ≥ 7');
      expect(bandsText(en, 0.5, 4.0), 'Green ≤ 0.5 · Red ≥ 4');
    });
  });

  group('erros do rascunho de limites', () {
    test('pt', () {
      expect(boundaryErrorText(pt, BoundaryDraftError.needTwoNumbers), 'Digite dois números.');
      expect(
        boundaryErrorText(pt, BoundaryDraftError.needDifferentNumbers),
        'Use dois números diferentes.',
      );
    });

    test('en', () {
      expect(boundaryErrorText(en, BoundaryDraftError.needTwoNumbers), 'Enter two numbers.');
      expect(
        boundaryErrorText(en, BoundaryDraftError.needDifferentNumbers),
        'Use two different numbers.',
      );
    });
  });
}

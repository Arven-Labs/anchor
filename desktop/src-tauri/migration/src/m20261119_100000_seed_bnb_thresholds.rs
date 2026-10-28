use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 31 — score próprio do BNB, quinto conjunto depois de ETH, BTC, ADA e XRP.
// **Só de mercado**: o CoinMetrics Community parou o on-chain do BNB em 2019
// (MVRV, Puell e endereços ativos vêm `null`) e não há fonte grátis sem chave.
// Limites por moeda (`indicator_thresholds.coin`); direção inferida pelos
// limites, como nos demais: `green < red` = menor é melhor. Calibrados com
// dados reais do BNB em 2026-10-27 (CoinMetrics, DefiLlama e Binance), ponto de
// partida ajustável no editor:
// - `mayer_multiple`: 2292 dias, p75 1,28 e p90 1,55 (hoje 1,14), então os
//   1,3/2,0 dos outros servem.
// - `tvl_trend` (TVL da BSC, variação em 30 dias): nos últimos 2 anos p10 -9,8,
//   mediana 1,1, p90 13,8 (hoje -5,7). 0/-10 como no ETH, com o vermelho no p10.
// - `funding_rate_7d` (% ao ano): p10 1,1, mediana 3,8, p90 7,0, máximo 10,8
//   (teto de 0,01% por 8h da Binance). 5/10 como ADA e XRP.
// - `long_short_ratio`: só 30 dias de histórico (a Binance limita): de 2,08 a
//   2,90, mediana 2,34, hoje 2,52. 2,2/3,0 cobre a faixa observada; **amostra
//   curta**, ajustar com mais leituras.
// - `nvt_ratio`, `open_interest_change_30d`, `stablecoin_supply_change_30d` e
//   `fear_greed_index`: **reaproveitam os limites dos outros** (razões ou
//   percentuais sem escala própria da moeda). O OI não tem histórico para
//   calibrar (30 dias na Binance).
// Fora do conjunto: MVRV, Puell, endereços ativos, netflow e supply das exchanges
// (sem dado atual), DVOL e put/call (a Deribit não lista BNB), net issuance, fees
// vs emission, staking, ETH/BTC e dominância (são do ETH).
// Duplicados em `domain::crypto_score::BNB_DEFAULT_THRESHOLDS`.
const BNB_SEEDS: [(&str, f64, f64); 8] = [
    ("nvt_ratio", 0.9, 1.3),
    ("tvl_trend", 0.0, -10.0),
    ("funding_rate_7d", 5.0, 10.0),
    ("open_interest_change_30d", 0.0, 25.0),
    ("long_short_ratio", 2.2, 3.0),
    ("stablecoin_supply_change_30d", 2.0, 0.0),
    ("fear_greed_index", 25.0, 75.0),
    ("mayer_multiple", 1.3, 2.0),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let now = chrono::Utc::now().to_rfc3339();
        let db = manager.get_connection();
        for (indicator, green_boundary, red_boundary) in BNB_SEEDS {
            // `OR IGNORE`: `(coin, indicator)` é único — rodar de novo não
            // duplica nem sobrescreve um ajuste feito pelo usuário.
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO indicator_thresholds (coin, indicator, green_boundary, red_boundary, updated_at) VALUES ('BNB', '{indicator}', {green_boundary}, {red_boundary}, '{now}')"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DELETE FROM indicator_thresholds WHERE coin = 'BNB'")
            .await?;
        Ok(())
    }
}

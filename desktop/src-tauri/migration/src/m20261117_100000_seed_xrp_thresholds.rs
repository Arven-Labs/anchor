use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 31 — score próprio do XRP, quarto conjunto depois de ETH, BTC e ADA.
// Limites por moeda (`indicator_thresholds.coin`); direção inferida pelos
// limites, como nos demais: `green < red` = menor é melhor. Calibrados com
// dados reais do XRP em 2026-10-26 (CoinMetrics Community e Binance), ponto de
// partida ajustável no editor:
// - `mvrv_z_score`: hoje -0,26; histórico de 4090 dias de -1,01 a 16,6 (topo de
//   2018), p90 2,19. Vermelho em 5: perto dos topos, sem esperar o de 2018.
// - `active_addresses_trend`: hoje -14%; p25 -23, mediana 0,7, p75 35. Muito
//   volátil, então o vermelho é -25 em vez de -10.
// - `funding_rate_7d` (% ao ano): hoje 7,4; p10 -3,7, mediana 1,6, p90 6,0,
//   máximo 10,1 (teto de 0,01% por 8h da Binance). 15 ou 20 nunca seriam atingidos.
// - `long_short_ratio`: só 30 dias de histórico (a Binance limita): de 2,21 a
//   3,20, mediana 2,58, hoje 2,97. 2,3/3,2 cobre a faixa observada; **amostra
//   curta**, ajustar com mais leituras.
// - `mayer_multiple`: p75 1,15 e p90 1,52 (hoje 1,01), então os 1,3/2,0 dos
//   outros servem.
// - `nvt_ratio`, `open_interest_change_30d`, `stablecoin_supply_change_30d` e
//   `fear_greed_index`: **reaproveitam os limites do ETH, BTC e ADA** (razões ou
//   percentuais sem escala própria da moeda). O OI não tem histórico para
//   calibrar (30 dias na Binance).
// Fora do conjunto: `puell_multiple` (o Community não tem `IssTotUSD` para XRP),
// exchange netflow e supply (sem `FlowInExUSD`/`FlowOutExUSD`/`SplyExNtv`), DVOL
// e put/call (a Deribit não lista XRP), fundamentos de rede e dominância (são do ETH).
// Duplicados em `domain::crypto_score::XRP_DEFAULT_THRESHOLDS`.
const XRP_SEEDS: [(&str, f64, f64); 9] = [
    ("mvrv_z_score", 0.0, 5.0),
    ("nvt_ratio", 0.9, 1.3),
    ("active_addresses_trend", 0.0, -25.0),
    ("funding_rate_7d", 5.0, 10.0),
    ("open_interest_change_30d", 0.0, 25.0),
    ("long_short_ratio", 2.3, 3.2),
    ("stablecoin_supply_change_30d", 2.0, 0.0),
    ("fear_greed_index", 25.0, 75.0),
    ("mayer_multiple", 1.3, 2.0),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let now = chrono::Utc::now().to_rfc3339();
        let db = manager.get_connection();
        for (indicator, green_boundary, red_boundary) in XRP_SEEDS {
            // `OR IGNORE`: `(coin, indicator)` é único — rodar de novo não
            // duplica nem sobrescreve um ajuste feito pelo usuário.
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO indicator_thresholds (coin, indicator, green_boundary, red_boundary, updated_at) VALUES ('XRP', '{indicator}', {green_boundary}, {red_boundary}, '{now}')"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DELETE FROM indicator_thresholds WHERE coin = 'XRP'")
            .await?;
        Ok(())
    }
}

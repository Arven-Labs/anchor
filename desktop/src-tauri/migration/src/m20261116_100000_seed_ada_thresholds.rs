use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 31 — score próprio do ADA, terceiro conjunto depois de ETH e BTC.
// Limites por moeda (`indicator_thresholds.coin`); direção inferida pelos
// limites, como nos demais: `green < red` = menor é melhor. Calibrados com
// dados reais do ADA em 2026-10-26 (CoinMetrics Community e Binance), ponto de
// partida ajustável no editor:
// - `mvrv_z_score`: hoje -0,36; histórico de 2886 dias de -0,61 a 4,50 (p90
//   0,95). Vermelho em 3,5, perto do topo de 2021.
// - `puell_multiple`: hoje 0,25; p25 0,23, mediana 0,50, p90 2,42. Vermelho em
//   2,5 (≈ p90); a emissão do ADA é de staking, não de mineração, então o
//   sentido é de "recompensa de hoje contra a média do ano".
// - `active_addresses_trend`: hoje -29,6%; p25 -19,8, mediana -1,6, p75 22,6.
//   Mais volátil que ETH/BTC, então o vermelho é -20 em vez de -10.
// - `funding_rate_7d` (% ao ano): hoje 2,5; p10 -5,6, mediana 2,4, p90 8,8,
//   máximo 10,95 (teto de 0,01% por 8h da Binance). 15 ou 20, como no BTC e no
//   ETH, nunca seriam atingidos.
// - `long_short_ratio`: só 30 dias de histórico (a Binance limita): de 1,76 a
//   2,58, mediana 2,02, hoje 1,84. 1,8/2,6 cobre a faixa observada; **amostra
//   curta**, ajustar com mais leituras.
// - `nvt_ratio`, `open_interest_change_30d`, `stablecoin_supply_change_30d`,
//   `fear_greed_index` e `mayer_multiple`: **reaproveitam os limites do ETH e do
//   BTC** (são razões ou percentuais sem escala própria da moeda). O OI não tem
//   histórico para calibrar (30 dias na Binance); o Mayer do ADA tem p75 1,25 e
//   p90 1,79, perto dos 1,3/2,0 usados nos outros.
// Fora do conjunto: exchange netflow e supply (o Community não tem
// `FlowInExUSD`/`FlowOutExUSD`/`SplyExNtv` para ADA), DVOL e put/call (a
// Deribit não lista ADA), fundamentos de rede e dominância (são do ETH).
// Duplicados em `domain::crypto_score::ADA_DEFAULT_THRESHOLDS`.
const ADA_SEEDS: [(&str, f64, f64); 10] = [
    ("mvrv_z_score", 0.0, 3.5),
    ("nvt_ratio", 0.9, 1.3),
    ("puell_multiple", 0.5, 2.5),
    ("active_addresses_trend", 0.0, -20.0),
    ("funding_rate_7d", 5.0, 10.0),
    ("open_interest_change_30d", 0.0, 25.0),
    ("long_short_ratio", 1.8, 2.6),
    ("stablecoin_supply_change_30d", 2.0, 0.0),
    ("fear_greed_index", 25.0, 75.0),
    ("mayer_multiple", 1.3, 2.0),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let now = chrono::Utc::now().to_rfc3339();
        let db = manager.get_connection();
        for (indicator, green_boundary, red_boundary) in ADA_SEEDS {
            // `OR IGNORE`: `(coin, indicator)` é único — rodar de novo não
            // duplica nem sobrescreve um ajuste feito pelo usuário.
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO indicator_thresholds (coin, indicator, green_boundary, red_boundary, updated_at) VALUES ('ADA', '{indicator}', {green_boundary}, {red_boundary}, '{now}')"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DELETE FROM indicator_thresholds WHERE coin = 'ADA'")
            .await?;
        Ok(())
    }
}

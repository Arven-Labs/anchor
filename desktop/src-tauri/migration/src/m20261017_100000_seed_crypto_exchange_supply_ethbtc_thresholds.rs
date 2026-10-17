use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 31, lote 3 — thresholds iniciais da oferta nas exchanges e da força
// relativa ETH/BTC (CoinMetrics Community). Direção inferida pelos limites,
// como nos demais: `green < red` = menor é melhor. Calibrados com 70 janelas
// reais de 30 dias em 2026-10-17: oferta nas exchanges entre −3,4% e +1,7%
// (mediana −1,8%, hoje +0,9%); ETH/BTC entre −8,4% e +11,4% (mediana +4,1%,
// hoje −6,4%). Ponto de partida, ajustável no editor de thresholds.
const SEEDS: [(&str, f64, f64); 2] = [
    // ETH saindo das exchanges é acumulação: menor é melhor.
    ("exchange_supply_change_30d", -1.0, 1.5),
    // ETH ganhando do BTC é apetite por risco: maior é melhor.
    ("eth_btc_trend_30d", 3.0, -3.0),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let now = chrono::Utc::now().to_rfc3339();
        let db = manager.get_connection();
        for (indicator, green_boundary, red_boundary) in SEEDS {
            // `OR IGNORE`: `indicator` é único — rodar de novo não duplica nem
            // sobrescreve um ajuste feito pelo usuário.
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO indicator_thresholds (indicator, green_boundary, red_boundary, updated_at) VALUES ('{indicator}', {green_boundary}, {red_boundary}, '{now}')"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (indicator, _, _) in SEEDS {
            db.execute_unprepared(&format!(
                "DELETE FROM indicator_thresholds WHERE indicator = '{indicator}'"
            ))
            .await?;
        }
        Ok(())
    }
}

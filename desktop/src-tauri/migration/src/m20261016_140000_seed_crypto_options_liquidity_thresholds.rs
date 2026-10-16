use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 31, lote 2 — thresholds iniciais de opções (Deribit) e liquidez de
// stablecoins (DefiLlama). Direção inferida pelos limites, como nos demais:
// `green < red` = menor é melhor. Calibrados com valores reais de 2026-10-16:
// DVOL do ETH entre 46 e 75 em 200 dias (mediana 53, hoje 51); put/call por
// open interest hoje em 0,52; variação de 30 dias das stablecoins entre −2,8%
// e +5,8% no último ano (mediana +0,5%, hoje +0,77%). Ponto de partida,
// ajustável no editor de thresholds.
const SEEDS: [(&str, f64, f64); 3] = [
    // Volatilidade implícita alta é estresse: menor é melhor.
    ("dvol", 55.0, 75.0),
    // Medo (muitos puts) é oportunidade, calma demais é topo: maior é melhor.
    ("put_call_ratio", 0.7, 0.4),
    // Liquidez entrando no mercado: maior é melhor.
    ("stablecoin_supply_change_30d", 2.0, 0.0),
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

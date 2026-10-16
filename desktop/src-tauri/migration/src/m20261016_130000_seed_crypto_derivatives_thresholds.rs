use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 31 — thresholds iniciais dos 3 indicadores de derivativos de ETH
// (Binance Futures). Direção inferida pelos limites, como nos 9 originais:
// `green < red` = menor é melhor. Calibrados com os valores reais de
// 2026-10-16 (funding 7d anualizado entre −4,8% e 10,2% nos últimos meses;
// long/short de contas entre 2,1 e 3,4 no último mês; OI do ETH −2,1% em 30
// dias). São ponto de partida, ajustáveis direto em `indicator_thresholds`
// (não há editor na UI ainda).
const SEEDS: [(&str, f64, f64); 3] = [
    ("funding_rate_7d", 5.0, 20.0),
    ("open_interest_change_30d", 0.0, 25.0),
    ("long_short_ratio", 2.0, 3.5),
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

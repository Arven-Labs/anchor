use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 31 — `sopr` e `nupl` no score do BTC (bitcoin-data.com, só BTC). Limites
// por moeda (`indicator_thresholds.coin`); direção inferida pelos limites, como
// nos demais: `green < red` = menor é melhor. Calibrados com a série diária de
// 2022-10-26 a 2026-10-12 (1448 dias, o que o plano grátis entrega), ponto de
// partida ajustável no editor:
// - `sopr` (razão do lucro realizado das moedas gastas; 1 = no zero a zero):
//   p05 0,986, p25 0,999, mediana 1,005, p75 1,012, p95 1,033, mínimo 0,871,
//   máximo 1,199. Verde ≤ 0,995 (gasto no prejuízo, capitulação) cai em 14% dos
//   dias; vermelho ≥ 1,02 (realização de lucro forte) em 13%.
// - `nupl` (lucro não realizado líquido, de -1 a 1): p10 0,158, p25 0,256,
//   mediana 0,431, p75 0,537, p95 0,594, máximo 0,640 (a janela de 4 anos não
//   inclui o topo de 2021). Verde ≤ 0,25 (medo e esperança) em 24% dos dias.
//   **O vermelho em 0,6 é parte palpite**: a janela nunca passou de 0,64, e as
//   bordas clássicas de euforia (0,75) ficariam inalcançáveis nesses dados.
// A fonte entrega o dado com 7 dias de atraso e limita a 10 chamadas por hora e
// 15 por dia.
// Duplicados em `domain::crypto_score::BTC_DEFAULT_THRESHOLDS`.
const BTC_SEEDS: [(&str, f64, f64); 2] = [("sopr", 0.995, 1.02), ("nupl", 0.25, 0.6)];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let now = chrono::Utc::now().to_rfc3339();
        let db = manager.get_connection();
        for (indicator, green_boundary, red_boundary) in BTC_SEEDS {
            // `OR IGNORE`: `(coin, indicator)` é único — rodar de novo não
            // duplica nem sobrescreve um ajuste feito pelo usuário.
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO indicator_thresholds (coin, indicator, green_boundary, red_boundary, updated_at) VALUES ('BTC', '{indicator}', {green_boundary}, {red_boundary}, '{now}')"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (indicator, _, _) in BTC_SEEDS {
            db.execute_unprepared(&format!(
                "DELETE FROM indicator_thresholds WHERE coin = 'BTC' AND indicator = '{indicator}'"
            ))
            .await?;
        }
        Ok(())
    }
}

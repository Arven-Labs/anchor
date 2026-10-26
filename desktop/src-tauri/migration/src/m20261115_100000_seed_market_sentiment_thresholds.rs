use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 31, lote 4 — grupo "Market sentiment". Limites por moeda
// (`indicator_thresholds.coin`, desde a `m20261017_110000`); direção inferida
// pelos limites, como nos demais: `green < red` = menor é melhor. Calibrados
// com valores reais de 2026-10-26, ponto de partida ajustável no editor:
// - `fear_greed_index`: 0 a 100, histórico completo de 3186 dias entre 5 e 95
//   (mediana 44; hoje 50). 25/75 são as bordas "Extreme Fear"/"Extreme Greed"
//   do próprio índice: medo extremo é oportunidade, ganância extrema é topo.
// - `mayer_multiple`: preço ÷ média de 200 dias. BTC hoje 1,10 (máximo 2,83,
//   mediana de 3 anos 1,10, p90 1,41); ETH hoje 1,08 (máximo 2,93, mediana de
//   3 anos 1,01, p90 1,45). Acima de 2 é sobrecompra histórica.
// - `btc_dominance` (só ETH): hoje 60,0% pela CoinGecko. **Sem calibragem
//   histórica** (a fonte só dá o valor de hoje): 55/65 é palpite em torno do
//   nível atual, menor é melhor para o ETH (dinheiro saindo do BTC).
// Duplicados em `domain::crypto_score::{ETH,BTC}_DEFAULT_THRESHOLDS`.
const ETH_SEEDS: [(&str, f64, f64); 3] =
    [("fear_greed_index", 25.0, 75.0), ("mayer_multiple", 1.3, 2.0), ("btc_dominance", 55.0, 65.0)];
const BTC_SEEDS: [(&str, f64, f64); 2] = [("fear_greed_index", 25.0, 75.0), ("mayer_multiple", 1.3, 2.0)];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let now = chrono::Utc::now().to_rfc3339();
        let db = manager.get_connection();
        for (coin, seeds) in [("ETH", &ETH_SEEDS[..]), ("BTC", &BTC_SEEDS[..])] {
            for (indicator, green_boundary, red_boundary) in seeds {
                // `OR IGNORE`: `(coin, indicator)` é único — rodar de novo não
                // duplica nem sobrescreve um ajuste feito pelo usuário.
                db.execute_unprepared(&format!(
                    "INSERT OR IGNORE INTO indicator_thresholds (coin, indicator, green_boundary, red_boundary, updated_at) VALUES ('{coin}', '{indicator}', {green_boundary}, {red_boundary}, '{now}')"
                ))
                .await?;
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (indicator, _, _) in ETH_SEEDS {
            db.execute_unprepared(&format!(
                "DELETE FROM indicator_thresholds WHERE indicator = '{indicator}'"
            ))
            .await?;
        }
        Ok(())
    }
}

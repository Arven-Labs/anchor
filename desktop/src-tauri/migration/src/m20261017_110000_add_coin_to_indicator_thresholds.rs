use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 31, score próprio do BTC — os limites passam a ser por moeda. O SQLite
// não troca a constraint única de uma coluna, então a tabela é recriada com
// `UNIQUE(coin, indicator)`; as linhas que já existiam viram `coin = 'ETH'`.
//
// Seeds do BTC calibrados com dados reais de 2026-10-17 (MVRV Z 0,9 com topos
// históricos acima de 7; Puell 1,07 em faixa de 0,5 a 1,6; DVOL 37,5 entre 34
// e 52; funding de 7 dias a 2,8%; long/short 1,53 entre 0,9 e 1,9; netflow
// entre −0,08 e −0,01 em só 100 dias de dado). Ponto de partida, ajustável no
// editor de thresholds. Duplicados em `domain::crypto_score::BTC_DEFAULT_THRESHOLDS`.
const BTC_SEEDS: [(&str, f64, f64); 12] = [
    ("mvrv_z_score", 0.0, 7.0),
    ("nvt_ratio", 0.9, 1.3),
    ("puell_multiple", 0.5, 4.0),
    ("active_addresses_trend", 0.0, -10.0),
    ("exchange_netflow", -0.02, 0.03),
    ("exchange_supply_change_30d", -1.0, 2.0),
    ("funding_rate_7d", 5.0, 15.0),
    ("open_interest_change_30d", 0.0, 25.0),
    ("long_short_ratio", 1.3, 2.0),
    ("dvol", 40.0, 55.0),
    ("put_call_ratio", 0.7, 0.4),
    ("stablecoin_supply_change_30d", 2.0, 0.0),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE indicator_thresholds_new (\
                id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, \
                coin VARCHAR NOT NULL, \
                indicator VARCHAR NOT NULL, \
                green_boundary DOUBLE NOT NULL, \
                red_boundary DOUBLE NOT NULL, \
                updated_at VARCHAR NOT NULL, \
                UNIQUE (coin, indicator))",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO indicator_thresholds_new (coin, indicator, green_boundary, red_boundary, updated_at) \
             SELECT 'ETH', indicator, green_boundary, red_boundary, updated_at FROM indicator_thresholds",
        )
        .await?;
        db.execute_unprepared("DROP TABLE indicator_thresholds").await?;
        db.execute_unprepared("ALTER TABLE indicator_thresholds_new RENAME TO indicator_thresholds").await?;

        let now = chrono::Utc::now().to_rfc3339();
        for (indicator, green_boundary, red_boundary) in BTC_SEEDS {
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO indicator_thresholds (coin, indicator, green_boundary, red_boundary, updated_at) VALUES ('BTC', '{indicator}', {green_boundary}, {red_boundary}, '{now}')"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE indicator_thresholds_old (\
                id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, \
                indicator VARCHAR NOT NULL UNIQUE, \
                green_boundary DOUBLE NOT NULL, \
                red_boundary DOUBLE NOT NULL, \
                updated_at VARCHAR NOT NULL)",
        )
        .await?;
        // Só o ETH volta: o schema antigo não distingue moedas.
        db.execute_unprepared(
            "INSERT INTO indicator_thresholds_old (indicator, green_boundary, red_boundary, updated_at) \
             SELECT indicator, green_boundary, red_boundary, updated_at FROM indicator_thresholds WHERE coin = 'ETH'",
        )
        .await?;
        db.execute_unprepared("DROP TABLE indicator_thresholds").await?;
        db.execute_unprepared("ALTER TABLE indicator_thresholds_old RENAME TO indicator_thresholds").await?;
        Ok(())
    }
}

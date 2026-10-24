use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fundos fechados e exclusivos (Lei 14.754/2023). `fund_details.structure` diz se o fundo e `aberto` (padrao, o que ja
// existia), `fechado` ou `exclusivo`: o IR do art. 17 e o mesmo nos tres (art. 17, par. 8), mas so os fechados e exclusivos
// tem estoque de rendimento a tributar. `fund_details.estoque_option` diz como o estoque foi tributado: nulo = nao tributado
// (ou contestado na Justica), `geral` = 15% (art. 27), `opcao_8` = 8% por opcao (art. 28).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE fund_details ADD COLUMN structure TEXT NOT NULL DEFAULT 'aberto'").await?;
        db.execute_unprepared("ALTER TABLE fund_details ADD COLUMN estoque_option TEXT").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE fund_details DROP COLUMN estoque_option").await?;
        db.execute_unprepared("ALTER TABLE fund_details DROP COLUMN structure").await?;
        Ok(())
    }
}

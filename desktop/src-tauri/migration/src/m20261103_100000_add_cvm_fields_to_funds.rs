use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Valor da cota lido do informe diario da CVM. `fund_quota_values.source` diz de onde veio a cota (`manual` = lancada pelo
// usuario, `cvm` = lida do informe; toda linha que ja existe e manual) e a leitura da CVM nunca sobrescreve uma manual.
// `fund_details.cvm_subclass` escolhe a subclasse quando o CNPJ da classe so tem subclasses no informe (nulo = a linha da
// classe, sem subclasse). O CNPJ do fundo fica em `assets.cnpj`, que ja existe.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE fund_quota_values ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'").await?;
        db.execute_unprepared("ALTER TABLE fund_details ADD COLUMN cvm_subclass TEXT").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE fund_details DROP COLUMN cvm_subclass").await?;
        db.execute_unprepared("ALTER TABLE fund_quota_values DROP COLUMN source").await?;
        Ok(())
    }
}

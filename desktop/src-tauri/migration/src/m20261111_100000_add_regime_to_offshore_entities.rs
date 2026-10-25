use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Regime da controlada no exterior (Lei 14.754, arts. 5º e 6º): `automatico` = lucro tributado todo 31/12 (as que caem no
// §5º do art. 5º); `ativa` = só é tributada quando o lucro é disponibilizado (art. 6º). Quem diz é o usuário: o app não
// mede a renda ativa nem confere a lista de países de tributação favorecida. As entidades que já existem ficam `automatico`.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE offshore_entities ADD COLUMN regime TEXT NOT NULL DEFAULT 'automatico'")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("ALTER TABLE offshore_entities DROP COLUMN regime").await?;
        Ok(())
    }
}

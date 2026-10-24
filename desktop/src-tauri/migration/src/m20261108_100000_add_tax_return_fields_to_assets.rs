use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Grupo, código e país da ficha "Bens e Direitos" da declaração, informados pelo usuário em cada ativo (o app não os adivinha:
// as fontes divergem e a tabela da Receita não é lida aqui). Texto, com os zeros à esquerda ("03", "01", "249"). Nulo = não
// informado (todas as linhas que já existem).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for column in ["dirpf_group", "dirpf_code", "dirpf_country"] {
            db.execute_unprepared(&format!("ALTER TABLE assets ADD COLUMN {column} TEXT")).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for column in ["dirpf_group", "dirpf_code", "dirpf_country"] {
            db.execute_unprepared(&format!("ALTER TABLE assets DROP COLUMN {column}")).await?;
        }
        Ok(())
    }
}

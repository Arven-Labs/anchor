use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// A descrição da regra do CDB dizia que o IOF não era modelado. Agora ele é (tabela do decreto
/// em `domain::tax::iof`, descontado antes do IR), e na amostra real foi o que fez o modelo
/// acertar o valor que o banco pagou. Só o texto muda, nunca o valor que o usuário editou.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE tax_rules SET description = 'IR withheld on CDB redemptions, as a flat rate (22.5% is the first bracket, up to 180 days; applied after IOF). The regressive IR table is not modeled' \
                 WHERE rule_key = 'fixed_income_ir_cdb' AND description LIKE '%IOF are not modeled'",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

use sea_orm_migration::prelude::*;

/// Fase 20 (Sessão 101) — toda a IA saiu do Anchor (chat, extração de PDF,
/// "About the asset", avisos de proventos da CVM lidos por IA); o Warden virou
/// a única interface de IA do ecossistema. Apaga as tabelas que só essas
/// features usavam. Ordem: filhas antes das mães (`ai_valuation_proposal` →
/// `ai_message` → `ai_conversation` → `ai_api_key`).
///
/// Irreversível de propósito: recriar o schema vazio não devolve o dado, e o
/// código que lia essas tabelas não existe mais. Pra guardar uma cópia, rodar
/// o Backup do Workspace (Fase 16) antes de aplicar.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            Alias::new("ai_valuation_proposal"),
            Alias::new("ai_message"),
            Alias::new("ai_conversation"),
            Alias::new("ai_api_key"),
            Alias::new("company_ai_info"),
            Alias::new("cvm_dividend_notice_documents"),
        ] {
            manager
                .drop_table(Table::drop().table(table).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "m20261014_100000_drop_ai_tables is irreversible".to_owned(),
        ))
    }
}

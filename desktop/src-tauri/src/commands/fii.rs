use std::sync::atomic::{AtomicBool, Ordering};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::commands::collector::CollectorSummary;
use crate::entity::{fii_cvm_monthly, fii_cvm_properties};
use crate::error::AppError;
use crate::finance_api::fii as finance_api_fii;
use crate::finance_core::cvm_fii_dfin;

/// Fase 10, item 8, Sessão 41 — sugestão de CNPJ pro cadastro de um FII,
/// resolvida cruzando bolsai + cadastro público da CVM (nunca chuta — 0 ou
/// mais de um match vira `None`, ver `finance_core::cvm_fii::resolve_of`).
/// O usuário confirma/edita antes de salvar em `assets.cnpj`
/// — este comando não escreve nada lá, só no cache de resolução.
#[derive(Serialize, Deserialize)]
pub struct FiiCnpjSuggestion {
    pub cnpj: String,
    pub fund_name: String,
}

/// Resolve ticker → CNPJ do fundo (`finance_api::fii::resolve_cnpj`: cache local,
/// depois bolsai + cadastro da CVM). Sem `lock`/`app` — a chamada não precisa do
/// lock de coletor (frontend só passa `{ ticker }`, ver
/// `AssetSection.tsx`/`FiiLookupSection.tsx`).
#[tauri::command]
pub async fn resolve_fii_cnpj(
    db: tauri::State<'_, DatabaseConnection>,
    ticker: String,
) -> Result<Option<FiiCnpjSuggestion>, AppError> {
    let resolution = finance_api_fii::resolve_cnpj(db.inner(), &ticker).await?;

    Ok(resolution.map(|r| FiiCnpjSuggestion {
        cnpj: r.cnpj,
        fund_name: r.fund_name,
    }))
}

/// Puxa indicadores mensais + imóveis (vacância/inadimplência) direto da
/// CVM pra uma lista de CNPJs já resolvidos e salvos — a bolsai não entra
/// aqui, só na sugestão inicial (`resolve_fii_cnpj` acima).
///
/// Chama `finance_api::fii::collect_cvm_data` direto.
#[tauri::command]
pub async fn run_fii_cvm_collector(
    lock: tauri::State<'_, AtomicBool>,
    db: tauri::State<'_, DatabaseConnection>,
    cnpjs: Vec<String>,
) -> Result<CollectorSummary, AppError> {
    if lock.swap(true, Ordering::SeqCst) {
        return Err(AppError::CollectorBusy);
    }
    let result = finance_api_fii::collect_cvm_data(db.inner(), &cnpjs)
        .await
        .map(|r| CollectorSummary {
            success: true,
            output: format!(
                "Fetched {} monthly indicator(s) and {} propert(y/ies)",
                r.monthly_count, r.properties_count,
            ),
        });
    lock.store(false, Ordering::SeqCst);

    result
}

#[tauri::command]
pub async fn list_fii_cvm_monthly(
    db: tauri::State<'_, DatabaseConnection>,
    cnpj: String,
) -> Result<Vec<fii_cvm_monthly::Model>, AppError> {
    let rows = fii_cvm_monthly::Entity::find()
        .filter(fii_cvm_monthly::Column::Cnpj.eq(cnpj))
        .order_by_asc(fii_cvm_monthly::Column::ReferenceDate)
        .all(db.inner())
        .await?;

    Ok(rows)
}

#[tauri::command]
pub async fn list_fii_cvm_properties(
    db: tauri::State<'_, DatabaseConnection>,
    cnpj: String,
) -> Result<Vec<fii_cvm_properties::Model>, AppError> {
    let rows = fii_cvm_properties::Entity::find()
        .filter(fii_cvm_properties::Column::Cnpj.eq(cnpj))
        .order_by_desc(fii_cvm_properties::Column::ReferenceDate)
        .all(db.inner())
        .await?;

    Ok(rows)
}

/// Uma demonstração financeira do fundo, do `DFIN` da CVM (ver `finance_core::cvm_fii_dfin`). Só o link para o PDF: nada
/// é baixado nem lido, e quem abre o link é o usuário.
#[derive(Serialize, Debug, PartialEq)]
pub struct FiiFinancialStatementView {
    pub reference_date: String,
    pub delivered_on: String,
    pub version: u32,
    pub auditor_opinion: String,
    pub pdf_url: String,
}

#[tauri::command]
pub async fn list_fii_financial_statements(cnpj: String) -> Result<Vec<FiiFinancialStatementView>, AppError> {
    Ok(cvm_fii_dfin::financial_statements(&cnpj)
        .await?
        .into_iter()
        .map(|s| FiiFinancialStatementView {
            reference_date: s.reference_date.to_string(),
            delivered_on: s.delivered_on.to_string(),
            version: s.version,
            auditor_opinion: s.auditor_opinion,
            pdf_url: s.link,
        })
        .collect())
}

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    Unchanged,
};
use serde::Deserialize;

use crate::entity::{assets, fii_cnpj_cache};
use crate::error::AppError;

/// Mantém `fii_cnpj_cache` (Sessão 43, usada pela tela de Pesquisa) em
/// sincronia sempre que `assets.cnpj` é escrito por aqui — sem isso, corrigir
/// o CNPJ de um FII em Ativos não apareceria na Pesquisa (ela lê o cache,
/// não `assets`), quebrando a expectativa do dono do projeto de que edição
/// numa tela reflita na outra. Find-then-write (sem upsert nativo do
/// SeaORM aqui) — mesmo estilo do resto do arquivo.
async fn upsert_fii_cnpj_cache(
    db: &DatabaseConnection,
    ticker: &str,
    cnpj: &str,
    fund_name: &str,
) -> Result<(), AppError> {
    let existing = fii_cnpj_cache::Entity::find()
        .filter(fii_cnpj_cache::Column::Ticker.eq(ticker))
        .one(db)
        .await?;

    let now = chrono::Utc::now().to_rfc3339();
    match existing {
        Some(row) => {
            fii_cnpj_cache::ActiveModel {
                id: Unchanged(row.id),
                cnpj: Set(cnpj.to_string()),
                fund_name: Set(fund_name.to_string()),
                resolved_at: Set(now),
                ..Default::default()
            }
            .update(db)
            .await?;
        }
        None => {
            fii_cnpj_cache::ActiveModel {
                ticker: Set(ticker.to_string()),
                cnpj: Set(cnpj.to_string()),
                fund_name: Set(fund_name.to_string()),
                resolved_at: Set(now),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }

    Ok(())
}

// Fase 10.2, escopo Sessão 29 — as demais classes expandidas na Sessão 30
// (REIT, metal, imóvel, empresa não listada, ver PHASE.md item 8) ficam pra
// uma fatia futura, junto com as decisões em aberto delas. `fii` entrou na
// Sessão 41, `etf_br` na Sessão 49 (2026-08-02) — mesmo mecanismo de
// listagem B3 e mesmo endpoint Yahoo (`{ticker}.SA`) que `acao_br` já usa,
// sem coletor novo — ver `ASSET_CLASSES_WITH_AUTO_QUOTE` em
// `profitability.rs`. `etf_br` fica só na fatia de cotação por ora — a CVM
// não tem uma categoria de dados abertos própria pra fundo de índice
// (achado pesquisando de verdade, diferente de FII/FIDC/FIP, que têm; ver
// PHASE.md item 8), então sem indicador de fundo específico ainda. `cripto`
// entrou na Sessão 51 — cotação automática também, mas fonte própria
// (CoinGecko, não Yahoo `.SA` — cripto não é listada na B3) via
// `--crypto-ticker` no coletor Python (`commands/collector.rs`). `bdr`
// entrou na Sessão 53 — recibo negociado na B3 representando empresa
// estrangeira (ex.: AAPL34), mas bate no mesmo endpoint Yahoo `.SA` que
// `acao_br`/`fii`/`etf_br` já usam (confirmado ao vivo), zero coletor
// novo, mesmo caminho de `etf_br`: sem fundamentos (bolsai não tem). `metal`
// entrou na Sessão 55 — cotação automática também (só ouro, `XAU`, por ora),
// fonte própria (Yahoo sem `.SA` — contrato futuro do COMEX, `GC=F`, não é
// listado na B3) via `--metal-ticker` no coletor Python, preço convertido
// de onça troy pra grama na fonte (`sources/metais_yahoo.py`). `acao_internacional`
// (ação americana NYSE/NASDAQ) ganhou cotação automática numa fatia 1 —
// mesmo endpoint Yahoo `.SA` do resto, só sem o sufixo (ticker puro, ex.:
// AAPL), via `--us-ticker` no coletor Python; `acoes_yahoo.py` ganhou um
// parâmetro `suffix` (default `.SA`) pra isso em vez de duplicar o módulo.
// Fatia 2 acrescentou fundamentos + DCF via SEC EDGAR (`sources/sec_edgar.py`)
// nas mesmas tabelas `stock_fundamentals`/`stock_dcf_fundamentals` que
// `acao_br` já usa (schema já era agnóstico de moeda/classe, sem migration).
// `imovel` entrou na Sessão 64 (2026-08-08) — cadastro manual (sem fonte de
// dados externa, fora de `ASSET_CLASSES_WITH_AUTO_QUOTE`), mesmo caminho de
// `tesouro_direto`/`renda_fixa` pro form genérico. Ganha histórico de
// avaliações e anexos (escritura/ITBI/IPTU) por `commands/property.rs`, ver
// PHASE.md item 8. `empresa_nao_listada` fechou a lista original da Sessão
// 30 na mesma Sessão 64 — mesmo esqueleto `AtivoManual` do imóvel (histórico
// de avaliações + anexos, reaproveitado sem mudança), mais 3 campos próprios
// em `assets` (`equity_shares_owned`/`equity_total_shares`/
// `equity_company_valuation`, nullable, mesmo padrão do `cnpj` do FII) —
// percentual de participação é sempre calculado a partir desses 2 últimos,
// nunca gravado (decisão explícita da sessão).

const ASSET_CLASSES: [&str; 15] = [
    "acao_br",
    "fii",
    "etf_br",
    "etf_renda_fixa",
    "cripto",
    "bdr",
    "metal",
    "acao_internacional",
    "reit",
    "etf_us",
    "tesouro_direto",
    "renda_fixa",
    "imovel",
    "empresa_nao_listada",
    "fundo",
];
const EXPOSURE_TYPES: [&str; 2] = ["pais", "categoria_especial"];

// Fase 10, item 8, Sessão 86 — subconjunto de `ASSET_CLASSES` que faz
// sentido como "o que um BDR realmente representa por trás" (pedido
// registrado desde a Sessão 53). Sem guard exigindo `asset_class == "bdr"`
// aqui — mesmo espírito solto do `cnpj`/`equity_*`, quem decide quando
// mostrar o campo é o frontend.
const BDR_UNDERLYING_CLASSES: [&str; 5] =
    ["acao_internacional", "etf_us", "reit", "metal", "cripto"];

fn validate_underlying_asset_class(value: &Option<String>) -> Result<(), AppError> {
    if let Some(v) = value {
        if !BDR_UNDERLYING_CLASSES.contains(&v.as_str()) {
            return Err(AppError::InvalidGuard(format!(
                "underlying_asset_class '{v}' invalid (expected one of {BDR_UNDERLYING_CLASSES:?})"
            )));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn list_assets(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<assets::Model>, AppError> {
    Ok(assets::Entity::find()
        .order_by_asc(assets::Column::Ticker)
        .all(db.inner())
        .await?)
}

#[derive(Deserialize)]
pub struct CreateAssetRequest {
    pub ticker: String,
    pub name: String,
    pub asset_class: String,
    pub currency: String,
    pub exchange: Option<String>,
    pub exposure_type: String,
    pub exposure_value: String,
    // Fase 10, item 8, Sessão 41 — só relevante pra `fii` por ora (chave de
    // junção com `fii_cvm_monthly`/`fii_cvm_properties`, resolvida no
    // frontend via `resolve_fii_cnpj` e confirmada pelo usuário antes do
    // submit). `None` pras demais classes, e também pro próprio FII se o
    // usuário não confirmar nenhuma sugestão.
    pub cnpj: Option<String>,
    // Fase 10, item 8, Sessão 64 — só relevante pra `empresa_nao_listada`.
    // `None` pras demais classes.
    pub equity_shares_owned: Option<f64>,
    pub equity_total_shares: Option<f64>,
    pub equity_company_valuation: Option<f64>,
    // Fase 10, item 8, Sessão 86 — só relevante pra `bdr`. `None` pras
    // demais classes.
    pub underlying_asset_class: Option<String>,
}

#[tauri::command]
pub async fn create_asset(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateAssetRequest,
) -> Result<assets::Model, AppError> {
    if !ASSET_CLASSES.contains(&request.asset_class.as_str()) {
        return Err(AppError::InvalidGuard(format!(
            "asset_class '{}' not supported yet (expected one of {ASSET_CLASSES:?})",
            request.asset_class
        )));
    }
    if !EXPOSURE_TYPES.contains(&request.exposure_type.as_str()) {
        return Err(AppError::InvalidGuard(format!(
            "exposure_type '{}' invalid (expected 'pais' or 'categoria_especial')",
            request.exposure_type
        )));
    }
    validate_underlying_asset_class(&request.underlying_asset_class)?;

    let asset = assets::ActiveModel {
        ticker: Set(request.ticker.clone()),
        name: Set(request.name.clone()),
        asset_class: Set(request.asset_class),
        currency: Set(request.currency),
        exchange: Set(request.exchange),
        exposure_type: Set(request.exposure_type),
        exposure_value: Set(request.exposure_value),
        cnpj: Set(request.cnpj.clone()),
        equity_shares_owned: Set(request.equity_shares_owned),
        equity_total_shares: Set(request.equity_total_shares),
        equity_company_valuation: Set(request.equity_company_valuation),
        underlying_asset_class: Set(request.underlying_asset_class),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    if let Some(cnpj) = request.cnpj {
        upsert_fii_cnpj_cache(db.inner(), &request.ticker, &cnpj, &request.name).await?;
    }

    Ok(asset)
}

#[derive(Deserialize)]
pub struct UpdateAssetCnpjRequest {
    pub asset_id: i32,
    pub cnpj: String,
}

/// Comando estreito (não um `update_asset` genérico) — só existe pro caso
/// de `resolve_fii_cnpj` não achar sugestão nenhuma no cadastro (ou o
/// usuário não confirmar), deixando `assets.cnpj` nulo. Sem isso não
/// haveria jeito de colar o CNPJ manualmente depois, já que `assets` não
/// tem nenhum outro comando de edição (catálogo pensado como append-only na
/// Fase 10.2). Mesmo padrão find-by-id→`NotFound`→`Unchanged` de
/// `rename_portfolio`.
#[tauri::command]
pub async fn update_asset_cnpj(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateAssetCnpjRequest,
) -> Result<assets::Model, AppError> {
    let existing = assets::Entity::find_by_id(request.asset_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {}", request.asset_id)))?;

    let updated = assets::ActiveModel {
        id: Unchanged(existing.id),
        cnpj: Set(Some(request.cnpj.clone())),
        ..Default::default()
    }
    .update(db.inner())
    .await?;

    upsert_fii_cnpj_cache(db.inner(), &existing.ticker, &request.cnpj, &existing.name).await?;

    Ok(updated)
}

#[derive(Deserialize)]
pub struct UpdateAssetEquityRequest {
    pub asset_id: i32,
    pub equity_shares_owned: Option<f64>,
    pub equity_total_shares: Option<f64>,
    pub equity_company_valuation: Option<f64>,
}

/// Comando estreito, mesmo espírito de `update_asset_cnpj` — participação
/// societária muda com rodadas de investimento/diluição, então não pode
/// ficar travada no valor do cadastro inicial, mas `assets` continua sem
/// `update_asset` genérico.
#[tauri::command]
pub async fn update_asset_equity(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateAssetEquityRequest,
) -> Result<assets::Model, AppError> {
    let existing = assets::Entity::find_by_id(request.asset_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {}", request.asset_id)))?;

    Ok(assets::ActiveModel {
        id: Unchanged(existing.id),
        equity_shares_owned: Set(request.equity_shares_owned),
        equity_total_shares: Set(request.equity_total_shares),
        equity_company_valuation: Set(request.equity_company_valuation),
        ..Default::default()
    }
    .update(db.inner())
    .await?)
}

#[derive(Deserialize)]
pub struct UpdateAssetDirpfRequest {
    pub asset_id: i32,
    pub group: Option<String>,
    pub code: Option<String>,
    pub country: Option<String>,
}

/// Normaliza um campo da ficha "Bens e Direitos": vazio vira `None`; só dígitos, até `max_len`; completa com zeros à esquerda
/// até `max_len` ("3" → "03"), o que é só formatação. Não confere se o valor existe na tabela da Receita.
pub fn normalize_dirpf_field(label: &str, raw: Option<&str>, max_len: usize) -> Result<Option<String>, AppError> {
    let Some(value) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if !value.chars().all(|c| c.is_ascii_digit()) || value.len() > max_len {
        return Err(AppError::InvalidInput(format!("{label} must be up to {max_len} digits")));
    }
    Ok(Some(format!("{value:0>max_len$}")))
}

/// Grava o grupo, o código e o país da ficha "Bens e Direitos" do ativo (informados pelo usuário; vazio apaga o campo).
#[tauri::command]
pub async fn update_asset_dirpf(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateAssetDirpfRequest,
) -> Result<assets::Model, AppError> {
    update_asset_dirpf_in(db.inner(), request).await
}

pub async fn update_asset_dirpf_in(
    db: &DatabaseConnection,
    request: UpdateAssetDirpfRequest,
) -> Result<assets::Model, AppError> {
    let group = normalize_dirpf_field("group", request.group.as_deref(), 2)?;
    let code = normalize_dirpf_field("code", request.code.as_deref(), 2)?;
    let country = normalize_dirpf_field("country", request.country.as_deref(), 3)?;
    let existing = assets::Entity::find_by_id(request.asset_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {}", request.asset_id)))?;
    Ok(assets::ActiveModel {
        id: Unchanged(existing.id),
        dirpf_group: Set(group),
        dirpf_code: Set(code),
        dirpf_country: Set(country),
        ..Default::default()
    }
    .update(db)
    .await?)
}

#[derive(Deserialize)]
pub struct SetAssetUpdate2023Request {
    pub asset_id: i32,
    /// Saldo de 31/12/2023 na moeda do ativo; `None` apaga a atualização.
    pub value: Option<f64>,
}

/// Grava (ou apaga) o saldo de 31/12/2023 da atualização opcional do art. 14 da Lei 14.754, informado pelo usuário. Não confere
/// se ele tinha direito à opção (declarado na DAA de 2022, não adquirido em 2023) nem se pagou os 8%.
#[tauri::command]
pub async fn set_asset_update_2023(
    db: tauri::State<'_, DatabaseConnection>,
    request: SetAssetUpdate2023Request,
) -> Result<assets::Model, AppError> {
    set_asset_update_2023_in(db.inner(), request).await
}

pub async fn set_asset_update_2023_in(
    db: &DatabaseConnection,
    request: SetAssetUpdate2023Request,
) -> Result<assets::Model, AppError> {
    if request.value.is_some_and(|v| !v.is_finite() || v <= 0.0) {
        return Err(AppError::InvalidInput("the 31/12/2023 value must be above zero".to_string()));
    }
    let existing = assets::Entity::find_by_id(request.asset_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {}", request.asset_id)))?;
    Ok(assets::ActiveModel {
        id: Unchanged(existing.id),
        update_2023_value: Set(request.value),
        ..Default::default()
    }
    .update(db)
    .await?)
}

#[cfg(test)]
mod dirpf_tests {
    use super::normalize_dirpf_field;

    #[test]
    fn blank_stays_empty_and_short_digits_are_zero_padded() {
        assert_eq!(normalize_dirpf_field("group", None, 2).unwrap(), None);
        assert_eq!(normalize_dirpf_field("group", Some("  "), 2).unwrap(), None);
        assert_eq!(normalize_dirpf_field("group", Some("3"), 2).unwrap().as_deref(), Some("03"));
        assert_eq!(normalize_dirpf_field("code", Some(" 01 "), 2).unwrap().as_deref(), Some("01"));
        assert_eq!(normalize_dirpf_field("country", Some("49"), 3).unwrap().as_deref(), Some("049"));
        assert_eq!(normalize_dirpf_field("country", Some("249"), 3).unwrap().as_deref(), Some("249"));
    }

    #[test]
    fn letters_and_too_many_digits_are_refused_with_the_field_name() {
        for (label, raw, max) in [("group", "ab", 2), ("group", "123", 2), ("country", "2499", 3), ("code", "1.5", 2)] {
            let err = normalize_dirpf_field(label, Some(raw), max).unwrap_err().to_string();
            assert!(err.contains(label), "{err}");
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateAssetUnderlyingClassRequest {
    pub asset_id: i32,
    pub underlying_asset_class: Option<String>,
}

/// Comando estreito, mesmo espírito de `update_asset_cnpj` — pedido
/// registrado desde a Sessão 53, fechado na Sessão 86: BDRs cadastrados
/// antes desta fatia ficam com `underlying_asset_class = null`, editável
/// aqui depois (inclusive pra limpar de volta pra `null`, mandando `None`).
#[tauri::command]
pub async fn update_asset_underlying_class(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateAssetUnderlyingClassRequest,
) -> Result<assets::Model, AppError> {
    validate_underlying_asset_class(&request.underlying_asset_class)?;

    let existing = assets::Entity::find_by_id(request.asset_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {}", request.asset_id)))?;

    Ok(assets::ActiveModel {
        id: Unchanged(existing.id),
        underlying_asset_class: Set(request.underlying_asset_class),
        ..Default::default()
    }
    .update(db.inner())
    .await?)
}

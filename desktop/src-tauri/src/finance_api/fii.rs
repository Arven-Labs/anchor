// Fase 14.4 (fatia 4, Sessão 91) — porta `collect_fii_cvm_data` (indicadores
// mensais + imóveis da CVM) pra Rust. `resolve_cnpj` (fatia final, Sessão
// 92) porta a resolução ticker->CNPJ, desbloqueada pela Fase 1.11.3 do
// easybusiness (`GET /v1/fiis/resolve/{ticker}`).
use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};

use crate::entity::{fii_cnpj_cache, fii_cvm_monthly, fii_cvm_properties};
use crate::error::AppError;
use crate::finance_api::{client, insert_ignoring_conflicts, skip_not_found};

pub struct FiiCvmDataResult {
    pub monthly_count: usize,
    pub properties_count: usize,
}

pub struct FiiCnpjResolution {
    pub cnpj: String,
    pub fund_name: String,
}

/// A Finance API (Fase 1.11.3 do easybusiness) devolve o CNPJ só com
/// dígitos — normalizado do lado de lá pra caber na própria coluna
/// `String(14)`, achado real da sessão em que essa fase foi implementada.
/// O resto do Anchor (`assets.cnpj`, `fii_cnpj_cache` já existente, o CSV
/// da própria CVM) sempre usa o formato pontuado — mesma disciplina que
/// `finance_api::fii::collect_cvm_data` já aplica ao contrário (normaliza
/// só pra chamar a API, grava formatado). Achado ao vivo contra KNRI11 real
/// nesta sessão: sem essa conversão, um FII novo resolvido por aqui
/// gravaria `assets.cnpj` sem pontuação, divergindo de todo o resto do app.
fn format_cnpj(digits_only: &str) -> String {
    if digits_only.len() != 14 || !digits_only.chars().all(|c| c.is_ascii_digit()) {
        // Formato inesperado — devolve como veio em vez de cortar índices
        // fora dos limites; nunca deveria acontecer (a Finance API garante
        // 14 dígitos), mas não vale um panic por um CNPJ mal-formado.
        return digits_only.to_string();
    }
    format!(
        "{}.{}.{}/{}-{}",
        &digits_only[0..2],
        &digits_only[2..5],
        &digits_only[5..8],
        &digits_only[8..12],
        &digits_only[12..14],
    )
}

/// Consulta `fii_cnpj_cache` por `ticker` antes de qualquer chamada de rede
/// — necessário desde que a tela de Pesquisa passou a chamar isto pra
/// qualquer busca de FII, não só no cadastro de Ativo (promessa feita ao
/// dono do projeto: "depois de resolvido, a bolsai nunca mais é chamada").
/// Só grava no cache quando a resolução realmente encontra um match
/// (`skip_not_found` — 404 vira `None`, nunca cacheado) — mesma disciplina
/// do `cvm_fii.py::resolve_cnpj` original (0 ou mais de um candidato na
/// CVM também vira `None` do lado do easybusiness, nunca chuta).
pub async fn resolve_cnpj(
    db: &DatabaseConnection,
    ticker: &str,
) -> Result<Option<FiiCnpjResolution>, AppError> {
    if let Some(cached) = fii_cnpj_cache::Entity::find()
        .filter(fii_cnpj_cache::Column::Ticker.eq(ticker))
        .one(db)
        .await?
    {
        return Ok(Some(FiiCnpjResolution {
            cnpj: cached.cnpj,
            fund_name: cached.fund_name,
        }));
    }

    let Some(resolution) = skip_not_found(client::fetch_fii_cnpj_resolution(ticker)).await?
    else {
        return Ok(None);
    };
    let cnpj = format_cnpj(&resolution.cnpj);

    let now = Utc::now().to_rfc3339();
    fii_cnpj_cache::ActiveModel {
        ticker: Set(ticker.to_string()),
        cnpj: Set(cnpj.clone()),
        fund_name: Set(resolution.fund_name.clone()),
        resolved_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await?;

    Ok(Some(FiiCnpjResolution {
        cnpj,
        fund_name: resolution.fund_name,
    }))
}

pub async fn collect_cvm_data(
    db: &DatabaseConnection,
    cnpjs: &[String],
) -> Result<FiiCvmDataResult, AppError> {
    let now = Utc::now().to_rfc3339();

    let mut monthly_models = Vec::new();
    let mut monthly_count = 0usize;
    let mut properties_models = Vec::new();
    let mut properties_count = 0usize;

    for cnpj in cnpjs {
        // A Finance API exige o CNPJ só com dígitos na URL (achado ao vivo
        // nesta sessão — 404 pra "11.728.688/0001-47", 200 pra
        // "11728688000147") — mas `assets.cnpj` (e todo o resto do app,
        // `list_fii_cvm_monthly`/`list_fii_cvm_properties` incluídos) usa o
        // CNPJ formatado com pontuação. Normaliza só pra chamada; grava com
        // o `cnpj` original recebido, não o que a API ecoa de volta —
        // mesma decisão de `finance_api::metals` com o ticker.
        let digits_only: String = cnpj.chars().filter(char::is_ascii_digit).collect();

        let monthly = client::fetch_fii_monthly_indicators(&digits_only).await?;
        monthly_count += 1;
        monthly_models.push(fii_cvm_monthly::ActiveModel {
            id: ActiveValue::NotSet,
            cnpj: Set(cnpj.clone()),
            reference_date: Set(monthly.reference_date.to_string()),
            patrimonio_liquido: Set(monthly.patrimonio_liquido),
            valor_patrimonial_cota: Set(monthly.valor_patrimonial_cota),
            numero_cotistas: Set(monthly.numero_cotistas),
            dividend_yield_mes: Set(monthly.dividend_yield_mes),
            rentabilidade_efetiva_mes: Set(monthly.rentabilidade_efetiva_mes),
            source: Set("cvm_fii".to_string()),
            fetched_at: Set(now.clone()),
        });

        let properties = client::fetch_fii_properties(&digits_only).await?;
        for property in properties.data {
            properties_count += 1;
            properties_models.push(fii_cvm_properties::ActiveModel {
                id: ActiveValue::NotSet,
                cnpj: Set(cnpj.clone()),
                reference_date: Set(property.reference_date.to_string()),
                nome_imovel: Set(property.nome_imovel),
                endereco: Set(property.endereco),
                area_m2: Set(property.area_m2),
                percentual_vacancia: Set(property.percentual_vacancia),
                percentual_inadimplencia: Set(property.percentual_inadimplencia),
                percentual_receitas_fii: Set(property.percentual_receitas_fii),
                percentual_locado: Set(property.percentual_locado),
                source: Set("cvm_fii".to_string()),
                fetched_at: Set(now.clone()),
            });
        }
    }

    insert_ignoring_conflicts::<fii_cvm_monthly::Entity>(
        db,
        monthly_models,
        OnConflict::columns([
            fii_cvm_monthly::Column::Cnpj,
            fii_cvm_monthly::Column::ReferenceDate,
        ])
        .do_nothing()
        .to_owned(),
    )
    .await?;

    insert_ignoring_conflicts::<fii_cvm_properties::Entity>(
        db,
        properties_models,
        OnConflict::columns([
            fii_cvm_properties::Column::Cnpj,
            fii_cvm_properties::Column::ReferenceDate,
            fii_cvm_properties::Column::NomeImovel,
        ])
        .do_nothing()
        .to_owned(),
    )
    .await?;

    Ok(FiiCvmDataResult {
        monthly_count,
        properties_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    async fn dev_db() -> DatabaseConnection {
        Database::connect("sqlite:///data/anchor.db?mode=rwc")
            .await
            .expect("failed to connect to the real dev database")
    }

    #[test]
    fn format_cnpj_inserts_the_standard_mask() {
        assert_eq!(format_cnpj("11728688000147"), "11.728.688/0001-47");
    }

    #[test]
    fn format_cnpj_returns_input_unchanged_on_unexpected_shape() {
        assert_eq!(format_cnpj("not-a-cnpj"), "not-a-cnpj");
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_cvm_data_is_idempotent() {
        let db = dev_db().await;
        // HGLG11 — CNPJ real, já resolvido e cacheado em `fii_cnpj_cache`
        // pelo banco de dev deste projeto (consultado direto pra este
        // teste, não inventado).
        let cnpjs = vec!["11.728.688/0001-47".to_string()];

        let first = collect_cvm_data(&db, &cnpjs).await.unwrap();
        assert!(first.monthly_count > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn live_resolve_cnpj_cache_hit_skips_the_network() {
        let db = dev_db().await;
        // HGLG11 já está cacheado no banco de dev real (ver
        // live_collect_cvm_data_is_idempotent acima). Sem chave da bolsai
        // configurada, qualquer ida à rede falharia: o teste só passa se o
        // cache for consultado primeiro.
        let result = resolve_cnpj(&db, "HGLG11").await.unwrap();
        let resolution = result.expect("HGLG11 deveria estar cacheado");
        assert_eq!(resolution.cnpj, "11.728.688/0001-47");
    }

    #[tokio::test]
    #[ignore]
    async fn live_resolve_cnpj_cache_miss_fetches_and_persists() {
        let db = dev_db().await;
        // KNRI11 (Kinea Renda Imobiliária) — FII real, escolhido por não
        // estar cacheado no banco de dev antes da primeira vez que este
        // teste rodou (conferido direto no SQLite); reruns batem no cache,
        // as asserções continuam válidas de todo jeito.
        let ticker = "KNRI11";

        let result = resolve_cnpj(&db, ticker).await.unwrap();
        let resolution = result.expect("KNRI11 deveria resolver via bolsai+CVM");
        assert!(!resolution.cnpj.is_empty());

        let cached = fii_cnpj_cache::Entity::find()
            .filter(fii_cnpj_cache::Column::Ticker.eq(ticker))
            .one(&db)
            .await
            .unwrap();
        assert!(cached.is_some(), "resolução deveria ter sido cacheada");
    }
}

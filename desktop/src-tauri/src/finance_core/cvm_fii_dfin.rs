// CVM, demonstrações financeiras de FII (`DFIN`, dados abertos): uma linha por documento entregue, com o CNPJ do fundo,
// a data de referência, a de entrega, a versão, o parecer do auditor e o link do PDF.
// `https://dados.cvm.gov.br/dados/FII/DOC/DFIN/DADOS/dfin_fii_{ano}.csv` (CSV direto por ano, não zip, em latin-1).
//
// Só aponta para o documento: o link vai para o `downloadDocumento` público do Fundos.NET e quem o abre é o usuário, no
// navegador. O app não baixa nem lê o PDF, e não consulta o Fundos.NET (a permissão de automatizar esse acesso não está
// confirmada, ver a Sessão 397). Não é RI no sentido da Sessão 389: são as demonstrações financeiras, 1 a 2 por fundo ao ano.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};

use crate::error::AppError;
use crate::finance_core::cvm_dfp::latin1;
use crate::finance_core::cvm_fii::normalize_cnpj;
use crate::finance_core::disk_cache;
use crate::finance_core::provider::BoxFuture;

const BASE_URL: &str = "https://dados.cvm.gov.br/dados/FII/DOC/DFIN/DADOS";

/// O CSV do ano corrente ganha linhas o ano todo; o do ano anterior só muda de leve.
const CURRENT_MAX_AGE: Duration = Duration::from_secs(12 * 3600);
const PREVIOUS_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Debug, Clone, PartialEq)]
pub struct FinancialStatement {
    /// Coluna `Data_Referencia`: a data-base da demonstração.
    pub reference_date: NaiveDate,
    /// Coluna `Data_Entrega`.
    pub delivered_on: NaiveDate,
    /// Coluna `Versao` (1 = original; 2 em diante = retificação); 1 se ilegível.
    pub version: u32,
    /// Coluna `Parecer_Auditor`, como a CVM escreve (ex.: "Sem ressalva e sem ênfase"); pode vir vazia.
    pub auditor_opinion: String,
    pub link: String,
}

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("CVM FII DFIN: {detail}"))
}

/// As demonstrações de `cnpj_digits` (só dígitos) dentro de um CSV anual.
fn read_statements(path: &Path, cnpj_digits: &str) -> Result<Vec<FinancialStatement>, AppError> {
    let mut reader =
        csv::ReaderBuilder::new().delimiter(b';').flexible(true).from_reader(BufReader::new(File::open(path)?));

    let headers = reader.byte_headers().map_err(unavailable)?.clone();
    let required = |name: &str| {
        headers
            .iter()
            .position(|h| h == name.as_bytes())
            .ok_or_else(|| unavailable(format!("missing column {name}")))
    };
    let (c_cnpj, c_reference, c_version, c_delivered, c_link, c_opinion) = (
        required("CNPJ_Fundo_Classe")?,
        required("Data_Referencia")?,
        required("Versao")?,
        required("Data_Entrega")?,
        required("Link_Download")?,
        required("Parecer_Auditor")?,
    );

    let parse_date = |name: &str, raw: &str| {
        NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").map_err(|e| unavailable(format!("invalid {name} '{raw}': {e}")))
    };

    let mut statements = Vec::new();
    for record in reader.byte_records() {
        let record = record.map_err(unavailable)?;
        let field = |i: usize| record.get(i).map(latin1).unwrap_or_default();
        if normalize_cnpj(&field(c_cnpj)) != cnpj_digits {
            continue;
        }
        statements.push(FinancialStatement {
            reference_date: parse_date("Data_Referencia", &field(c_reference))?,
            delivered_on: parse_date("Data_Entrega", &field(c_delivered))?,
            version: field(c_version).trim().parse().unwrap_or(1),
            auditor_opinion: field(c_opinion).trim().to_string(),
            link: field(c_link).trim().to_string(),
        });
    }
    Ok(statements)
}

/// Para a mesma `Data_Referencia`, só a maior `Versao` (empate: a entrega mais nova): a retificação substitui a anterior.
fn newest_versions(statements: Vec<FinancialStatement>) -> Vec<FinancialStatement> {
    let mut by_reference: HashMap<NaiveDate, FinancialStatement> = HashMap::new();
    for statement in statements {
        match by_reference.get(&statement.reference_date) {
            Some(kept) if (kept.version, kept.delivered_on) >= (statement.version, statement.delivered_on) => {}
            _ => {
                by_reference.insert(statement.reference_date, statement);
            }
        }
    }
    by_reference.into_values().collect()
}

// ---------------------------------------------------------------------------
// Download e orquestração
// ---------------------------------------------------------------------------

pub trait CsvFetcher: Send + Sync {
    /// Caminho local do CSV publicado no ano `year`; erro se não existe ou não baixou.
    fn csv<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>>;
}

pub struct CvmDfinCsvs;

impl CsvFetcher for CvmDfinCsvs {
    fn csv<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
        Box::pin(async move {
            let name = format!("dfin_fii_{year}.csv");
            let max_age = if year >= Utc::now().year() { CURRENT_MAX_AGE } else { PREVIOUS_MAX_AGE };
            disk_cache::get_or_download(
                "CVM FII DFIN",
                &format!("cvm_fii_dfin/{name}"),
                &format!("{BASE_URL}/{name}"),
                Some(max_age),
            )
            .await
        })
    }
}

async fn year_statements(
    fetcher: &dyn CsvFetcher,
    year: i32,
    cnpj_digits: String,
) -> Result<Vec<FinancialStatement>, AppError> {
    let path = fetcher.csv(year).await?;
    tokio::task::spawn_blocking(move || read_statements(&path, &cnpj_digits))
        .await
        .map_err(|e| unavailable(format!("parsing task failed: {e}")))?
}

/// Do ano `current_year` e do anterior, da entrega mais nova à mais antiga, sem repetir o mesmo documento (identidade =
/// o link). Fundo sem demonstração é uma lista vazia, não `NoData`. Um ano pode falhar (o outro costuma bastar), mas se
/// não sobrou nenhuma linha e algum ano falhou, o erro sobe, pra não confundir "a CVM caiu" com "sem demonstrações".
pub async fn statements_with(
    fetcher: &dyn CsvFetcher,
    cnpj: &str,
    current_year: i32,
) -> Result<Vec<FinancialStatement>, AppError> {
    let digits = normalize_cnpj(cnpj);
    let mut statements = Vec::new();
    let mut errors = Vec::new();
    for year in [current_year, current_year - 1] {
        match year_statements(fetcher, year, digits.clone()).await {
            Ok(found) => statements.extend(found),
            Err(err) => errors.push(err),
        }
    }
    if statements.is_empty() && !errors.is_empty() {
        return Err(errors.remove(0));
    }
    let mut seen = HashSet::new();
    statements.retain(|s| seen.insert(s.link.clone()));
    let mut statements = newest_versions(statements);
    statements.sort_by(|a, b| b.delivered_on.cmp(&a.delivered_on).then_with(|| b.reference_date.cmp(&a.reference_date)));
    Ok(statements)
}

pub async fn financial_statements(cnpj: &str) -> Result<Vec<FinancialStatement>, AppError> {
    statements_with(&CvmDfinCsvs, cnpj, Utc::now().year()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    const HEADER: &str =
        "Tipo_Fundo_Classe;CNPJ_Fundo_Classe;Nome_Fundo_Classe;Data_Referencia;Versao;Data_Entrega;Link_Download;Parecer_Auditor";

    const VIA_PARQUE: &str = "00.332.266/0001-31";
    const PANAMBY: &str = "00.613.094/0001-74";
    const LINK_A: &str = "https://fnet.bmfbovespa.com.br/fnet/publico/downloadDocumento?id=1";
    const LINK_B: &str = "https://fnet.bmfbovespa.com.br/fnet/publico/downloadDocumento?id=2";
    const LINK_C: &str = "https://fnet.bmfbovespa.com.br/fnet/publico/downloadDocumento?id=3";

    fn row(cnpj: &str, reference: &str, version: &str, delivered: &str, link: &str, opinion: &str) -> String {
        format!("CLASSES - FII;{cnpj};FUNDO TESTE;{reference};{version};{delivered};{link};{opinion}")
    }

    /// Texto de teste só tem caracteres < U+0100, então codifica 1:1 em latin-1.
    fn csv_bytes(header: &str, lines: &[String]) -> Vec<u8> {
        let mut all = vec![header.to_string()];
        all.extend_from_slice(lines);
        all.join("\n").chars().map(|c| c as u8).collect()
    }

    /// Fetcher falso: um CSV por ano; ano ausente = falha de download.
    struct Fake {
        csvs: HashMap<i32, PathBuf>,
    }

    impl Fake {
        fn with_header(header: &str, years: Vec<(i32, Vec<String>)>) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "anchor-dfin-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let csvs = years
                .into_iter()
                .map(|(year, lines)| {
                    let path = dir.join(format!("dfin_fii_{year}.csv"));
                    std::fs::write(&path, csv_bytes(header, &lines)).unwrap();
                    (year, path)
                })
                .collect();
            Fake { csvs }
        }

        fn new(years: Vec<(i32, Vec<String>)>) -> Self {
            Self::with_header(HEADER, years)
        }
    }

    impl CsvFetcher for Fake {
        fn csv<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
            Box::pin(async move {
                self.csvs.get(&year).cloned().ok_or_else(|| unavailable(format!("download failed for {year}")))
            })
        }
    }

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[tokio::test]
    async fn lists_the_fund_statements_newest_delivery_first() {
        let fake = Fake::new(vec![
            (2025, vec![row(VIA_PARQUE, "2025-07-31", "1", "2025-10-30", LINK_A, "Sem ressalva e sem ênfase")]),
            (2026, vec![row(VIA_PARQUE, "2025-12-31", "1", "2026-04-01", LINK_B, "Sem ressalva e com ênfase")]),
        ]);

        let found = statements_with(&fake, VIA_PARQUE, 2026).await.unwrap();

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].link, LINK_B);
        assert_eq!(found[0].reference_date, date("2025-12-31"));
        assert_eq!(found[0].delivered_on, date("2026-04-01"));
        assert_eq!(found[0].version, 1);
        assert_eq!(found[0].auditor_opinion, "Sem ressalva e com ênfase");
        assert_eq!(found[1].link, LINK_A);
    }

    #[tokio::test]
    async fn keeps_only_the_requested_fund_comparing_digits() {
        let fake = Fake::new(vec![(
            2026,
            vec![
                row(VIA_PARQUE, "2025-12-31", "1", "2026-04-01", LINK_A, ""),
                row(PANAMBY, "2025-12-31", "1", "2026-03-30", LINK_B, ""),
            ],
        )]);

        // o CNPJ pedido vem sem pontuação e o do arquivo com
        let found = statements_with(&fake, "00332266000131", 2026).await.unwrap();

        assert_eq!(found.iter().map(|s| s.link.as_str()).collect::<Vec<_>>(), vec![LINK_A]);
    }

    #[tokio::test]
    async fn a_reissued_statement_keeps_only_the_newest_version() {
        let fake = Fake::new(vec![(
            2026,
            vec![
                row(VIA_PARQUE, "2025-12-31", "1", "2026-03-20", LINK_A, ""),
                row(VIA_PARQUE, "2025-12-31", "2", "2026-04-10", LINK_B, ""),
                // outra data de referência: é outra demonstração, fica
                row(VIA_PARQUE, "2025-07-31", "1", "2025-10-30", LINK_C, ""),
            ],
        )]);

        let found = statements_with(&fake, VIA_PARQUE, 2026).await.unwrap();

        assert_eq!(found.iter().map(|s| s.link.as_str()).collect::<Vec<_>>(), vec![LINK_B, LINK_C]);
        assert_eq!(found[0].version, 2);
    }

    #[tokio::test]
    async fn the_same_document_in_both_years_counts_once() {
        let line = row(VIA_PARQUE, "2025-07-31", "1", "2025-12-30", LINK_A, "");
        let fake = Fake::new(vec![(2025, vec![line.clone()]), (2026, vec![line])]);

        let found = statements_with(&fake, VIA_PARQUE, 2026).await.unwrap();

        assert_eq!(found.len(), 1);
    }

    #[tokio::test]
    async fn a_fund_without_statements_is_an_empty_list() {
        let fake = Fake::new(vec![
            (2025, vec![row(PANAMBY, "2025-07-31", "1", "2025-10-30", LINK_A, "")]),
            (2026, vec![]),
        ]);

        assert!(statements_with(&fake, VIA_PARQUE, 2026).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn one_missing_year_is_tolerated() {
        let fake = Fake::new(vec![(2025, vec![row(VIA_PARQUE, "2025-07-31", "1", "2025-10-30", LINK_A, "")])]);

        let found = statements_with(&fake, VIA_PARQUE, 2026).await.unwrap();

        assert_eq!(found.len(), 1);
    }

    #[tokio::test]
    async fn both_years_failing_is_provider_unavailable() {
        let fake = Fake::new(vec![]);

        let err = statements_with(&fake, VIA_PARQUE, 2026).await.unwrap_err();

        assert!(matches!(err, AppError::ProviderUnavailable(_)), "{err:?}");
    }

    #[tokio::test]
    async fn an_empty_year_next_to_a_failed_one_is_still_an_error() {
        // 2026 sem nenhuma linha e 2025 que não baixou: pode ser "a CVM caiu", então não vira lista vazia
        let fake = Fake::new(vec![(2026, vec![])]);

        let err = statements_with(&fake, VIA_PARQUE, 2026).await.unwrap_err();

        assert!(matches!(err, AppError::ProviderUnavailable(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_missing_column_is_provider_unavailable_not_an_empty_list() {
        let header = "CNPJ_Fundo_Classe;Data_Referencia;Versao;Data_Entrega;Parecer_Auditor";
        let fake = Fake::with_header(header, vec![(2026, vec![format!("{VIA_PARQUE};2025-12-31;1;2026-04-01;")])]);

        let err = statements_with(&fake, VIA_PARQUE, 2026).await.unwrap_err();

        assert!(matches!(&err, AppError::ProviderUnavailable(msg) if msg.contains("Link_Download")), "{err:?}");
    }

    #[tokio::test]
    async fn an_invalid_delivery_date_is_provider_unavailable() {
        let fake = Fake::new(vec![(2026, vec![row(VIA_PARQUE, "2025-12-31", "1", "31/03/2026", LINK_A, "")])]);

        let err = statements_with(&fake, VIA_PARQUE, 2026).await.unwrap_err();

        assert!(matches!(&err, AppError::ProviderUnavailable(msg) if msg.contains("Data_Entrega")), "{err:?}");
    }

    #[tokio::test]
    async fn an_unreadable_version_counts_as_the_first() {
        let fake = Fake::new(vec![(2026, vec![row(VIA_PARQUE, "2025-12-31", "", "2026-04-01", LINK_A, "")])]);

        let found = statements_with(&fake, VIA_PARQUE, 2026).await.unwrap();

        assert_eq!(found[0].version, 1);
    }
}

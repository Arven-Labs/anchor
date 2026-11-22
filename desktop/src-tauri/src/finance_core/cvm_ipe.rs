// CVM, dados abertos de empresas (IPE, Informações Periódicas e Eventuais) —
// porta de `lume/api/app/sources/cvm_ipe.py`. Mesmo desenho do `cvm_dfp`: um zip
// por ano com um CSV `;` em latin-1 de todas as companhias abertas:
// `https://dados.cvm.gov.br/dados/CIA_ABERTA/DOC/IPE/DADOS/ipe_cia_aberta_{ano}.zip`
//
// Diferenças em relação ao DFP:
// - o zip leva o ano de **publicação**, não o fiscal, e cresce o tempo todo
//   (empresas entregam documentos novos); por isso o cache em disco expira em
//   horas no ano corrente, em vez dos 30 dias do DFP;
// - o aviso de dividendos de janeiro pode estar no zip de qualquer um dos dois
//   anos, então lê o ano atual e o anterior.
//
// Esquema conferido ao vivo no arquivo de 2026 (~13 MB, 33 mil linhas):
// `CNPJ_Companhia;Nome_Companhia;Codigo_CVM;Data_Referencia;Categoria;Tipo;
// Especie;Assunto;Data_Entrega;Tipo_Apresentacao;Protocolo_Entrega;Versao;
// Link_Download`. A categoria **"Relatório Proventos"** (481 linhas) é um formulário
// padronizado da CVM, e filtrar só por ela já é preciso (as categorias livres,
// como "Fato Relevante", falam de dividendos só de vez em quando). O CSV não traz
// valor, data de pagamento nem data-com: isso fica no PDF de `Link_Download`.
//
// `Protocolo_Entrega` vem em branco em toda linha dessa categoria (conferido ao
// vivo), então a identidade do aviso é o `Link_Download`, que carrega um
// `numSequencia` único por entrega. `Codigo_CVM` vem com zeros à esquerda
// (`001023`): compara como inteiro.

use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};

use crate::error::AppError;
use crate::finance_core::cvm_dfp::latin1;
use crate::finance_core::disk_cache;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "cvm_ipe";
const ZIP_URL: &str = "https://dados.cvm.gov.br/dados/CIA_ABERTA/DOC/IPE/DADOS";
const DIVIDEND_CATEGORY: &str = "Relatório Proventos";

/// O zip do ano corrente ganha linhas o dia todo; o do ano anterior só muda de leve.
const CURRENT_ZIP_MAX_AGE: Duration = Duration::from_secs(12 * 3600);
const PREVIOUS_ZIP_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Debug, Clone, PartialEq)]
pub struct DividendNotice {
    /// O protocolo da entrega ou, quando vem em branco (o normal nessa categoria), o link.
    pub protocolo_entrega: String,
    pub data_entrega: NaiveDate,
    pub link_download: String,
}

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("CVM IPE: {detail}"))
}

/// Avisos de `cvm_code` dentro de um zip (primeiro arquivo do zip).
fn read_notices(zip_path: &Path, cvm_code: i32) -> Result<Vec<DividendNotice>, AppError> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(unavailable)?;
    let entry = archive.by_index(0).map_err(unavailable)?;
    let mut reader = csv::ReaderBuilder::new().delimiter(b';').flexible(true).from_reader(entry);

    let headers = reader.byte_headers().map_err(unavailable)?.clone();
    let required = |name: &str| {
        headers
            .iter()
            .position(|h| h == name.as_bytes())
            .ok_or_else(|| unavailable(format!("missing column {name}")))
    };
    let (c_cvm, c_category, c_delivered, c_protocol, c_link) = (
        required("Codigo_CVM")?,
        required("Categoria")?,
        required("Data_Entrega")?,
        required("Protocolo_Entrega")?,
        required("Link_Download")?,
    );

    let mut notices = Vec::new();
    for record in reader.byte_records() {
        let record = record.map_err(unavailable)?;
        let field = |i: usize| record.get(i).map(latin1).unwrap_or_default();
        if field(c_category) != DIVIDEND_CATEGORY || field(c_cvm).trim().parse::<i32>().ok() != Some(cvm_code) {
            continue;
        }
        let delivered = field(c_delivered);
        let data_entrega = NaiveDate::parse_from_str(delivered.trim(), "%Y-%m-%d")
            .map_err(|e| unavailable(format!("invalid Data_Entrega '{delivered}': {e}")))?;
        let link_download = field(c_link);
        let protocolo = field(c_protocol);
        notices.push(DividendNotice {
            protocolo_entrega: if protocolo.is_empty() { link_download.clone() } else { protocolo },
            data_entrega,
            link_download,
        });
    }
    Ok(notices)
}

// ---------------------------------------------------------------------------
// Download e orquestração
// ---------------------------------------------------------------------------

pub trait ZipFetcher: Send + Sync {
    /// Caminho local do zip publicado no ano `year`; erro se não existe/não baixou.
    fn zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>>;
}

pub struct CvmIpeZips;

impl ZipFetcher for CvmIpeZips {
    fn zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
        Box::pin(async move {
            let name = format!("ipe_cia_aberta_{year}.zip");
            let max_age = if year >= Utc::now().year() { CURRENT_ZIP_MAX_AGE } else { PREVIOUS_ZIP_MAX_AGE };
            disk_cache::get_or_download("CVM IPE", &format!("cvm_ipe/{name}"), &format!("{ZIP_URL}/{name}"), Some(max_age)).await
        })
    }
}

async fn year_notices(fetcher: &dyn ZipFetcher, year: i32, cvm_code: i32) -> Result<Vec<DividendNotice>, AppError> {
    let path = fetcher.zip(year).await?;
    tokio::task::spawn_blocking(move || read_notices(&path, cvm_code))
        .await
        .map_err(|e| unavailable(format!("parsing task failed: {e}")))?
}

/// Avisos do ano `current_year` e do anterior, do mais antigo ao mais novo.
/// Empresa sem aviso é uma lista vazia, não `NoData`. Um ano pode falhar (o
/// anterior costuma bastar), mas se não sobrou nenhuma linha e algum ano falhou,
/// o erro sobe, pra não confundir "a CVM caiu" com "sem avisos".
pub async fn notices_with(
    fetcher: &dyn ZipFetcher,
    cvm_code: i32,
    current_year: i32,
) -> Result<Vec<DividendNotice>, AppError> {
    let mut notices = Vec::new();
    let mut errors = Vec::new();
    for year in [current_year, current_year - 1] {
        match year_notices(fetcher, year, cvm_code).await {
            Ok(found) => notices.extend(found),
            Err(err) => errors.push(err),
        }
    }
    if notices.is_empty() && !errors.is_empty() {
        return Err(errors.remove(0));
    }
    let mut seen = HashSet::new();
    notices.retain(|n| seen.insert(n.protocolo_entrega.clone()));
    notices.sort_by(|a, b| a.data_entrega.cmp(&b.data_entrega).then_with(|| a.protocolo_entrega.cmp(&b.protocolo_entrega)));
    Ok(notices)
}

// ---------------------------------------------------------------------------
// Releases e apresentações de resultado (RI)
// ---------------------------------------------------------------------------

/// Só links: o conteúdo do PDF não é lido (parsing de dado externo, e a IA saiu do Anchor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FilingKind {
    /// Categoria "Dados Econômico-Financeiros", tipo "Press-release".
    Release,
    /// Categoria "Comunicado ao Mercado", tipo "Apresentações a analistas/agentes do mercado".
    Presentation,
    /// Categoria "Fato Relevante" (o `Tipo` vem sempre vazio no IPE).
    MaterialFact,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanyFiling {
    /// Coluna `Data_Referencia`; com o `subject`, identifica o mesmo fato entre reapresentações.
    pub reference_date: String,
    /// Coluna `Versao` (1 = original; 2 em diante = reapresentação); 1 se ilegível.
    pub version: u32,
    pub protocolo_entrega: String,
    pub data_entrega: NaiveDate,
    pub link_download: String,
    pub kind: FilingKind,
    /// Coluna `Assunto` (ex.: "Apresentação de Resultados 3T26"); pode vir vazia.
    pub subject: String,
}

/// Quais pares (categoria, tipo) do IPE são release ou apresentação de resultado.
/// Qualquer outro (Fato Relevante, Comunicado em geral, Proventos...) devolve `None`.
pub fn classify_filing(category: &str, kind: &str) -> Option<FilingKind> {
    match (category, kind) {
        ("Dados Econômico-Financeiros", "Press-release") => Some(FilingKind::Release),
        ("Comunicado ao Mercado", "Apresentações a analistas/agentes do mercado") => Some(FilingKind::Presentation),
        _ => None,
    }
}

/// Releases e apresentações de `cvm_code` dentro de um zip (primeiro arquivo do zip).
fn read_filings(zip_path: &Path, cvm_code: i32) -> Result<Vec<CompanyFiling>, AppError> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(unavailable)?;
    let entry = archive.by_index(0).map_err(unavailable)?;
    let mut reader = csv::ReaderBuilder::new().delimiter(b';').flexible(true).from_reader(entry);

    let headers = reader.byte_headers().map_err(unavailable)?.clone();
    let required = |name: &str| {
        headers
            .iter()
            .position(|h| h == name.as_bytes())
            .ok_or_else(|| unavailable(format!("missing column {name}")))
    };
    let (c_cvm, c_category, c_type, c_subject, c_delivered, c_protocol, c_link, c_reference, c_version) = (
        required("Codigo_CVM")?,
        required("Categoria")?,
        required("Tipo")?,
        required("Assunto")?,
        required("Data_Entrega")?,
        required("Protocolo_Entrega")?,
        required("Link_Download")?,
        required("Data_Referencia")?,
        required("Versao")?,
    );

    let mut filings = Vec::new();
    for record in reader.byte_records() {
        let record = record.map_err(unavailable)?;
        let field = |i: usize| record.get(i).map(latin1).unwrap_or_default();
        let Some(kind) = classify_filing(&field(c_category), &field(c_type)) else {
            continue;
        };
        if field(c_cvm).trim().parse::<i32>().ok() != Some(cvm_code) {
            continue;
        }
        let delivered = field(c_delivered);
        let data_entrega = NaiveDate::parse_from_str(delivered.trim(), "%Y-%m-%d")
            .map_err(|e| unavailable(format!("invalid Data_Entrega '{delivered}': {e}")))?;
        let link_download = field(c_link);
        let protocolo = field(c_protocol);
        filings.push(CompanyFiling {
            reference_date: field(c_reference),
            version: field(c_version).trim().parse().unwrap_or(1),
            protocolo_entrega: if protocolo.is_empty() { link_download.clone() } else { protocolo },
            data_entrega,
            link_download,
            kind,
            subject: field(c_subject),
        });
    }
    Ok(filings)
}

async fn year_filings(fetcher: &dyn ZipFetcher, year: i32, cvm_code: i32) -> Result<Vec<CompanyFiling>, AppError> {
    let path = fetcher.zip(year).await?;
    tokio::task::spawn_blocking(move || read_filings(&path, cvm_code))
        .await
        .map_err(|e| unavailable(format!("parsing task failed: {e}")))?
}

/// Do ano `current_year` e do anterior, do mais antigo ao mais novo, sem repetir a mesma
/// entrega (identidade = protocolo ou, em branco, o link). Mesma regra de erro de `notices_with`.
pub async fn filings_with(
    fetcher: &dyn ZipFetcher,
    cvm_code: i32,
    current_year: i32,
) -> Result<Vec<CompanyFiling>, AppError> {
    let mut filings = Vec::new();
    let mut errors = Vec::new();
    for year in [current_year, current_year - 1] {
        match year_filings(fetcher, year, cvm_code).await {
            Ok(found) => filings.extend(found),
            Err(err) => errors.push(err),
        }
    }
    if filings.is_empty() && !errors.is_empty() {
        return Err(errors.remove(0));
    }
    let mut seen = HashSet::new();
    filings.retain(|f| seen.insert(f.protocolo_entrega.clone()));
    filings.sort_by(|a, b| {
        a.data_entrega.cmp(&b.data_entrega).then_with(|| a.protocolo_entrega.cmp(&b.protocolo_entrega))
    });
    Ok(filings)
}

pub async fn company_filings(cvm_code: i32) -> Result<Vec<CompanyFiling>, AppError> {
    filings_with(&CvmIpeZips, cvm_code, Utc::now().year()).await
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub trait DividendNoticeProvider: DataProvider {
    fn dividend_notices<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<Vec<DividendNotice>, AppError>>;
}

pub struct CvmIpe;

impl DataProvider for CvmIpe {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl DividendNoticeProvider for CvmIpe {
    fn dividend_notices<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<Vec<DividendNotice>, AppError>> {
        Box::pin(async move { notices_with(&CvmIpeZips, cvm_code, Utc::now().year()).await })
    }
}

fn providers() -> Vec<Box<dyn DividendNoticeProvider>> {
    vec![Box::new(CvmIpe)]
}

pub async fn dividend_notices(cvm_code: i32) -> Result<Sourced<Vec<DividendNotice>>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.dividend_notices(cvm_code))).collect();
    cascade(&format!("dividend notices for CVM {cvm_code}"), attempts).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    const HEADER: &str = "CNPJ_Companhia;Nome_Companhia;Codigo_CVM;Data_Referencia;Categoria;Tipo;Especie;Assunto;Data_Entrega;Tipo_Apresentacao;Protocolo_Entrega;Versao;Link_Download";

    fn row(cvm: &str, category: &str, delivered: &str, protocol: &str, link: &str) -> String {
        format!("00.000.000/0001-91;BANCO DO BRASIL S.A.;{cvm};2026-02-10;{category};;;JCP;{delivered};AP - Apresentação;{protocol};1;{link}")
    }

    fn dividend(cvm: &str, delivered: &str, link: &str) -> String {
        row(cvm, DIVIDEND_CATEGORY, delivered, "", link)
    }

    /// Texto de teste só tem caracteres < U+0100, então codifica 1:1 em latin-1.
    fn csv_bytes(lines: &[String]) -> Vec<u8> {
        let mut all = vec![HEADER.to_string()];
        all.extend_from_slice(lines);
        all.join("\n").chars().map(|c| c as u8).collect()
    }

    fn zip_bytes(name: &str, data: &[u8]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buf);
        writer
            .start_file(name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
            .unwrap();
        writer.write_all(data).unwrap();
        writer.finish().unwrap();
        buf.into_inner()
    }

    /// Fetcher falso: um zip por ano; ano ausente = falha de download.
    struct Fake {
        zips: HashMap<i32, PathBuf>,
    }

    impl Fake {
        fn new(zips: Vec<(i32, Vec<u8>)>) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "anchor-ipe-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let zips = zips
                .into_iter()
                .map(|(year, csv)| {
                    let path = dir.join(format!("ipe_cia_aberta_{year}.zip"));
                    std::fs::write(&path, zip_bytes(&format!("ipe_cia_aberta_{year}.csv"), &csv)).unwrap();
                    (year, path)
                })
                .collect();
            Fake { zips }
        }
    }

    impl ZipFetcher for Fake {
        fn zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
            Box::pin(async move {
                self.zips.get(&year).cloned().ok_or_else(|| unavailable(format!("download failed for {year}")))
            })
        }
    }

    const BB: i32 = 1023;
    const LINK_A: &str = "https://www.rad.cvm.gov.br/ENET/frmDownloadDocumento.aspx?numSequencia=1&numProtocolo=1";
    const LINK_B: &str = "https://www.rad.cvm.gov.br/ENET/frmDownloadDocumento.aspx?numSequencia=2&numProtocolo=2";
    const LINK_C: &str = "https://www.rad.cvm.gov.br/ENET/frmDownloadDocumento.aspx?numSequencia=3&numProtocolo=3";

    #[tokio::test]
    async fn keeps_only_dividend_notices_of_the_company() {
        let fake = Fake::new(vec![(
            2026,
            csv_bytes(&[
                dividend("001023", "2026-02-11", LINK_A),
                dividend("009512", "2026-02-12", LINK_B),
                row("001023", "Fato Relevante", "2026-02-13", "999", LINK_C),
            ]),
        )]);
        let notices = notices_with(&fake, BB, 2026).await.unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].link_download, LINK_A);
        assert_eq!(notices[0].data_entrega, NaiveDate::from_ymd_opt(2026, 2, 11).unwrap());
    }

    #[tokio::test]
    async fn blank_protocol_falls_back_to_the_link() {
        let fake = Fake::new(vec![(2026, csv_bytes(&[dividend("1023", "2026-02-11", LINK_A)]))]);
        assert_eq!(notices_with(&fake, BB, 2026).await.unwrap()[0].protocolo_entrega, LINK_A);
    }

    #[tokio::test]
    async fn filled_protocol_is_kept() {
        let line = row("1023", DIVIDEND_CATEGORY, "2026-02-11", "111", LINK_A);
        let fake = Fake::new(vec![(2026, csv_bytes(&[line]))]);
        assert_eq!(notices_with(&fake, BB, 2026).await.unwrap()[0].protocolo_entrega, "111");
    }

    #[tokio::test]
    async fn merges_both_years_oldest_first() {
        let fake = Fake::new(vec![
            (2026, csv_bytes(&[dividend("1023", "2026-02-11", LINK_B)])),
            (2025, csv_bytes(&[dividend("1023", "2025-12-20", LINK_A)])),
        ]);
        let notices = notices_with(&fake, BB, 2026).await.unwrap();
        let links: Vec<_> = notices.iter().map(|n| n.link_download.as_str()).collect();
        assert_eq!(links, [LINK_A, LINK_B]);
    }

    #[tokio::test]
    async fn the_same_filing_in_both_years_counts_once() {
        let line = dividend("1023", "2026-01-05", LINK_A);
        let fake = Fake::new(vec![(2026, csv_bytes(&[line.clone()])), (2025, csv_bytes(&[line]))]);
        assert_eq!(notices_with(&fake, BB, 2026).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn one_missing_year_is_tolerated() {
        let fake = Fake::new(vec![(2025, csv_bytes(&[dividend("1023", "2025-12-20", LINK_A)]))]);
        assert_eq!(notices_with(&fake, BB, 2026).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn both_years_failing_is_provider_unavailable() {
        let fake = Fake::new(vec![]);
        assert!(matches!(notices_with(&fake, BB, 2026).await, Err(AppError::ProviderUnavailable(_))));
    }

    #[tokio::test]
    async fn company_without_notices_is_an_empty_list() {
        let fake = Fake::new(vec![
            (2026, csv_bytes(&[dividend("9512", "2026-02-11", LINK_A)])),
            (2025, csv_bytes(&[])),
        ]);
        assert!(notices_with(&fake, BB, 2026).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_empty_year_next_to_a_failed_one_is_still_an_error() {
        // Sem nenhuma linha e com um ano fora do ar não dá pra afirmar "sem avisos".
        let fake = Fake::new(vec![(2026, csv_bytes(&[]))]);
        assert!(matches!(notices_with(&fake, BB, 2026).await, Err(AppError::ProviderUnavailable(_))));
    }

    #[tokio::test]
    async fn a_missing_column_is_provider_unavailable() {
        let csv = "Codigo_CVM;Categoria\n1023;Relatório Proventos".chars().map(|c| c as u8).collect();
        let fake = Fake::new(vec![(2026, csv), (2025, Vec::new())]);
        let err = notices_with(&fake, BB, 2026).await.unwrap_err();
        assert!(matches!(&err, AppError::ProviderUnavailable(m) if m.contains("missing column")), "{err}");
    }

    #[tokio::test]
    async fn an_invalid_delivery_date_is_provider_unavailable() {
        let fake = Fake::new(vec![(2026, csv_bytes(&[dividend("1023", "11/02/2026", LINK_A)])), (2025, csv_bytes(&[]))]);
        assert!(matches!(notices_with(&fake, BB, 2026).await, Err(AppError::ProviderUnavailable(_))));
    }

    // --- releases e apresentações (RI) ---

    const CAT_FIN: &str = "Dados Econômico-Financeiros";
    const CAT_COMM: &str = "Comunicado ao Mercado";
    const TIPO_RELEASE: &str = "Press-release";
    const TIPO_PRES: &str = "Apresentações a analistas/agentes do mercado";

    fn filing_row(cvm: &str, category: &str, tipo: &str, subject: &str, delivered: &str, link: &str) -> String {
        format!("00.000.000/0001-91;BANCO DO BRASIL S.A.;{cvm};2026-02-10;{category};{tipo};;{subject};{delivered};AP - Apresentação;;1;{link}")
    }

    fn release(cvm: &str, delivered: &str, link: &str) -> String {
        filing_row(cvm, CAT_FIN, TIPO_RELEASE, "Release 3T26", delivered, link)
    }

    #[test]
    fn classifies_press_release_and_analyst_presentation() {
        assert_eq!(classify_filing(CAT_FIN, TIPO_RELEASE), Some(FilingKind::Release));
        assert_eq!(classify_filing(CAT_COMM, TIPO_PRES), Some(FilingKind::Presentation));
    }

    #[test]
    fn classification_needs_both_category_and_type() {
        assert_eq!(classify_filing(CAT_COMM, TIPO_RELEASE), None);
        assert_eq!(classify_filing(CAT_FIN, TIPO_PRES), None);
        assert_eq!(classify_filing(CAT_FIN, "Demonstrações Financeiras Anuais Completas"), None);
        assert_eq!(classify_filing(DIVIDEND_CATEGORY, ""), None);
    }

    #[test]
    fn classifies_a_material_fact_by_its_category_whatever_the_type() {
        // No IPE medido (2025 e 2026, 4.984 linhas) o `Tipo` é sempre vazio; a categoria basta.
        assert_eq!(classify_filing("Fato Relevante", ""), Some(FilingKind::MaterialFact));
        assert_eq!(classify_filing("Fato Relevante", "qualquer"), Some(FilingKind::MaterialFact));
    }

    #[test]
    fn similar_categories_are_not_material_facts() {
        assert_eq!(classify_filing("Política de Divulgação de Ato ou Fato Relevante", ""), None);
        assert_eq!(classify_filing("Comunicado ao Mercado", ""), None);
        assert_eq!(classify_filing("fato relevante", ""), None);
    }

    /// Fato relevante com `Data_Referencia`, `Versao` e protocolo próprios (a reapresentação
    /// é outra entrega, com protocolo e link novos).
    fn fact_row(cvm: &str, subject: &str, reference: &str, version: &str, delivered: &str, id: &str) -> String {
        format!(
            "00.000.000/0001-91;BANCO DO BRASIL S.A.;{cvm};{reference};Fato Relevante;;;{subject};{delivered};AP - Apresentação;P-{id};{version};https://x/{id}"
        )
    }

    #[tokio::test]
    async fn filings_include_the_material_facts_of_the_company_only() {
        let fake = Fake::new(vec![(
            2026,
            csv_bytes(&[
                fact_row("001023", "Projeções 2026", "2026-02-20", "1", "2026-02-20", "a"),
                fact_row("009512", "Outra empresa", "2026-02-21", "1", "2026-02-21", "b"),
            ]),
        )]);
        let filings = filings_with(&fake, BB, 2026).await.unwrap();
        assert_eq!(filings.len(), 1);
        assert_eq!(filings[0].kind, FilingKind::MaterialFact);
        assert_eq!(filings[0].subject, "Projeções 2026");
        assert_eq!(filings[0].link_download, "https://x/a");
    }

    #[tokio::test]
    async fn a_reissued_material_fact_keeps_only_the_newest_version() {
        let fake = Fake::new(vec![(
            2026,
            csv_bytes(&[
                fact_row("1023", "Distribuição de Dividendos", "2026-12-18", "1", "2026-12-18", "v1"),
                fact_row("1023", "Distribuição de Dividendos", "2026-12-18", "3", "2026-12-20", "v3"),
                fact_row("1023", "Distribuição de Dividendos", "2026-12-18", "2", "2026-12-19", "v2"),
                fact_row("1023", "Outro assunto", "2026-12-18", "1", "2026-12-18", "other"),
            ]),
        )]);
        let links: Vec<String> =
            filings_with(&fake, BB, 2026).await.unwrap().into_iter().map(|f| f.link_download).collect();
        assert_eq!(links, ["https://x/other", "https://x/v3"]);
    }

    #[tokio::test]
    async fn the_same_subject_on_another_reference_date_is_another_fact() {
        let fake = Fake::new(vec![(
            2026,
            csv_bytes(&[
                fact_row("1023", "Remuneração aos Acionistas", "2026-02-19", "1", "2026-02-19", "feb"),
                fact_row("1023", "Remuneração aos Acionistas", "2026-05-15", "1", "2026-05-15", "may"),
            ]),
        )]);
        assert_eq!(filings_with(&fake, BB, 2026).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn an_unreadable_version_counts_as_the_original() {
        let fake = Fake::new(vec![(
            2026,
            csv_bytes(&[
                fact_row("1023", "Assunto", "2026-03-01", "", "2026-03-01", "blank"),
                fact_row("1023", "Assunto", "2026-03-01", "2", "2026-03-02", "v2"),
            ]),
        )]);
        let links: Vec<String> =
            filings_with(&fake, BB, 2026).await.unwrap().into_iter().map(|f| f.link_download).collect();
        assert_eq!(links, ["https://x/v2"]);
    }

    #[tokio::test]
    async fn versions_of_a_release_are_left_alone() {
        // A reapresentação só é recolhida para fato relevante, onde foi medida.
        let versioned = |version: &str, link: &str| {
            format!(
                "00.000.000/0001-91;BB;1023;2026-02-10;{CAT_FIN};{TIPO_RELEASE};;Release 3T26;2026-11-05;AP - Apresentação;;{version};{link}"
            )
        };
        let fake = Fake::new(vec![(2026, csv_bytes(&[versioned("1", LINK_A), versioned("2", LINK_B)]))]);
        assert_eq!(filings_with(&fake, BB, 2026).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn filings_keep_only_the_ri_categories_of_the_company() {
        let fake = Fake::new(vec![(
            2026,
            csv_bytes(&[
                release("001023", "2026-11-05", LINK_A),
                release("009512", "2026-11-06", LINK_B),
                filing_row("001023", CAT_COMM, TIPO_PRES, "Apresentação de Resultados 3T26", "2026-11-07", LINK_C),
                filing_row("001023", "Fato Relevante", "", "Qualquer", "2026-11-08", "https://x/fr"),
                filing_row("001023", CAT_FIN, "Demonstrações Financeiras Anuais Completas", "", "2026-11-09", "https://x/dfp"),
                dividend("001023", "2026-11-10", "https://x/div"),
            ]),
        )]);
        let filings = filings_with(&fake, BB, 2026).await.unwrap();
        let links: Vec<_> = filings.iter().map(|f| f.link_download.as_str()).collect();
        // Release, apresentação e fato relevante; DFP e proventos seguem fora.
        assert_eq!(links, [LINK_A, LINK_C, "https://x/fr"]);
    }

    #[tokio::test]
    async fn filing_carries_kind_subject_and_delivery_date() {
        let fake = Fake::new(vec![(
            2026,
            csv_bytes(&[filing_row("1023", CAT_COMM, TIPO_PRES, "Apresentação de Resultados 3T26", "2026-11-07", LINK_C)]),
        )]);
        let f = &filings_with(&fake, BB, 2026).await.unwrap()[0];
        assert_eq!(f.kind, FilingKind::Presentation);
        assert_eq!(f.subject, "Apresentação de Resultados 3T26");
        assert_eq!(f.data_entrega, NaiveDate::from_ymd_opt(2026, 11, 7).unwrap());
        assert_eq!(f.protocolo_entrega, LINK_C);
    }

    #[tokio::test]
    async fn filings_merge_both_years_oldest_first() {
        let fake = Fake::new(vec![
            (2026, csv_bytes(&[release("1023", "2026-05-05", LINK_B)])),
            (2025, csv_bytes(&[release("1023", "2025-11-05", LINK_A)])),
        ]);
        let filings = filings_with(&fake, BB, 2026).await.unwrap();
        let links: Vec<_> = filings.iter().map(|f| f.link_download.as_str()).collect();
        assert_eq!(links, [LINK_A, LINK_B]);
    }

    #[tokio::test]
    async fn filings_repeated_in_both_years_count_once() {
        let line = release("1023", "2026-01-05", LINK_A);
        let fake = Fake::new(vec![(2026, csv_bytes(&[line.clone()])), (2025, csv_bytes(&[line]))]);
        assert_eq!(filings_with(&fake, BB, 2026).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn company_without_filings_is_an_empty_list() {
        let fake = Fake::new(vec![
            (2026, csv_bytes(&[release("9512", "2026-02-11", LINK_A)])),
            (2025, csv_bytes(&[])),
        ]);
        assert!(filings_with(&fake, BB, 2026).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn filings_with_both_years_failing_is_provider_unavailable() {
        let fake = Fake::new(vec![]);
        assert!(matches!(filings_with(&fake, BB, 2026).await, Err(AppError::ProviderUnavailable(_))));
    }

    #[tokio::test]
    async fn filings_need_the_type_and_subject_columns() {
        let csv = "Codigo_CVM;Categoria;Data_Entrega;Protocolo_Entrega;Link_Download\n1023;Comunicado ao Mercado;2026-11-07;;x"
            .chars()
            .map(|c| c as u8)
            .collect();
        let fake = Fake::new(vec![(2026, csv), (2025, Vec::new())]);
        let err = filings_with(&fake, BB, 2026).await.unwrap_err();
        assert!(matches!(&err, AppError::ProviderUnavailable(m) if m.contains("missing column")), "{err}");
    }

    #[tokio::test]
    async fn a_filing_with_an_invalid_delivery_date_is_provider_unavailable() {
        let fake = Fake::new(vec![(2026, csv_bytes(&[release("1023", "07/11/2026", LINK_A)])), (2025, csv_bytes(&[]))]);
        assert!(matches!(filings_with(&fake, BB, 2026).await, Err(AppError::ProviderUnavailable(_))));
    }

    // Ao vivo (rede, ~13 MB): `cargo test --lib -- --ignored --nocapture live_dividend_notices`.
    #[tokio::test]
    #[ignore]
    async fn live_dividend_notices() {
        let notices = dividend_notices(1023).await.unwrap();
        assert_eq!(notices.source, SOURCE);
        println!("Banco do Brasil: {} notice(s), last {:?}", notices.value.len(), notices.value.last());
        assert!(!notices.value.is_empty());
        assert!(notices.value.iter().all(|n| n.link_download.starts_with("https://www.rad.cvm.gov.br/")));
        assert!(notices.value.windows(2).all(|w| w[0].data_entrega <= w[1].data_entrega));
    }
}

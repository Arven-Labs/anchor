// Curva DI futuro (DI×PRE) da B3 — porta de `lume/api/app/sources/b3_taxa_swap.py`.
// Arquivo TaxaSwap do sistema legado "Pesquisa por Pregão" (o UP2DATA novo
// exige sessão/Cloudflare): público, sem chave.
//
// `GET https://www.b3.com.br/pesquisapregao/download?filelist=TS{YYMMDD}.ex_,`
// devolve um zip externo com um blob autoextraível (`.ex_`) que embute um zip
// interno com `TaxaSwap.txt` (largura fixa, latin-1). Colunas (base 0):
//   [21:26] código da curva ('PRE  ', 'DIC  ')   [41:46] dias corridos
//   [46:51] dias úteis                            [51]    sinal da taxa
//   [52:66] taxa teórica (14 dígitos, ÷1e9)       [66]    vértice F/M
// Data sem pregão devolve HTTP 200 com um zip vazio de 22 bytes: é "sem dado".
// TLS valida normalmente; não desligamos a verificação de certificado.

use std::io::{Cursor, Read};
use std::sync::OnceLock;
use std::time::Duration;

use chrono::NaiveDate;

use crate::error::AppError;
use crate::finance_core::cache::{Cached, TtlCache};
use crate::finance_core::http;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "b3_taxa_swap";
const URL_TEMPLATE: &str = "https://www.b3.com.br/pesquisapregao/download?filelist=TS";
const TIMEOUT: Duration = Duration::from_secs(30);
const CURVE_CODE: &str = "PRE";
/// Zip vazio (sem entradas) tem 22 bytes.
const EMPTY_ZIP_MAX_SIZE: usize = 22;
/// A B3 recusa clientes sem navegador.
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
/// Mesmo TTL padrão do Lume (`cache_ttl_seconds`).
const TTL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, PartialEq)]
pub struct DiVertex {
    pub dias_corridos: i32,
    pub dias_uteis: i32,
    pub rate_pct: f64,
    /// `F` fixo ou `M` móvel.
    pub vertice_type: char,
}

fn bad(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("B3 TaxaSwap: {detail}"))
}

/// Abre o zip externo, acha o zip interno dentro do blob `.ex_` pela assinatura
/// `PK\x03\x04` e devolve o `TaxaSwap.txt` decodificado em latin-1.
pub fn extract_taxa_swap_txt(raw: &[u8]) -> Result<String, AppError> {
    let first_entry = |bytes: &[u8], what: &str| -> Result<Vec<u8>, AppError> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(bad)?;
        if archive.is_empty() {
            return Err(bad(format!("{what} zip is empty")));
        }
        let mut entry = archive.by_index(0).map_err(bad)?;
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(bad)?;
        Ok(data)
    };

    let blob = first_entry(raw, "outer .ex_")?;
    let offset = blob
        .windows(4)
        .position(|w| w == b"PK\x03\x04")
        .ok_or_else(|| bad("could not locate inner zip signature inside .ex_ blob"))?;
    // No arquivo real da B3 os offsets do diretório central são relativos ao
    // blob inteiro (conferido ao vivo: o zip interno começa no byte 86036 e é
    // lá que o diretório central aponta), então o blob todo já é um zip válido
    // com um executável na frente. Num zip cujos offsets são relativos ao seu
    // próprio início (o que o teste do Lume monta) vale o recorte a partir da
    // assinatura. O Python tolerava os dois; a crate `zip` exige escolher.
    let txt = match first_entry(&blob, "inner") {
        Ok(txt) => txt,
        Err(_) => first_entry(&blob[offset..], "inner")?,
    };
    Ok(txt.iter().map(|&b| b as char).collect()) // latin-1: cada byte é um code point
}

/// Vértices da curva `PRE`, na ordem do arquivo. Linhas de outras curvas ou
/// curtas demais são ignoradas.
pub fn parse_pre_curve(txt: &str) -> Result<Vec<DiVertex>, AppError> {
    let mut vertices = Vec::new();
    for line in txt.lines() {
        let chars: Vec<char> = line.chars().collect();
        if chars.len() < 67 {
            continue;
        }
        let field = |range: std::ops::Range<usize>| chars[range].iter().collect::<String>();
        if field(21..26).trim_end() != CURVE_CODE {
            continue;
        }
        let number = |range: std::ops::Range<usize>, what: &str| -> Result<i64, AppError> {
            field(range.clone()).trim().parse::<i64>().map_err(|_| bad(format!("invalid {what}")))
        };
        let sign = if chars[51] == '+' { 1.0 } else { -1.0 };
        vertices.push(DiVertex {
            dias_corridos: number(41..46, "dias corridos")? as i32,
            dias_uteis: number(46..51, "dias uteis")? as i32,
            rate_pct: sign * number(52..66, "rate")? as f64 / 1e9 * 100.0,
            vertice_type: chars[66],
        });
    }
    Ok(vertices)
}

/// Curva do dia: `None` em data sem pregão (fim de semana, feriado, ainda não publicada).
async fn fetch_curve(date: NaiveDate) -> Result<Option<Vec<DiVertex>>, AppError> {
    let url = format!("{URL_TEMPLATE}{}.ex_,", date.format("%y%m%d"));
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(BROWSER_UA)
        .build()
        .map_err(bad)?;
    let response = client.get(&url).send().await.map_err(bad)?;
    let status = response.status().as_u16();
    if status != 200 {
        http::check_status("B3 TaxaSwap", status, String::new())?;
    }
    let raw = response.bytes().await.map_err(bad)?;
    if raw.len() <= EMPTY_ZIP_MAX_SIZE {
        return Ok(None);
    }
    Ok(Some(parse_pre_curve(&extract_taxa_swap_txt(&raw)?)?))
}

pub trait DiCurveProvider: DataProvider {
    fn curve<'a>(&'a self, date: NaiveDate) -> BoxFuture<'a, Result<Option<Vec<DiVertex>>, AppError>>;
}

pub struct B3TaxaSwap;

impl DataProvider for B3TaxaSwap {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl DiCurveProvider for B3TaxaSwap {
    fn curve<'a>(&'a self, date: NaiveDate) -> BoxFuture<'a, Result<Option<Vec<DiVertex>>, AppError>> {
        Box::pin(fetch_curve(date))
    }
}

fn providers() -> Vec<Box<dyn DiCurveProvider>> {
    vec![Box::new(B3TaxaSwap)]
}

type Curve = Sourced<Option<Vec<DiVertex>>>;
static CACHE: OnceLock<TtlCache<NaiveDate, Curve>> = OnceLock::new();

/// Curva DI×PRE de `date`, ordenada por dias úteis (vértices repetidos: fica o
/// primeiro, como o `ON CONFLICT DO NOTHING` do Lume). Sem pregão é `NoData`.
/// Cache de 1 h na memória, inclusive dos dias sem pregão: a Home volta dia a
/// dia procurando a última curva e sem isso repetiria o download a cada abertura.
pub async fn di_curve(date: NaiveDate) -> Result<Cached<Sourced<Vec<DiVertex>>>, AppError> {
    let cached = CACHE
        .get_or_init(TtlCache::new)
        .get_or_refresh(date, TTL, || async {
            let providers = providers();
            let attempts = providers.iter().map(|p| (p.id(), p.curve(date))).collect();
            cascade(&format!("DI curve {date}"), attempts).await
        })
        .await?;
    let Cached { value, cached, stale, fetched_at } = cached;
    let source = value.source;
    match value.value {
        Some(mut vertices) if !vertices.is_empty() => {
            vertices.sort_by_key(|v| v.dias_uteis);
            vertices.dedup_by_key(|v| v.dias_uteis);
            Ok(Cached { value: Sourced { value: vertices, source }, cached, stale, fetched_at })
        }
        _ => Err(AppError::NoData(format!("no DI curve for {date} (no trading)"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn line(curve: &str, corridos: i32, uteis: i32, rate_decimal: f64, vertice: char) -> String {
        let sign = if rate_decimal >= 0.0 { '+' } else { '-' };
        format!(
            "{}{:<5}{}{:>5}{:>5}{}{:0>14}{}00000",
            "0".repeat(21),
            curve,
            "X".repeat(15),
            corridos,
            uteis,
            sign,
            (rate_decimal.abs() * 1e9).round() as u64,
            vertice
        )
    }

    fn zip_of(name: &str, data: &[u8]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buf);
        writer
            .start_file(name, SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
            .unwrap();
        writer.write_all(data).unwrap();
        writer.finish().unwrap();
        buf.into_inner()
    }

    /// Mesma montagem do teste do Lume: blob com cabeçalho de executável + zip interno.
    fn taxa_swap_file(lines: &[String]) -> Vec<u8> {
        let inner = zip_of("TaxaSwap.txt", lines.join("\r\n").as_bytes());
        let mut blob = b"SELF_EXTRACTING_EXE_HEADER".to_vec();
        blob.extend(inner);
        zip_of("TS260915.ex_", &blob)
    }

    #[test]
    fn extracts_and_parses_only_the_pre_curve() {
        let file = taxa_swap_file(&[
            line("PRE", 1, 1, 0.1390, 'F'),
            line("DIC", 1, 1, 0.0500, 'F'), // outra curva, ignorada
            line("PRE", 30, 21, 0.1370, 'M'),
        ]);
        let vertices = parse_pre_curve(&extract_taxa_swap_txt(&file).unwrap()).unwrap();
        assert_eq!(vertices.len(), 2);
        assert_eq!((vertices[0].dias_corridos, vertices[0].dias_uteis, vertices[0].vertice_type), (1, 1, 'F'));
        assert!((vertices[0].rate_pct - 13.9).abs() < 1e-9, "{}", vertices[0].rate_pct);
        assert_eq!((vertices[1].dias_corridos, vertices[1].dias_uteis, vertices[1].vertice_type), (30, 21, 'M'));
        assert!((vertices[1].rate_pct - 13.7).abs() < 1e-9);
    }

    #[test]
    fn negative_rates_keep_their_sign() {
        let vertices = parse_pre_curve(&line("PRE", 5, 3, -0.0123, 'F')).unwrap();
        assert!((vertices[0].rate_pct + 1.23).abs() < 1e-9, "{}", vertices[0].rate_pct);
    }

    #[test]
    fn short_lines_are_ignored() {
        assert!(parse_pre_curve("too short\r\n\r\n").unwrap().is_empty());
    }

    #[test]
    fn malformed_zip_is_provider_unavailable() {
        assert!(matches!(extract_taxa_swap_txt(b"not a zip at all"), Err(AppError::ProviderUnavailable(_))));
        let no_inner = zip_of("TS.ex_", b"no signature in here");
        assert!(matches!(extract_taxa_swap_txt(&no_inner), Err(AppError::ProviderUnavailable(_))));
    }

    #[test]
    fn an_empty_zip_is_at_most_the_empty_size() {
        let empty = zip::ZipWriter::new(Cursor::new(Vec::new())).finish().unwrap().into_inner();
        assert!(empty.len() <= EMPTY_ZIP_MAX_SIZE, "{} bytes", empty.len());
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored finance_core::b3_taxa_swap`.
    #[tokio::test]
    #[ignore]
    async fn live_curve_for_a_recent_trading_day_and_a_sunday() {
        let mut date = chrono::Utc::now().date_naive();
        let curve = loop {
            match di_curve(date).await {
                Ok(curve) => break curve,
                Err(AppError::NoData(_)) => date = date.pred_opt().unwrap(),
                Err(other) => panic!("{other}"),
            }
        };
        println!("curve {date}: {} vertices, first {:?}", curve.value.value.len(), curve.value.value[0]);
        assert_eq!(curve.value.source, SOURCE);
        assert!(curve.value.value.len() > 200);
        assert!(di_curve(date).await.unwrap().cached);

        let sunday = NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();
        assert!(matches!(di_curve(sunday).await.unwrap_err(), AppError::NoData(_)));
    }
}

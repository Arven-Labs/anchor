// Opções de ações/ETFs da B3: catálogo de séries e último preço negociado —
// porta de `lume/api/app/sources/b3_options_series.py` e `b3_cotahist.py`. As
// duas fontes são grátis e sem chave, e juntas dão tudo que existe de graça: não
// há bid/ask, open interest nem IV de mercado.
//
// - **Séries Autorizadas**: um zip com um arquivo de texto `|` (latin-1) com o
//   universo de séries registradas hoje. Duas formas de linha: o tipo `02`
//   (ação/ETF, ~85 mil de ~87 mil) e o `03` (opção de índice em pontos, layout
//   diferente), que fica de fora de propósito: a exposição a índice já vem das
//   opções de BOVA11, que são `02`.
// - **COTAHIST anual**: um zip por ano (~80 MB, ~700 MB descomprimido, ~2,8
//   milhões de linhas de 245 bytes, 86 % opção) com todos os pregões. Não existe
//   COTAHIST diário. Guardamos só o último preço por série.
//
// Os dois arquivos são o mercado inteiro, então o cache é **global** (não por
// ativo): em memória, 24 h, com o fallback `stale` do `TtlCache`. Os arquivos
// ficam no `disk_cache`, pra um reinício do app não baixar tudo de novo. A
// primeira chamada leva ~2 min.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};

use crate::error::AppError;
use crate::finance_core::black_scholes::OptionType;
use crate::finance_core::cache::{Cached, TtlCache};
use crate::finance_core::cvm_dfp::latin1;
use crate::finance_core::disk_cache;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "b3_series_autorizadas+b3_cotahist";
/// O `fileId` é o identificador do arquivo no CMS da B3 e **muda quando ela
/// republica o arquivo** (mudou entre 2026-09 e 2026-10: o antigo, `...CCA023A7E4...`,
/// passou a redirecionar para a página de erro). Se o download deixar de ser um
/// zip, pegue o link atual em `SERIES_PAGE` (a página é lenta, ~70 s).
const SERIES_URL: &str = "https://www.b3.com.br/lumis/portal/file/fileDownload.jsp?fileId=8AA8D0CDA134A78701A146F166082C07";
const SERIES_PAGE: &str = "https://www.b3.com.br/pt_br/market-data-e-indices/servicos-de-dados/market-data/consultas/mercado-a-vista/opcoes/series-autorizadas/";
const COTAHIST_URL: &str = "https://bvmf.bmfbovespa.com.br/InstDados/SerHist";
/// A B3 recusa clientes sem navegador.
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

const SERIES_TIMEOUT: Duration = Duration::from_secs(60);
/// ~80 MB: o Lume dava 180 s só pro download.
const COTAHIST_TIMEOUT: Duration = Duration::from_secs(300);
/// Os arquivos mudam todo dia útil; o mesmo TTL do Lume (`options_ttl_seconds`).
const TTL: Duration = Duration::from_secs(24 * 3600);

const EQUITY_ROW_TYPE: &str = "02";
const CALL_MARKET_CODE: &str = "70";
const AMERICAN_STYLE_LABEL: &str = "Americano";

/// Raiz da opção que não é o ticker sem o dígito da classe. Sem mapa genérico
/// público: caso a caso (PENDING P2 do Lume). Embraer negocia como `EMBR3` e suas
/// opções são `EMBJ…`.
const ROOT_OVERRIDES: [(&str, &str); 1] = [("EMBR", "EMBJ")];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionStyle {
    American,
    European,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptionSeries {
    pub series_ticker: String,
    /// Raiz do ativo-objeto como a B3 registra (`PETR`, `BOVA`, `EMBJ`).
    pub root: String,
    pub option_type: OptionType,
    pub strike: f64,
    pub expiration: NaiveDate,
    pub style: OptionStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LastTrade {
    pub trade_date: NaiveDate,
    pub close_price: f64,
}

/// O mercado inteiro de uma vez: o que está registrado e o que já negociou.
#[derive(Debug, Clone, Default)]
pub struct OptionsMarket {
    pub series: Vec<OptionSeries>,
    pub last_trades: HashMap<String, LastTrade>,
}

impl OptionsMarket {
    pub fn find(&self, series_ticker: &str) -> Option<&OptionSeries> {
        self.series.iter().find(|s| s.series_ticker == series_ticker)
    }

    /// Séries de `root` por vencimento e strike, cada uma com o último negócio, se houve.
    pub fn for_root(&self, root: &str) -> Vec<(OptionSeries, Option<LastTrade>)> {
        let mut found: Vec<_> = self
            .series
            .iter()
            .filter(|s| s.root == root)
            .map(|s| (s.clone(), self.last_trades.get(&s.series_ticker).copied()))
            .collect();
        found.sort_by(|a, b| {
            a.0.expiration
                .cmp(&b.0.expiration)
                .then(a.0.strike.total_cmp(&b.0.strike))
                .then_with(|| a.0.series_ticker.cmp(&b.0.series_ticker))
        });
        found
    }
}

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("B3 options: {detail}"))
}

/// Raiz da opção a partir do ticker do ativo: tira os dígitos finais (`PETR4` →
/// `PETR`) e aplica as exceções conhecidas (`EMBR3` → `EMBJ`).
pub fn option_root(ticker: &str) -> String {
    let root = ticker.trim().to_uppercase().trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
    ROOT_OVERRIDES
        .iter()
        .find(|(from, _)| *from == root)
        .map_or(root, |(_, to)| (*to).to_string())
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_series_line(line: &str) -> Result<Option<OptionSeries>, AppError> {
    let fields: Vec<&str> = line.trim_end_matches(['\r', '\n']).split('|').collect();
    if fields.first().copied() != Some(EQUITY_ROW_TYPE) {
        return Ok(None);
    }
    let field = |i: usize| fields.get(i).copied().ok_or_else(|| unavailable(format!("series row has {} fields", fields.len())));
    let root = field(6)?.split_whitespace().next().ok_or_else(|| unavailable("series row without root"))?;
    let strike = field(16)?.trim().parse::<f64>().map_err(|_| unavailable(format!("invalid strike '{}'", fields[16])))?;
    let expiration = NaiveDate::parse_from_str(field(17)?.trim(), "%Y%m%d")
        .map_err(|e| unavailable(format!("invalid expiration '{}': {e}", fields[17])))?;
    Ok(Some(OptionSeries {
        series_ticker: field(13)?.trim().to_string(),
        root: root.to_string(),
        option_type: if field(2)? == CALL_MARKET_CODE { OptionType::Call } else { OptionType::Put },
        strike,
        expiration,
        style: if field(15)? == AMERICAN_STYLE_LABEL { OptionStyle::American } else { OptionStyle::European },
    }))
}

/// Séries registradas (tipo `02`) do texto já decodificado de Séries Autorizadas.
pub fn parse_series(text: &str) -> Result<Vec<OptionSeries>, AppError> {
    let mut series = Vec::new();
    for line in text.lines() {
        if let Some(parsed) = parse_series_line(line)? {
            series.push(parsed);
        }
    }
    Ok(series)
}

/// Fatias fixas do COTAHIST (0-based, fim exclusivo), em bytes: o arquivo é
/// latin-1, então um byte é um caractere.
const TPREG: std::ops::Range<usize> = 0..2;
const DATA: std::ops::Range<usize> = 2..10;
const CODNEG: std::ops::Range<usize> = 12..24;
const TPMERC: std::ops::Range<usize> = 24..27;
const PREULT: std::ops::Range<usize> = 108..121;
const MIN_LINE_LEN: usize = 121;

/// Último negócio de cada série de opção no COTAHIST. Lê linha a linha (o arquivo
/// descomprimido passa de 700 MB) e fica com o pregão de maior data de cada
/// série, sem supor que o arquivo está ordenado. Linhas que não são cotação
/// (`TPREG` != `01`, ou seja, cabeçalho `00` e rodapé `99`) e os mercados que não
/// são opção (`TPMERC` != 070/080) são ignorados. Toda linha de opção é um
/// negócio de verdade (`TOTNEG` nunca é 0).
pub fn parse_cotahist(mut reader: impl BufRead) -> Result<HashMap<String, LastTrade>, AppError> {
    let mut latest: HashMap<String, LastTrade> = HashMap::new();
    let mut line = Vec::with_capacity(256);
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line).map_err(unavailable)? == 0 {
            break;
        }
        if line.len() < MIN_LINE_LEN || &line[TPREG] != b"01" || !matches!(&line[TPMERC], b"070" | b"080") {
            continue;
        }
        let text = |range: std::ops::Range<usize>| latin1(&line[range]);
        let ticker = text(CODNEG).trim().to_string();
        let date = text(DATA);
        let trade_date = NaiveDate::parse_from_str(&date, "%Y%m%d").map_err(|e| unavailable(format!("COTAHIST date '{date}': {e}")))?;
        let price = text(PREULT);
        let close_price = price
            .trim()
            .parse::<u64>()
            .map_err(|_| unavailable(format!("COTAHIST price '{price}' of {ticker}")))? as f64
            / 100.0;
        match latest.get(&ticker) {
            Some(existing) if existing.trade_date > trade_date => {}
            _ => {
                latest.insert(ticker, LastTrade { trade_date, close_price });
            }
        }
    }
    Ok(latest)
}

fn read_series(zip_path: &Path) -> Result<Vec<OptionSeries>, AppError> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(unavailable)?;
    let mut entry = archive.by_index(0).map_err(unavailable)?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut bytes)?;
    parse_series(&latin1(&bytes))
}

fn read_cotahist(zip_path: &Path) -> Result<HashMap<String, LastTrade>, AppError> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(unavailable)?;
    let entry = archive.by_index(0).map_err(unavailable)?;
    parse_cotahist(BufReader::new(entry))
}

// ---------------------------------------------------------------------------
// Download e orquestração
// ---------------------------------------------------------------------------

pub trait OptionsFiles: Send + Sync {
    /// Zip das Séries Autorizadas.
    fn series_zip<'a>(&'a self) -> BoxFuture<'a, Result<PathBuf, AppError>>;
    /// Zip do COTAHIST do ano `year`; erro se não existe/não baixou.
    fn cotahist_zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>>;
}

pub struct B3Files;

/// A B3 responde 200 com uma página HTML de erro quando o arquivo some ou muda de
/// lugar, e o `disk_cache` guardaria isso como se fosse o zip por 24 h. Confere o
/// cabeçalho `PK` e, se não for zip, apaga o arquivo e explica.
fn ensure_zip(path: PathBuf, what: &str, hint: &str) -> Result<PathBuf, AppError> {
    let mut magic = [0u8; 2];
    let is_zip = File::open(&path)
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut magic))
        .map(|()| &magic == b"PK")
        .unwrap_or(false);
    if is_zip {
        return Ok(path);
    }
    let _ = std::fs::remove_file(&path);
    Err(unavailable(format!("{what}: the download is not a zip ({hint})")))
}

impl OptionsFiles for B3Files {
    fn series_zip<'a>(&'a self) -> BoxFuture<'a, Result<PathBuf, AppError>> {
        Box::pin(async move {
            let path = disk_cache::get_or_download_with(
                "B3 Séries Autorizadas",
                "b3_options/series_autorizadas.zip",
                SERIES_URL,
                Some(TTL),
                &[("User-Agent", BROWSER_UA)],
                SERIES_TIMEOUT,
            )
            .await?;
            ensure_zip(path, "B3 Séries Autorizadas", &format!("the fileId probably changed; get the current link at {SERIES_PAGE}"))
        })
    }

    fn cotahist_zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
        Box::pin(async move {
            let name = format!("COTAHIST_A{year}.ZIP");
            let path = disk_cache::get_or_download_with(
                "B3 COTAHIST",
                &format!("b3_options/{name}"),
                &format!("{COTAHIST_URL}/{name}"),
                Some(TTL),
                &[],
                COTAHIST_TIMEOUT,
            )
            .await?;
            ensure_zip(path, "B3 COTAHIST", &format!("{name} may not be published yet"))
        })
    }
}

async fn year_trades(files: &dyn OptionsFiles, year: i32) -> Result<HashMap<String, LastTrade>, AppError> {
    let path = files.cotahist_zip(year).await?;
    tokio::task::spawn_blocking(move || read_cotahist(&path))
        .await
        .map_err(|e| unavailable(format!("parsing task failed: {e}")))?
}

/// Último preço de cada série. Em geral basta o COTAHIST do ano corrente, e se ele
/// falhar o erro sobe (o `TtlCache` serve o dado antigo como `stale`). O ano
/// anterior só entra em **janeiro**, quando o arquivo novo ainda tem poucos
/// pregões: aí os dois são juntos, valendo o negócio de data mais recente, e um
/// deles pode faltar. Fora de janeiro cair pro ano anterior serviria preços de meses
/// atrás como se fossem atuais (a B3 oscila, e isso já aconteceu num teste ao vivo).
async fn last_trades(files: &dyn OptionsFiles, today: NaiveDate) -> Result<HashMap<String, LastTrade>, AppError> {
    let current = year_trades(files, today.year()).await;
    if today.month() != 1 {
        return current;
    }
    match (current, year_trades(files, today.year() - 1).await) {
        (Ok(mut merged), Ok(previous)) => {
            for (ticker, trade) in previous {
                merged.entry(ticker).and_modify(|kept| {
                    if trade.trade_date > kept.trade_date {
                        *kept = trade;
                    }
                }).or_insert(trade);
            }
            Ok(merged)
        }
        (Ok(only), Err(_)) | (Err(_), Ok(only)) => Ok(only),
        (Err(err), Err(_)) => Err(err),
    }
}

pub async fn market_with(files: &dyn OptionsFiles, today: NaiveDate) -> Result<OptionsMarket, AppError> {
    let series_path = files.series_zip().await?;
    let series = tokio::task::spawn_blocking(move || read_series(&series_path))
        .await
        .map_err(|e| unavailable(format!("parsing task failed: {e}")))??;
    let last_trades = last_trades(files, today).await?;
    Ok(OptionsMarket { series, last_trades })
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub trait OptionsProvider: DataProvider {
    fn market<'a>(&'a self) -> BoxFuture<'a, Result<OptionsMarket, AppError>>;
}

pub struct B3Options;

impl DataProvider for B3Options {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl OptionsProvider for B3Options {
    fn market<'a>(&'a self) -> BoxFuture<'a, Result<OptionsMarket, AppError>> {
        Box::pin(async move { market_with(&B3Files, Utc::now().date_naive()).await })
    }
}

fn providers() -> Vec<Box<dyn OptionsProvider>> {
    vec![Box::new(B3Options)]
}

static CACHE: OnceLock<TtlCache<(), Sourced<OptionsMarket>>> = OnceLock::new();

/// O mercado de opções inteiro, em cache de 24 h na memória (global: o arquivo é
/// o mercado todo). A primeira chamada baixa ~80 MB e leva ~2 min.
pub async fn market() -> Result<Cached<Sourced<OptionsMarket>>, AppError> {
    CACHE
        .get_or_init(TtlCache::new)
        .get_or_refresh((), TTL, || async {
            let providers = providers();
            let attempts = providers.iter().map(|p| (p.id(), p.market())).collect();
            cascade("B3 options market", attempts).await
        })
        .await
}

/// Séries de uma raiz (`PETR`) com o último negócio de cada uma. Raiz
/// desconhecida é lista vazia, não erro.
pub async fn series_for_root(root: &str) -> Result<Cached<Sourced<Vec<(OptionSeries, Option<LastTrade>)>>>, AppError> {
    let cached = market().await?;
    let root = root.trim().to_uppercase();
    let Cached { value, cached, stale, fetched_at } = cached;
    Ok(Cached {
        value: Sourced { value: value.value.for_root(&root), source: value.source },
        cached,
        stale,
        fetched_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    /// Linha do tipo `02`: só os campos que o parser lê têm valor de verdade.
    fn series_line(market: &str, root: &str, ticker: &str, style: &str, strike: &str, expiry: &str) -> String {
        let mut f = vec![String::new(); 18];
        f[0] = "02".into();
        f[1] = "PETROBRAS".into();
        f[2] = market.into();
        f[6] = root.into();
        f[13] = ticker.into();
        f[14] = "1".into();
        f[15] = style.into();
        f[16] = strike.into();
        f[17] = expiry.into();
        f.join("|")
    }

    #[test]
    fn parses_equity_series_and_skips_other_rows() {
        let text = [
            "01|20260915|20260916|00:01:43".to_string(),
            series_line("70", "PETR", "PETRJ199 ", "Europeu", "17.610000000000", "20261016"),
            series_line("80", "BOVA  FM", "BOVAV125", "Americano", "125.000000000000", "20261019"),
            "03|IBOVESPA|70|x".to_string(),
        ]
        .join("\r\n");
        let series = parse_series(&text).unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(
            series[0],
            OptionSeries {
                series_ticker: "PETRJ199".into(),
                root: "PETR".into(),
                option_type: OptionType::Call,
                strike: 17.61,
                expiration: date(2026, 10, 16),
                style: OptionStyle::European,
            }
        );
        assert_eq!(series[1].root, "BOVA");
        assert_eq!(series[1].option_type, OptionType::Put);
        assert_eq!(series[1].style, OptionStyle::American);
    }

    #[test]
    fn a_short_equity_row_is_provider_unavailable() {
        assert!(matches!(parse_series("02|x|70"), Err(AppError::ProviderUnavailable(_))));
    }

    #[test]
    fn a_bad_expiration_is_provider_unavailable() {
        let line = series_line("70", "PETR", "PETRJ199", "Europeu", "17.61", "16/10/2026");
        assert!(matches!(parse_series(&line), Err(AppError::ProviderUnavailable(_))));
    }

    #[test]
    fn roots_drop_the_class_digit_and_apply_overrides() {
        assert_eq!(option_root("PETR4"), "PETR");
        assert_eq!(option_root("vale3"), "VALE");
        assert_eq!(option_root("BOVA11"), "BOVA");
        assert_eq!(option_root("EMBR3"), "EMBJ");
    }

    /// Linha de 245 bytes do COTAHIST, no layout oficial da B3.
    fn cotahist_line(tpreg: &str, codneg: &str, tpmerc: &str, data: &str, preult_reais: f64) -> String {
        let preult = format!("{:0>13}", (preult_reais * 100.0).round() as u64);
        let line = [
            tpreg.to_string(),
            data.to_string(),
            "78".into(),
            format!("{codneg:<12}"),
            tpmerc.into(),
            "X".repeat(12),
            "X".repeat(10),
            "0".repeat(3),
            "R$  ".into(),
            "0".repeat(52),
            preult,
            "0".repeat(13 * 2),
            "00001".into(),
            "0".repeat(18 * 2),
            "0".repeat(13),
            "0".into(),
            "20261016".into(),
            "0".repeat(7),
            "0".repeat(13),
            "X".repeat(12),
            "0".repeat(3),
        ]
        .concat();
        assert_eq!(line.len(), 245, "layout do COTAHIST");
        line
    }

    fn cotahist(lines: &[String]) -> Cursor<Vec<u8>> {
        Cursor::new((lines.join("\r\n") + "\r\n").into_bytes())
    }

    #[test]
    fn cotahist_keeps_the_latest_trade_per_option_series() {
        let trades = parse_cotahist(cotahist(&[
            format!("00COTAHIST.2026{}", " ".repeat(230)),
            cotahist_line("01", "PETRJ199", "070", "20260105", 1.50),
            cotahist_line("01", "PETRJ199", "070", "20260310", 2.25),
            cotahist_line("01", "PETRJ199", "070", "20260210", 9.99),
            cotahist_line("01", "BOVAV125", "080", "20260301", 0.07),
            format!("99COTAHIST.2026{}", " ".repeat(230)),
        ]))
        .unwrap();
        assert_eq!(trades.len(), 2);
        assert_eq!(trades["PETRJ199"], LastTrade { trade_date: date(2026, 3, 10), close_price: 2.25 });
        assert_eq!(trades["BOVAV125"], LastTrade { trade_date: date(2026, 3, 1), close_price: 0.07 });
    }

    #[test]
    fn cotahist_ignores_non_option_markets() {
        let trades = parse_cotahist(cotahist(&[
            cotahist_line("01", "PETR4", "010", "20260105", 38.0),
            cotahist_line("01", "PETRJ199", "070", "20260105", 1.5),
        ]))
        .unwrap();
        assert_eq!(trades.keys().collect::<Vec<_>>(), ["PETRJ199"]);
    }

    #[test]
    fn cotahist_a_later_row_wins_a_same_day_tie() {
        let trades = parse_cotahist(cotahist(&[
            cotahist_line("01", "PETRJ199", "070", "20260105", 1.0),
            cotahist_line("01", "PETRJ199", "070", "20260105", 2.0),
        ]))
        .unwrap();
        assert_eq!(trades["PETRJ199"].close_price, 2.0);
    }

    #[test]
    fn cotahist_garbage_price_is_provider_unavailable() {
        let mut line = cotahist_line("01", "PETRJ199", "070", "20260105", 1.0);
        line.replace_range(108..121, "XXXXXXXXXXXXX");
        assert!(matches!(parse_cotahist(cotahist(&[line])), Err(AppError::ProviderUnavailable(_))));
    }

    fn zip_bytes(name: &str, data: &[u8]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buf);
        writer
            .start_file(name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
            .unwrap();
        writer.write_all(data).unwrap();
        writer.finish().unwrap();
        buf.into_inner()
    }

    /// Arquivos falsos: o zip das séries e um COTAHIST por ano (ano ausente = falha).
    struct Fake {
        series: PathBuf,
        cotahist: HashMap<i32, PathBuf>,
    }

    impl Fake {
        fn new(series_text: &str, cotahist: Vec<(i32, Vec<String>)>) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "anchor-options-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let series = dir.join("series.zip");
            let latin1_bytes: Vec<u8> = series_text.chars().map(|c| c as u8).collect();
            std::fs::write(&series, zip_bytes("SI_D_SEDE.txt", &latin1_bytes)).unwrap();
            let cotahist = cotahist
                .into_iter()
                .map(|(year, lines)| {
                    let path = dir.join(format!("cotahist_{year}.zip"));
                    std::fs::write(&path, zip_bytes(&format!("COTAHIST_A{year}.TXT"), (lines.join("\r\n") + "\r\n").as_bytes())).unwrap();
                    (year, path)
                })
                .collect();
            Fake { series, cotahist }
        }
    }

    impl OptionsFiles for Fake {
        fn series_zip<'a>(&'a self) -> BoxFuture<'a, Result<PathBuf, AppError>> {
            Box::pin(async move { Ok(self.series.clone()) })
        }
        fn cotahist_zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
            Box::pin(async move {
                self.cotahist.get(&year).cloned().ok_or_else(|| unavailable(format!("download failed for {year}")))
            })
        }
    }

    fn series_text() -> String {
        [
            series_line("70", "PETR", "PETRJ199", "Europeu", "17.61", "20261016"),
            series_line("70", "PETR", "PETRL522", "Europeu", "49.91", "20261221"),
            series_line("80", "PETR", "PETRX400", "Europeu", "40.00", "20261221"),
            series_line("70", "PETR", "PETRU696", "Europeu", "69.60", "20261221"),
            series_line("80", "BOVA", "BOVAV125", "Europeu", "125.00", "20261019"),
        ]
        .join("\n")
    }

    #[tokio::test]
    async fn market_joins_series_with_last_trades_by_root() {
        let fake = Fake::new(
            &series_text(),
            vec![(2026, vec![
                cotahist_line("01", "PETRL522", "070", "20260916", 3.10),
                cotahist_line("01", "PETRJ199", "070", "20260915", 1.50),
            ])],
        );
        let market = market_with(&fake, date(2026, 10, 16)).await.unwrap();
        let petr = market.for_root("PETR");
        let tickers: Vec<_> = petr.iter().map(|(s, _)| s.series_ticker.as_str()).collect();
        // Por vencimento e depois strike.
        assert_eq!(tickers, ["PETRJ199", "PETRX400", "PETRL522", "PETRU696"]);
        assert_eq!(petr[0].1.unwrap().close_price, 1.5);
        assert!(petr[1].1.is_none(), "série registrada que nunca negociou");
        assert!(market.for_root("EMBJ").is_empty());
        assert!(market.find("PETRL522").is_some());
        assert!(market.find("NOPE").is_none());
    }

    #[tokio::test]
    async fn in_january_the_previous_year_fills_in_and_the_latest_trade_wins() {
        let fake = Fake::new(
            &series_text(),
            vec![
                (2026, vec![cotahist_line("01", "PETRJ199", "070", "20260105", 1.1)]),
                (2025, vec![
                    cotahist_line("01", "PETRJ199", "070", "20251230", 0.9),
                    cotahist_line("01", "BOVAV125", "080", "20251229", 0.4),
                ]),
            ],
        );
        let market = market_with(&fake, date(2026, 1, 12)).await.unwrap();
        assert_eq!(market.last_trades["PETRJ199"].close_price, 1.1, "o negócio de janeiro vale mais que o de dezembro");
        assert_eq!(market.last_trades["BOVAV125"].close_price, 0.4, "só negociou no ano anterior");
    }

    #[tokio::test]
    async fn in_january_the_new_year_file_may_not_exist_yet() {
        let fake = Fake::new(&series_text(), vec![(2025, vec![cotahist_line("01", "PETRJ199", "070", "20251230", 0.9)])]);
        let market = market_with(&fake, date(2026, 1, 3)).await.unwrap();
        assert_eq!(market.last_trades["PETRJ199"].close_price, 0.9);
    }

    #[tokio::test]
    async fn outside_january_a_missing_current_year_never_serves_old_prices() {
        let fake = Fake::new(&series_text(), vec![(2025, vec![cotahist_line("01", "PETRJ199", "070", "20251230", 0.9)])]);
        assert!(matches!(market_with(&fake, date(2026, 10, 16)).await, Err(AppError::ProviderUnavailable(_))));
    }

    #[test]
    fn an_html_error_page_is_not_kept_as_a_zip() {
        let dir = std::env::temp_dir().join(format!("anchor-options-zip-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();

        let html = dir.join("error.zip");
        std::fs::write(&html, "<!DOCTYPE html><html>erro</html>").unwrap();
        let err = ensure_zip(html.clone(), "B3 test", "hint").unwrap_err();
        assert!(matches!(&err, AppError::ProviderUnavailable(m) if m.contains("not a zip") && m.contains("hint")), "{err}");
        assert!(!html.exists(), "o HTML não pode ficar em cache");

        let real = dir.join("real.zip");
        std::fs::write(&real, zip_bytes("x.txt", b"ok")).unwrap();
        assert_eq!(ensure_zip(real.clone(), "B3 test", "hint").unwrap(), real);

        assert!(ensure_zip(dir.join("missing.zip"), "B3 test", "hint").is_err());
    }

    #[tokio::test]
    async fn no_cotahist_at_all_is_provider_unavailable() {
        let fake = Fake::new(&series_text(), vec![]);
        assert!(matches!(market_with(&fake, date(2026, 10, 16)).await, Err(AppError::ProviderUnavailable(_))));
        assert!(matches!(market_with(&fake, date(2026, 1, 12)).await, Err(AppError::ProviderUnavailable(_))));
    }

    // Ao vivo (rede, ~80 MB, ~2 min na primeira vez):
    // `cargo test --lib -- --ignored --nocapture live_options_series`.
    #[tokio::test]
    #[ignore]
    async fn live_options_series() {
        let petr = series_for_root("PETR").await.unwrap();
        assert_eq!(petr.value.source, SOURCE);
        let traded = petr.value.value.iter().filter(|(_, last)| last.is_some()).count();
        println!("PETR: {} series, {} with a last trade", petr.value.value.len(), traded);
        assert!(petr.value.value.len() > 100 && traded > 10);
        assert!(series_for_root("ZZZZ").await.unwrap().value.value.is_empty());

        let market = market().await.unwrap().value.value;
        println!("market: {} series, {} last prices", market.series.len(), market.last_trades.len());
        assert!(!market.for_root(&option_root("EMBR3")).is_empty(), "EMBJ deveria ter séries");
    }
}

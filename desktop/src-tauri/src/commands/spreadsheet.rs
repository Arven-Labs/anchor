// Leitura de planilha (CSV, xls, xlsx) como texto. Módulo neutro: o importador da B3
// (`portfolio_io`) e a prévia de anexo (`property`, `thesis`) leem daqui, sem dependerem
// uns dos outros (ADR 0007, passo 4).
use crate::error::AppError;

/// Lê a primeira aba do xlsx como texto, linha a linha.
pub fn read_xlsx_rows(path: &str) -> Result<Vec<Vec<String>>, AppError> {
    use calamine::{open_workbook_auto, Data, Reader};
    let mut workbook =
        open_workbook_auto(path).map_err(|e| AppError::InvalidInput(format!("planilha ilegível: {e}")))?;
    let name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| AppError::InvalidInput("planilha sem abas".to_string()))?;
    let range = workbook
        .worksheet_range(&name)
        .map_err(|e| AppError::InvalidInput(format!("aba '{name}' ilegível: {e}")))?;
    let text = |cell: &Data| match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt
            .as_datetime()
            .map(|d| d.format("%d/%m/%Y").to_string())
            .unwrap_or_default(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(_) => String::new(),
    };
    Ok(range.rows().map(|row| row.iter().map(&text).collect()).collect())
}

/// Teto de tamanho do arquivo lido para a prévia de anexo (bytes).
const PREVIEW_MAX_BYTES: u64 = 20 * 1024 * 1024;
/// Teto de linhas devolvidas na prévia de anexo.
const PREVIEW_MAX_ROWS: usize = 1000;

/// Lê um anexo de planilha (CSV, xls ou xlsx) como texto, para a prévia da tela.
/// O tipo vem do `content_type` gravado no anexo, não da extensão do arquivo.
pub fn read_spreadsheet_preview(path: &str, content_type: Option<&str>) -> Result<Vec<Vec<String>>, AppError> {
    read_spreadsheet_preview_with(path, content_type, PREVIEW_MAX_BYTES, PREVIEW_MAX_ROWS)
}

fn read_spreadsheet_preview_with(
    path: &str,
    content_type: Option<&str>,
    max_bytes: u64,
    max_rows: usize,
) -> Result<Vec<Vec<String>>, AppError> {
    let is_csv = match content_type {
        Some("text/csv") => true,
        Some("application/vnd.ms-excel")
        | Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet") => false,
        other => {
            return Err(AppError::InvalidInput(format!("anexo sem prévia de planilha (tipo {other:?})")));
        }
    };
    let size = std::fs::metadata(path)
        .map_err(|e| AppError::InvalidInput(format!("anexo ilegível: {e}")))?
        .len();
    if size > max_bytes {
        return Err(AppError::InvalidInput(format!(
            "arquivo grande demais para a prévia ({size} bytes, teto {max_bytes})"
        )));
    }
    let mut rows = if is_csv { read_csv_rows(path, max_rows)? } else { read_xlsx_rows(path)? };
    rows.truncate(max_rows);
    Ok(rows)
}

/// CSV de anexo como texto: sem cabeçalho, linhas de tamanho variável, separador `,`, `;` ou tab
/// pelo que mais aparece na primeira linha (CSV brasileiro costuma usar `;`). Bytes fora de UTF-8
/// viram U+FFFD em vez de recusar o arquivo.
fn read_csv_rows(path: &str, max_rows: usize) -> Result<Vec<Vec<String>>, AppError> {
    let bytes = std::fs::read(path).map_err(|e| AppError::InvalidInput(format!("anexo ilegível: {e}")))?;
    let first_line = bytes.split(|b| *b == b'\n').next().unwrap_or_default();
    // `max_by_key` devolve o último empate: a vírgula ganha quando nada aparece.
    let delimiter = [b';', b'\t', b',']
        .into_iter()
        .max_by_key(|d| first_line.iter().filter(|b| *b == d).count())
        .unwrap_or(b',');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(bytes.as_slice());
    reader
        .byte_records()
        .take(max_rows)
        .map(|record| {
            let record = record.map_err(|e| AppError::InvalidInput(format!("CSV ilegível: {e}")))?;
            Ok(record.iter().map(|cell| String::from_utf8_lossy(cell).into_owned()).collect())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(lines: &[&[&str]]) -> Vec<Vec<String>> {
        lines.iter().map(|l| l.iter().map(|c| c.to_string()).collect()).collect()
    }

    /// Monta à mão um xlsx mínimo (zip de partes XML), sem crate de escrita de planilha.
    fn minimal_xlsx(sheet_xml: &str) -> Vec<u8> {
        use std::io::Write;
        const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
        const PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
        let head = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
        let parts: Vec<(&str, String)> = vec![
            (
                "[Content_Types].xml",
                format!(
                    r#"{head}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#
                ),
            ),
            (
                "_rels/.rels",
                format!(
                    r#"{head}<Relationships xmlns="{PKG_REL}"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
                ),
            ),
            (
                "xl/workbook.xml",
                format!(
                    r#"{head}<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="Negociação" sheetId="1" r:id="rId1"/></sheets></workbook>"#
                ),
            ),
            (
                "xl/_rels/workbook.xml.rels",
                format!(
                    r#"{head}<Relationships xmlns="{PKG_REL}"><Relationship Id="rId1" Type="{REL}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="{REL}/styles" Target="styles.xml"/></Relationships>"#
                ),
            ),
            // Estilo 0 = geral; estilo 1 = formato de data (numFmtId 14).
            (
                "xl/styles.xml",
                format!(
                    r#"{head}<styleSheet xmlns="{MAIN}"><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14" applyNumberFormat="1"/></cellXfs></styleSheet>"#
                ),
            ),
            ("xl/worksheets/sheet1.xml", format!(r#"{head}<worksheet xmlns="{MAIN}"><sheetData>{sheet_xml}</sheetData></worksheet>"#)),
        ];
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buf);
        for (name, xml) in &parts {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated))
                .unwrap();
            writer.write_all(xml.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        buf.into_inner()
    }

    /// Teste de caracterização de `read_xlsx_rows`: texto, inteiro, decimal,
    /// booleano, data (`dd/mm/aaaa`) e célula vazia. Fixa o comportamento da
    /// leitura (calamine + zip) para que uma troca de versão dessas crates não
    /// passe despercebida; a planilha real da B3 só roda sob `ANCHOR_B3_SAMPLE`.
    #[test]
    fn read_xlsx_rows_reads_text_numbers_bool_and_date() {
        let sheet = concat!(
            r#"<row r="1"><c r="A1" t="inlineStr"><is><t>PETR4</t></is></c><c r="B1"><v>100</v></c><c r="C1"><v>12.5</v></c>"#,
            r#"<c r="D1" t="b"><v>1</v></c><c r="E1" s="1"><v>45292</v></c></row>"#,
            r#"<row r="2"><c r="A2" t="inlineStr"><is><t>só texto</t></is></c></row>"#,
        );
        let path = std::env::temp_dir().join(format!(
            "anchor_xlsx_test_{}_{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::write(&path, minimal_xlsx(sheet)).unwrap();

        let rows = read_xlsx_rows(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        let rows = rows.expect("xlsx mínimo abre");

        assert_eq!(
            rows,
            vec![
                vec!["PETR4", "100", "12.5", "true", "01/01/2024"],
                vec!["só texto", "", "", "", ""],
            ]
        );
    }

    #[test]
    fn read_xlsx_rows_rejects_a_file_that_is_not_a_spreadsheet() {
        let path = std::env::temp_dir().join(format!("anchor_not_xlsx_{}.xlsx", std::process::id()));
        std::fs::write(&path, b"isto nao e um zip").unwrap();
        let result = read_xlsx_rows(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert!(matches!(result, Err(AppError::InvalidInput(m)) if m.contains("planilha ilegível")));
    }

    /// `.xls` passa pelo leitor CFB/OLE2 do calamine (RUSTSEC-2026-0436: pânico e OOM em arquivo
    /// malformado). Um arquivo só com a assinatura OLE2 e lixo (cabeçalho truncado) tem de virar
    /// erro tratado, não pânico. Não reproduz o aviso (o OOM exige um arquivo sob medida e
    /// derrubaria o runner): a correção é provada pela versão no `Cargo.lock`.
    #[test]
    fn read_xlsx_rows_rejects_a_truncated_ole2_file_without_panicking() {
        const OLE2_SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        let mut bytes = OLE2_SIGNATURE.to_vec();
        bytes.extend_from_slice(&[0xFF; 100]);
        let path = std::env::temp_dir().join(format!("anchor_truncated_ole2_{}.xls", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();
        let result = std::panic::catch_unwind(|| read_xlsx_rows(path.to_str().unwrap()));
        std::fs::remove_file(&path).ok();
        let result = result.expect("a leitura não pode entrar em pânico");
        assert!(matches!(result, Err(AppError::InvalidInput(m)) if m.contains("planilha ilegível")));
    }

    #[test]
    fn read_xlsx_rows_rejects_an_empty_file() {
        let path = std::env::temp_dir().join(format!("anchor_empty_{}.xlsx", std::process::id()));
        std::fs::write(&path, b"").unwrap();
        let result = read_xlsx_rows(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert!(matches!(result, Err(AppError::InvalidInput(m)) if m.contains("planilha ilegível")));
    }

    // --- Prévia de anexo de planilha (substitui o `xlsx` do npm) -----------------------------

    const CT_CSV: Option<&str> = Some("text/csv");
    const CT_XLS: Option<&str> = Some("application/vnd.ms-excel");
    const CT_XLSX: Option<&str> = Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");

    /// Grava `bytes` num arquivo temporário com a extensão dada, roda `f` e apaga o arquivo.
    fn with_temp_file<T>(ext: &str, bytes: &[u8], f: impl FnOnce(&str) -> T) -> T {
        let path = std::env::temp_dir().join(format!(
            "anchor_preview_{}_{}.{ext}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::write(&path, bytes).unwrap();
        let out = f(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        out
    }

    #[test]
    fn preview_csv_keeps_ragged_rows_accents_and_quoted_commas() {
        let csv = "ativo,valor\n\"ação, B\",1\nso uma coluna\n";
        let got = with_temp_file("csv", csv.as_bytes(), |p| read_spreadsheet_preview(p, CT_CSV)).unwrap();
        assert_eq!(got, rows(&[&["ativo", "valor"], &["ação, B", "1"], &["so uma coluna"]]));
    }

    #[test]
    fn preview_csv_detects_a_semicolon_separator() {
        let csv = "ativo;valor\nPETR4;12,5\n";
        let got = with_temp_file("csv", csv.as_bytes(), |p| read_spreadsheet_preview(p, CT_CSV)).unwrap();
        assert_eq!(got, rows(&[&["ativo", "valor"], &["PETR4", "12,5"]]));
    }

    #[test]
    fn preview_csv_empty_file_is_an_empty_table() {
        let got = with_temp_file("csv", b"", |p| read_spreadsheet_preview(p, CT_CSV)).unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn preview_xlsx_reads_the_first_sheet_as_text() {
        let sheet = concat!(
            r#"<row r="1"><c r="A1" t="inlineStr"><is><t>PETR4</t></is></c><c r="B1"><v>100</v></c>"#,
            r#"<c r="C1" s="1"><v>45292</v></c></row>"#,
        );
        let bytes = minimal_xlsx(sheet);
        let got = with_temp_file("xlsx", &bytes, |p| read_spreadsheet_preview(p, CT_XLSX)).unwrap();
        assert_eq!(got, rows(&[&["PETR4", "100", "01/01/2024"]]));
    }

    #[test]
    fn preview_xls_content_type_is_read_as_a_workbook_not_csv() {
        let got = with_temp_file("xls", b"isto nao e uma planilha", |p| read_spreadsheet_preview(p, CT_XLS));
        assert!(matches!(got, Err(AppError::InvalidInput(m)) if m.contains("planilha ilegível")));
    }

    #[test]
    fn preview_rejects_an_unknown_or_missing_content_type() {
        for ct in [None, Some("application/pdf"), Some("image/png")] {
            let got = with_temp_file("csv", b"a,b\n", |p| read_spreadsheet_preview(p, ct));
            assert!(matches!(got, Err(AppError::InvalidInput(_))), "content_type {ct:?} devia ser recusado");
        }
    }

    #[test]
    fn preview_rejects_a_missing_file() {
        let got = read_spreadsheet_preview("/nao/existe/anchor.csv", CT_CSV);
        assert!(matches!(got, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn preview_rejects_a_file_above_the_size_cap() {
        let got = with_temp_file("csv", b"a,b\n1,2\n", |p| read_spreadsheet_preview_with(p, CT_CSV, 4, 1000));
        assert!(matches!(got, Err(AppError::InvalidInput(m)) if m.contains("grande")));
    }

    #[test]
    fn preview_truncates_rows_above_the_row_cap() {
        let csv = "1\n2\n3\n4\n5\n";
        let got = with_temp_file("csv", csv.as_bytes(), |p| read_spreadsheet_preview_with(p, CT_CSV, 1024, 3)).unwrap();
        assert_eq!(got, rows(&[&["1"], &["2"], &["3"]]));
    }

    #[test]
    fn preview_row_cap_also_applies_to_workbooks() {
        let sheet = concat!(
            r#"<row r="1"><c r="A1"><v>1</v></c></row><row r="2"><c r="A2"><v>2</v></c></row>"#,
            r#"<row r="3"><c r="A3"><v>3</v></c></row>"#,
        );
        let bytes = minimal_xlsx(sheet);
        let got = with_temp_file("xlsx", &bytes, |p| read_spreadsheet_preview_with(p, CT_XLSX, 1_000_000, 2)).unwrap();
        assert_eq!(got, rows(&[&["1"], &["2"]]));
    }
}

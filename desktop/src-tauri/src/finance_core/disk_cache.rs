// Cache em disco dos arquivos grandes que as fontes públicas publicam por ano
// (zips da CVM, lista de tickers da SEC). O diretório é definido uma vez na
// inicialização (`init`, com o `app_cache_dir()` do SO); sem ele, e nos testes,
// cai em `temp_dir()/anchor-cache`. Arquivos antigos podem expirar (`max_age`);
// se o novo download falhar e existir cópia antiga, ela é usada.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use crate::error::AppError;
use crate::finance_core::http;

static DIR: OnceLock<PathBuf> = OnceLock::new();
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// Define o diretório de cache (a primeira chamada vale; as seguintes são ignoradas).
pub fn init(dir: PathBuf) {
    let _ = DIR.set(dir);
}

pub fn dir() -> PathBuf {
    DIR.get().cloned().unwrap_or_else(|| std::env::temp_dir().join("anchor-cache"))
}

/// Precisa baixar? `age` é a idade do arquivo no disco (`None` = não existe);
/// `max_age` `None` significa que o arquivo nunca expira.
pub fn needs_download(age: Option<Duration>, max_age: Option<Duration>) -> bool {
    match (age, max_age) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(age), Some(max)) => age >= max,
    }
}

fn age_of(path: &Path) -> Option<Duration> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(SystemTime::now().duration_since(modified).unwrap_or_default())
}

/// Grava em arquivo temporário e renomeia, pra nunca deixar um zip pela metade.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("part{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Caminho local de `relative_path` dentro do cache, baixando de `url` se
/// faltar ou tiver expirado. `source` só rotula as mensagens de erro.
pub async fn get_or_download(
    source: &str,
    relative_path: &str,
    url: &str,
    max_age: Option<Duration>,
) -> Result<PathBuf, AppError> {
    let path = dir().join(relative_path);
    let age = age_of(&path);
    if !needs_download(age, max_age) {
        return Ok(path);
    }
    match http::get_bytes(source, url, DOWNLOAD_TIMEOUT).await {
        Ok(bytes) => {
            write_atomic(&path, &bytes)?;
            Ok(path)
        }
        // Cópia antiga é melhor que nada quando a fonte está fora do ar.
        Err(AppError::ProviderUnavailable(msg)) if age.is_some() => {
            eprintln!("{source}: using the cached copy after download failure: {msg}");
            Ok(path)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_are_always_downloaded() {
        assert!(needs_download(None, None));
        assert!(needs_download(None, Some(Duration::from_secs(10))));
    }

    #[test]
    fn files_without_max_age_never_expire() {
        assert!(!needs_download(Some(Duration::from_secs(10 * 365 * 86400)), None));
    }

    #[test]
    fn files_expire_at_max_age() {
        let max = Some(Duration::from_secs(100));
        assert!(!needs_download(Some(Duration::from_secs(99)), max));
        assert!(needs_download(Some(Duration::from_secs(100)), max));
    }

    #[test]
    fn atomic_write_creates_parents_and_leaves_no_partial_file() {
        let dir = std::env::temp_dir().join(format!("anchor-disk-cache-test-{}", std::process::id()));
        let path = dir.join("nested").join("file.zip");
        write_atomic(&path, b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        write_atomic(&path, b"world").unwrap(); // sobrescreve
        assert_eq!(std::fs::read(&path).unwrap(), b"world");
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".part"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

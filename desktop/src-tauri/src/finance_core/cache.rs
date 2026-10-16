// Cache em memória com TTL para dados que não são gravados no `anchor.db`
// (ex.: série da Selic e curva DI, calculadas a cada abertura da Home). O que
// é gravado já tem `fetched_at`/`source` na própria tabela. Mesmo contrato do
// `single_row_cache` do Lume: fonte caiu e há dado antigo → devolve o antigo
// marcado `stale`; sem dado antigo → propaga o erro. `NoData` nunca vira stale
// (é resposta da fonte, não falha).

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct Cached<V> {
    pub value: V,
    /// `true` quando veio do cache sem consultar a fonte.
    pub cached: bool,
    /// `true` quando a fonte falhou e o valor é antigo.
    pub stale: bool,
    pub fetched_at: DateTime<Utc>,
}

struct Entry<V> {
    stored: Instant,
    fetched_at: DateTime<Utc>,
    value: V,
}

pub struct TtlCache<K, V> {
    entries: Mutex<HashMap<K, Entry<V>>>,
}

impl<K: Hash + Eq + Clone, V: Clone> TtlCache<K, V> {
    pub fn new() -> Self {
        TtlCache { entries: Mutex::new(HashMap::new()) }
    }

    pub async fn get_or_refresh<F, Fut>(
        &self,
        key: K,
        ttl: Duration,
        fetch: F,
    ) -> Result<Cached<V>, AppError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, AppError>>,
    {
        let existing = {
            let entries = self.entries.lock().unwrap();
            entries.get(&key).map(|e| (e.stored, e.fetched_at, e.value.clone()))
        };
        if let Some((stored, fetched_at, value)) = &existing {
            if stored.elapsed() < ttl {
                return Ok(Cached { value: value.clone(), cached: true, stale: false, fetched_at: *fetched_at });
            }
        }
        match fetch().await {
            Ok(value) => {
                let fetched_at = Utc::now();
                self.entries.lock().unwrap().insert(
                    key,
                    Entry { stored: Instant::now(), fetched_at, value: value.clone() },
                );
                Ok(Cached { value, cached: false, stale: false, fetched_at })
            }
            Err(AppError::ProviderUnavailable(msg)) => match existing {
                Some((_, fetched_at, value)) => {
                    eprintln!("serving stale data after provider failure: {msg}");
                    Ok(Cached { value, cached: true, stale: true, fetched_at })
                }
                None => Err(AppError::ProviderUnavailable(msg)),
            },
            Err(other) => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    const LONG: Duration = Duration::from_secs(3600);

    #[tokio::test]
    async fn second_call_inside_ttl_is_a_cache_hit() {
        let cache = TtlCache::<&str, i32>::new();
        let calls = AtomicU32::new(0);
        let fetch = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(7)
        };
        let first = cache.get_or_refresh("k", LONG, fetch).await.unwrap();
        let second = cache.get_or_refresh("k", LONG, fetch).await.unwrap();
        assert!(!first.cached && second.cached && !second.stale);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(first.fetched_at, second.fetched_at);
    }

    #[tokio::test]
    async fn expired_entry_is_refetched() {
        let cache = TtlCache::<&str, i32>::new();
        cache.get_or_refresh("k", LONG, || async { Ok(1) }).await.unwrap();
        let again = cache.get_or_refresh("k", Duration::ZERO, || async { Ok(2) }).await.unwrap();
        assert_eq!(again.value, 2);
        assert!(!again.cached);
    }

    #[tokio::test]
    async fn provider_outage_serves_the_old_value_as_stale() {
        let cache = TtlCache::<&str, i32>::new();
        cache.get_or_refresh("k", LONG, || async { Ok(1) }).await.unwrap();
        let got = cache
            .get_or_refresh("k", Duration::ZERO, || async {
                Err(AppError::ProviderUnavailable("down".into()))
            })
            .await
            .unwrap();
        assert_eq!(got.value, 1);
        assert!(got.stale && got.cached);
    }

    #[tokio::test]
    async fn outage_without_old_value_is_an_error() {
        let cache = TtlCache::<&str, i32>::new();
        let err = cache
            .get_or_refresh("k", LONG, || async { Err(AppError::ProviderUnavailable("down".into())) })
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::ProviderUnavailable(_)));
    }

    #[tokio::test]
    async fn no_data_is_never_stale() {
        let cache = TtlCache::<&str, i32>::new();
        cache.get_or_refresh("k", LONG, || async { Ok(1) }).await.unwrap();
        let err = cache
            .get_or_refresh("k", Duration::ZERO, || async { Err(AppError::NoData("gone".into())) })
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NoData(_)));
    }
}

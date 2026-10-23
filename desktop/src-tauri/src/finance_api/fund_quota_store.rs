// Valor da cota dos fundos lido do informe diário da CVM e gravado em `fund_quota_values` (`source = 'cvm'`). Só vão à rede
// os meses que faltam: o mês corrente e o anterior sempre (ainda recebem dias e retificações), os fechados só se o fundo
// não tem nenhuma cota da CVM naquele mês. Uma cota lançada à mão nunca é sobrescrita. A CVM oscila e o zip do mês tem
// ~7,5 MB: depois de uma falha a rede fica de molho por `COOLDOWN_SECS`, e uma busca bem-sucedida dispensa outra por
// `THROTTLE_SECS` (abrir a aba Taxes várias vezes não relê o zip), salvo no botão "Atualizar da CVM" (`force`).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Serialize;

use crate::entity::fund_quota_value;
use crate::error::AppError;
use crate::finance_core::cvm_inf_diario::{self, Extract, Target, ZipFetcher};

pub const SOURCE_CVM: &str = "cvm";
pub const SOURCE_MANUAL: &str = "manual";

/// Linhas por `INSERT`: 5 colunas × 500 fica bem abaixo do limite de variáveis do SQLite.
const INSERT_CHUNK: usize = 500;
/// Depois de uma falha da CVM, não tenta de novo por 10 minutos.
const COOLDOWN_SECS: i64 = 600;
/// Depois de uma busca bem-sucedida, a abertura da aba não busca de novo por 1 hora.
const THROTTLE_SECS: i64 = 3600;

/// Um fundo a buscar: o CNPJ (só dígitos), a subclasse escolhida e a data desde a qual as cotas interessam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundTarget {
    pub asset_id: i32,
    pub cnpj: String,
    pub subclass: Option<String>,
    pub since: NaiveDate,
}

/// Como está a leitura da CVM de um fundo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CvmStatus {
    /// O fundo não tem CNPJ cadastrado.
    NoCnpj,
    /// Ainda não buscou (rede desligada, em espera ou nada rodou desde que o app abriu) e não há cota da CVM gravada.
    Pending,
    Ok,
    /// O informe não traz o CNPJ, ou traz sem cota usável para a subclasse escolhida.
    NotFound,
    /// O CNPJ só tem subclasses no informe: o usuário escolhe uma.
    NeedsSubclass { options: Vec<String> },
    Error { message: String },
}

/// O que se lembra entre uma busca e outra enquanto o app está aberto. Em produção é o `SESSION`; os testes criam o seu.
pub struct Session {
    last_failure: AtomicI64,
    last_success: AtomicI64,
    /// `(ativo, ano, mês)` de meses fechados já lidos, mesmo que o fundo não tivesse cota neles.
    checked: Mutex<BTreeSet<(i32, i32, u32)>>,
    outcomes: Mutex<BTreeMap<i32, CvmStatus>>,
}

pub static SESSION: Session = Session::new();

impl Session {
    pub const fn new() -> Self {
        Session {
            last_failure: AtomicI64::new(0),
            last_success: AtomicI64::new(0),
            checked: Mutex::new(BTreeSet::new()),
            outcomes: Mutex::new(BTreeMap::new()),
        }
    }

    /// O último resultado da busca do fundo, se houve.
    pub fn outcome(&self, asset_id: i32) -> Option<CvmStatus> {
        self.outcomes.lock().expect("outcomes lock").get(&asset_id).cloned()
    }

    /// O CNPJ ou a subclasse do fundo mudou: esquece o que se sabia dele e libera a próxima busca.
    pub fn forget(&self, asset_id: i32) {
        self.checked.lock().expect("checked lock").retain(|(id, _, _)| *id != asset_id);
        self.outcomes.lock().expect("outcomes lock").remove(&asset_id);
        self.last_success.store(0, Ordering::Relaxed);
    }
}

/// Os meses `(ano, mês)` a ler para cobrir `since..=today`, dadas as datas de cota da CVM já gravadas do fundo.
pub fn plan_months(since: NaiveDate, today: NaiveDate, stored_cvm_dates: &[NaiveDate]) -> Vec<(i32, u32)> {
    cvm_inf_diario::months(since, today)
        .into_iter()
        .filter(|&month| {
            cvm_inf_diario::is_recent(month, today)
                || !stored_cvm_dates.iter().any(|d| (d.year(), d.month()) == month)
        })
        .collect()
}

/// Dentro da janela de espera depois de uma falha? Os valores são segundos unix, 0 = nunca.
fn cooling_down(last_failure: i64, now: i64) -> bool {
    last_failure > 0 && now - last_failure < COOLDOWN_SECS
}

/// Houve uma busca bem-sucedida há menos de `THROTTLE_SECS`?
fn throttled(last_success: i64, now: i64) -> bool {
    last_success > 0 && now - last_success < THROTTLE_SECS
}

async fn stored_cvm_dates(db: &DatabaseConnection, asset_ids: &[i32]) -> Result<HashMap<i32, Vec<NaiveDate>>, AppError> {
    let rows = fund_quota_value::Entity::find()
        .filter(fund_quota_value::Column::AssetId.is_in(asset_ids.to_vec()))
        .filter(fund_quota_value::Column::Source.eq(SOURCE_CVM))
        .all(db)
        .await?;
    let mut out: HashMap<i32, Vec<NaiveDate>> = HashMap::new();
    for row in rows {
        if let Ok(date) = NaiveDate::parse_from_str(&row.quota_date, "%Y-%m-%d") {
            out.entry(row.asset_id).or_default().push(date);
        }
    }
    Ok(out)
}

/// Grava as cotas da CVM do ativo, pulando as datas em que o usuário lançou a sua.
async fn upsert(
    db: &DatabaseConnection,
    asset_id: i32,
    quotas: &BTreeMap<NaiveDate, f64>,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let manual: HashSet<String> = fund_quota_value::Entity::find()
        .filter(fund_quota_value::Column::AssetId.eq(asset_id))
        .filter(fund_quota_value::Column::Source.eq(SOURCE_MANUAL))
        .all(db)
        .await?
        .into_iter()
        .map(|row| row.quota_date)
        .collect();
    let created_at = now.to_rfc3339();
    let rows: Vec<(String, f64)> = quotas
        .iter()
        .map(|(date, value)| (date.format("%Y-%m-%d").to_string(), *value))
        .filter(|(date, _)| !manual.contains(date))
        .collect();
    for chunk in rows.chunks(INSERT_CHUNK) {
        let models = chunk.iter().map(|(date, value)| fund_quota_value::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            asset_id: Set(asset_id),
            quota_date: Set(date.clone()),
            value: Set(*value),
            source: Set(SOURCE_CVM.to_string()),
            created_at: Set(created_at.clone()),
        });
        fund_quota_value::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([fund_quota_value::Column::AssetId, fund_quota_value::Column::QuotaDate])
                    .update_column(fund_quota_value::Column::Value)
                    .to_owned(),
            )
            .exec(db)
            .await?;
    }
    Ok(())
}

/// O estado do fundo depois de uma busca: `got_quotas` se esta rodada trouxe cota, `stored` se já havia cota da CVM gravada.
fn status_of(
    target: &FundTarget,
    got_quotas: bool,
    stored: bool,
    subclasses: Option<&BTreeSet<String>>,
) -> CvmStatus {
    if got_quotas {
        return CvmStatus::Ok;
    }
    if target.subclass.is_none() {
        if let Some(options) = subclasses.filter(|o| !o.is_empty()) {
            return CvmStatus::NeedsSubclass { options: options.iter().cloned().collect() };
        }
    }
    if stored {
        CvmStatus::Ok
    } else {
        CvmStatus::NotFound
    }
}

/// Completa as cotas da CVM dos `targets`. Devolve o estado de cada fundo; em falha da CVM entra em espera e devolve o erro
/// (quem chama registra e segue com o que está gravado). `force` ignora só o intervalo entre buscas, nunca a espera pós-falha.
pub async fn refresh(
    db: &DatabaseConnection,
    session: &Session,
    fetcher: &dyn ZipFetcher,
    targets: &[FundTarget],
    today: NaiveDate,
    force: bool,
) -> Result<HashMap<i32, CvmStatus>, AppError> {
    if targets.is_empty() {
        return Ok(HashMap::new());
    }
    let now = Utc::now();
    let now_secs = now.timestamp();
    if cooling_down(session.last_failure.load(Ordering::Relaxed), now_secs) {
        return Err(AppError::ProviderUnavailable("CVM informe diário: waiting after a recent failure".to_string()));
    }
    if !force && throttled(session.last_success.load(Ordering::Relaxed), now_secs) {
        return Ok(HashMap::new());
    }

    let ids: Vec<i32> = targets.iter().map(|t| t.asset_id).collect();
    let stored = stored_cvm_dates(db, &ids).await?;

    // Mês -> índices dos fundos que precisam dele.
    let mut wanted: BTreeMap<(i32, u32), Vec<usize>> = BTreeMap::new();
    {
        let checked = session.checked.lock().expect("checked lock");
        for (i, target) in targets.iter().enumerate() {
            let dates = stored.get(&target.asset_id).map(Vec::as_slice).unwrap_or(&[]);
            for month in plan_months(target.since, today, dates) {
                if !cvm_inf_diario::is_recent(month, today) && checked.contains(&(target.asset_id, month.0, month.1)) {
                    continue;
                }
                wanted.entry(month).or_default().push(i);
            }
        }
    }

    let mut got = vec![false; targets.len()];
    let mut subclasses: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (month, indices) in wanted {
        let month_targets: Vec<Target> = indices
            .iter()
            .map(|&i| Target { cnpj: targets[i].cnpj.clone(), subclass: targets[i].subclass.clone() })
            .collect();
        let extract: Extract = match cvm_inf_diario::extract_month(fetcher, month, month_targets).await {
            Ok(extract) => extract,
            // O mês corrente ainda não foi publicado (a CVM abre o arquivo depois do primeiro dia útil).
            Err(AppError::NoData(_)) if month == (today.year(), today.month()) => continue,
            Err(err) => {
                session.last_failure.store(now_secs, Ordering::Relaxed);
                let message = err.to_string();
                let mut outcomes = session.outcomes.lock().expect("outcomes lock");
                for &i in &indices {
                    outcomes.insert(targets[i].asset_id, CvmStatus::Error { message: message.clone() });
                }
                return Err(err);
            }
        };
        for (cnpj, options) in extract.subclasses {
            subclasses.entry(cnpj).or_default().extend(options);
        }
        for (slot, &i) in indices.iter().enumerate() {
            let quotas: BTreeMap<NaiveDate, f64> = extract.quotas[slot].iter().copied().collect();
            if !quotas.is_empty() {
                got[i] = true;
                upsert(db, targets[i].asset_id, &quotas, now).await?;
            }
            if !cvm_inf_diario::is_recent(month, today) {
                session.checked.lock().expect("checked lock").insert((targets[i].asset_id, month.0, month.1));
            }
        }
    }

    let mut statuses = HashMap::new();
    for (i, target) in targets.iter().enumerate() {
        let has_stored = stored.get(&target.asset_id).is_some_and(|d| !d.is_empty());
        statuses.insert(target.asset_id, status_of(target, got[i], has_stored, subclasses.get(&target.cnpj)));
    }
    session.outcomes.lock().expect("outcomes lock").extend(statuses.clone());
    session.last_success.store(now_secs, Ordering::Relaxed);
    Ok(statuses)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn nothing_stored_reads_every_month_from_the_start() {
        let plan = plan_months(d(2026, 5, 10), d(2026, 10, 22), &[]);
        assert_eq!(plan, vec![(2026, 5), (2026, 6), (2026, 7), (2026, 8), (2026, 9), (2026, 10)]);
    }

    #[test]
    fn closed_months_with_stored_quotas_are_skipped_but_the_recent_two_always_read() {
        let stored = [d(2026, 5, 29), d(2026, 6, 30), d(2026, 7, 31), d(2026, 8, 31), d(2026, 9, 30), d(2026, 10, 20)];
        let plan = plan_months(d(2026, 5, 10), d(2026, 10, 22), &stored);
        assert_eq!(plan, vec![(2026, 9), (2026, 10)]);
    }

    #[test]
    fn a_closed_month_with_a_hole_is_read_again() {
        let stored = [d(2026, 5, 29), d(2026, 7, 31), d(2026, 8, 31), d(2026, 9, 30)];
        let plan = plan_months(d(2026, 5, 10), d(2026, 10, 22), &stored);
        assert_eq!(plan, vec![(2026, 6), (2026, 9), (2026, 10)]);
    }

    #[test]
    fn the_window_never_goes_before_the_first_published_month() {
        let plan = plan_months(d(2019, 1, 1), d(2021, 2, 10), &[]);
        assert_eq!(plan, vec![(2021, 1), (2021, 2)]);
    }

    #[test]
    fn the_network_rests_after_a_failure_and_between_successes() {
        assert!(!cooling_down(0, 1_000_000), "nunca falhou");
        assert!(cooling_down(1_000_000, 1_000_300));
        assert!(!cooling_down(1_000_000, 1_000_600));
        assert!(!throttled(0, 1_000_000), "nunca buscou");
        assert!(throttled(1_000_000, 1_001_000));
        assert!(!throttled(1_000_000, 1_003_600));
    }

    fn target(subclass: Option<&str>) -> FundTarget {
        FundTarget { asset_id: 1, cnpj: "00017024000153".into(), subclass: subclass.map(str::to_string), since: d(2026, 1, 1) }
    }

    #[test]
    fn status_prefers_fresh_quotas_then_asks_for_a_subclass_then_falls_back_to_stored() {
        let options: BTreeSet<String> = ["A".to_string(), "B".to_string()].into();
        assert_eq!(status_of(&target(None), true, false, Some(&options)), CvmStatus::Ok);
        assert_eq!(
            status_of(&target(None), false, false, Some(&options)),
            CvmStatus::NeedsSubclass { options: vec!["A".into(), "B".into()] }
        );
        // Com subclasse escolhida e sem cota nela, listar as opções não ajuda: a escolha é que não existe.
        assert_eq!(status_of(&target(Some("Z")), false, false, Some(&options)), CvmStatus::NotFound);
        assert_eq!(status_of(&target(None), false, true, None), CvmStatus::Ok);
        assert_eq!(status_of(&target(None), false, false, None), CvmStatus::NotFound);
    }
}

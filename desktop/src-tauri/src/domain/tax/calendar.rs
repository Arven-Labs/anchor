use chrono::{Datelike, Duration, NaiveDate, Weekday};

// Vencimento do DARF de renda variável: último dia útil do mês seguinte ao
// da apuração. "Dia útil" aqui = dia em que banco abre: fora fins de semana,
// feriados nacionais e os dias sem expediente bancário (Carnaval, Corpus
// Christi, 31/12). Feriados estaduais/municipais ficam de fora — o motor não
// sabe onde o usuário mora; na dúvida, vence um dia útil "antes" em vez de
// depois, nunca ao contrário.

/// Domingo de Páscoa (algoritmo anônimo gregoriano — Meeus/Jones/Butcher).
fn easter(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("valid Easter date")
}

fn is_bank_holiday(date: NaiveDate) -> bool {
    let fixed = [
        (1, 1),   // Confraternização Universal
        (4, 21),  // Tiradentes
        (5, 1),   // Dia do Trabalho
        (9, 7),   // Independência
        (10, 12), // Nossa Senhora Aparecida
        (11, 2),  // Finados
        (11, 15), // Proclamação da República
        (11, 20), // Consciência Negra (Lei 14.759/2023)
        (12, 25), // Natal
        (12, 31), // sem expediente bancário
    ];
    if fixed.contains(&(date.month(), date.day())) {
        return true;
    }
    let e = easter(date.year());
    let movable = [
        e - Duration::days(48), // segunda de Carnaval
        e - Duration::days(47), // terça de Carnaval
        e - Duration::days(2),  // Sexta-feira Santa
        e + Duration::days(60), // Corpus Christi
    ];
    movable.contains(&date)
}

pub fn is_business_day(date: NaiveDate) -> bool {
    !matches!(date.weekday(), Weekday::Sat | Weekday::Sun) && !is_bank_holiday(date)
}

/// Último dia útil de (`year`, `month`).
pub fn last_business_day(year: i32, month: u32) -> NaiveDate {
    let (next_y, next_m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let mut day = NaiveDate::from_ymd_opt(next_y, next_m, 1).expect("valid date") - Duration::days(1);
    while !is_business_day(day) {
        day -= Duration::days(1);
    }
    day
}

/// Último dia útil do mês seguinte a (`year`, `month`).
pub fn darf_due_date(year: i32, month: u32) -> NaiveDate {
    let (next_y, next_m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let (after_y, after_m) = if next_m == 12 { (next_y + 1, 1) } else { (next_y, next_m + 1) };
    let mut day = NaiveDate::from_ymd_opt(after_y, after_m, 1).expect("valid date") - Duration::days(1);
    while !is_business_day(day) {
        day -= Duration::days(1);
    }
    day
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn easter_known_years() {
        assert_eq!(easter(2024), d("2024-03-31"));
        assert_eq!(easter(2025), d("2025-04-20"));
        assert_eq!(easter(2026), d("2026-04-05"));
    }

    #[test]
    fn receita_example_february_gain_due_end_of_march() {
        // Exemplo da página "Cálculo e Pagamento": venda em fev/2024 → paga
        // até o último dia útil de março. 31/03/2024 é domingo, 29/03 é
        // Sexta-feira Santa → 28/03.
        assert_eq!(darf_due_date(2024, 2), d("2024-03-28"));
    }

    #[test]
    fn skips_weekend() {
        // Fevereiro/2026 termina num sábado (28/02) → sexta 27/02.
        assert_eq!(darf_due_date(2026, 1), d("2026-02-27"));
    }

    #[test]
    fn november_gain_due_december_30() {
        // 31/12 não tem expediente bancário.
        assert_eq!(darf_due_date(2025, 11), d("2025-12-30"));
    }

    #[test]
    fn december_gain_rolls_into_next_year() {
        // Janeiro/2027 termina num domingo (31/01) → sexta 29/01.
        assert_eq!(darf_due_date(2026, 12), d("2027-01-29"));
    }
}

// Calendário de dias úteis brasileiros, para contar os dias em que um título pós-fixado rende (252 ao ano).
// Só os feriados **nacionais**: os fixos e os que dependem da Páscoa (Carnaval, Sexta-feira Santa e
// Corpus Christi). Feriados municipais e estaduais não entram, porque o mercado financeiro segue o nacional.
use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// Domingo de Páscoa (calendário gregoriano, algoritmo de Meeus/Jones/Butcher).
pub fn easter(year: i32) -> NaiveDate {
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
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("Páscoa é sempre uma data válida")
}

/// Feriado nacional: fixo, ou móvel contado a partir da Páscoa. O Dia da Consciência Negra (20/11) é
/// nacional desde 2024 (Lei 14.759/2023).
pub fn is_national_holiday(date: NaiveDate) -> bool {
    let fixed = matches!(
        (date.month(), date.day()),
        (1, 1) | (4, 21) | (5, 1) | (9, 7) | (10, 12) | (11, 2) | (11, 15) | (12, 25)
    ) || (date.year() >= 2024 && date.month() == 11 && date.day() == 20);
    if fixed {
        return true;
    }
    let easter = easter(date.year());
    // Carnaval (segunda e terça), Sexta-feira Santa e Corpus Christi.
    [-48, -47, -2, 60].iter().any(|offset| date == easter + Duration::days(*offset))
}

pub fn is_business_day(date: NaiveDate) -> bool {
    !matches!(date.weekday(), Weekday::Sat | Weekday::Sun) && !is_national_holiday(date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn easter_matches_known_dates() {
        assert_eq!(easter(2024), d(2024, 3, 31));
        assert_eq!(easter(2025), d(2025, 4, 20));
        assert_eq!(easter(2026), d(2026, 4, 5));
    }

    #[test]
    fn the_movable_holidays_of_2025_follow_easter() {
        assert!(is_national_holiday(d(2025, 3, 3)), "segunda de Carnaval");
        assert!(is_national_holiday(d(2025, 3, 4)), "terça de Carnaval");
        assert!(is_national_holiday(d(2025, 4, 18)), "Sexta-feira Santa");
        assert!(is_national_holiday(d(2025, 6, 19)), "Corpus Christi");
        assert!(!is_national_holiday(d(2025, 3, 5)), "quarta de Cinzas não é feriado");
    }

    #[test]
    fn the_fixed_holidays_apply_and_black_awareness_day_starts_in_2024() {
        for (m, day) in [(1, 1), (4, 21), (5, 1), (9, 7), (10, 12), (11, 2), (11, 15), (12, 25)] {
            assert!(is_national_holiday(d(2025, m, day)), "{day}/{m}");
        }
        assert!(!is_national_holiday(d(2023, 11, 20)));
        assert!(is_national_holiday(d(2024, 11, 20)));
        assert!(is_national_holiday(d(2025, 11, 20)));
    }

    #[test]
    fn weekends_and_holidays_are_not_business_days() {
        assert!(!is_business_day(d(2025, 6, 14)), "sábado");
        assert!(!is_business_day(d(2025, 6, 15)), "domingo");
        assert!(!is_business_day(d(2025, 4, 21)), "Tiradentes numa segunda");
        assert!(is_business_day(d(2025, 6, 16)));
    }

    #[test]
    fn the_year_2025_has_the_252_business_days_of_the_annual_basis() {
        let mut count = 0;
        let mut day = d(2025, 1, 1);
        while day.year() == 2025 {
            if is_business_day(day) {
                count += 1;
            }
            day += Duration::days(1);
        }
        // 261 dias de semana menos 9 feriados que caem em dia útil (os de 7/9, 12/10, 2/11 e 15/11 caem no fim de semana).
        assert_eq!(count, 252);
    }
}

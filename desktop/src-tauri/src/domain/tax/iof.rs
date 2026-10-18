// IOF sobre o rendimento de títulos de renda fixa resgatados em menos de 30 dias
// (Decreto 6.306/2007, tabela regressiva): cobra uma parte do **rendimento**, que
// cai de 96% no 1º dia até 0% no 30º. É descontado antes do IR, que incide sobre o
// que sobra. A tabela é fixada por decreto, por isso mora aqui e não em `tax_rules`.
// Fonte secundária: confira no Planalto.

/// % do rendimento cobrado de IOF, do 1º ao 30º dia aplicado.
const IOF_PERCENT_BY_DAY: [u32; 30] = [
    96, 93, 90, 86, 83, 80, 76, 73, 70, 66, 63, 60, 56, 53, 50, 46, 43, 40, 36, 33, 30, 26, 23, 20, 16, 13, 10, 6, 3, 0,
];

/// Fração do rendimento cobrada de IOF para `days` dias aplicados. No mesmo dia
/// (`days <= 0`) vale a do 1º dia; a partir do 30º é zero.
pub fn iof_rate_for_days(days: i64) -> f64 {
    match days {
        d if d <= 1 => f64::from(IOF_PERCENT_BY_DAY[0]) / 100.0,
        d if d >= 30 => 0.0,
        d => f64::from(IOF_PERCENT_BY_DAY[(d - 1) as usize]) / 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_falls_from_96_percent_to_zero_at_thirty_days() {
        assert_eq!(iof_rate_for_days(0), 0.96, "no mesmo dia vale a do 1º");
        assert_eq!(iof_rate_for_days(1), 0.96);
        assert_eq!(iof_rate_for_days(10), 0.66);
        assert_eq!(iof_rate_for_days(20), 0.33);
        assert_eq!(iof_rate_for_days(29), 0.03);
        assert_eq!(iof_rate_for_days(30), 0.0);
        assert_eq!(iof_rate_for_days(400), 0.0);
    }

    #[test]
    fn the_table_never_rises() {
        for day in 1..=30 {
            assert!(iof_rate_for_days(day + 1) <= iof_rate_for_days(day), "dia {day}");
        }
    }
}

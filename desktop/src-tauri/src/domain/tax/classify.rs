// Classe de ativo do app → como a Receita trata a venda. Só classes B3 de
// mercado à vista entram no cálculo desta fatia; o resto vira aviso ou é
// ignorado (ver `TaxTreatment`).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaxTreatment {
    /// Ação comum (não unit): gaveta comum, elegível à isenção de vendas
    /// mensais até o limite.
    StockExemptEligible,
    /// Unit, ETF de ações ou BDR: gaveta comum, nunca isento.
    CommonNoExemption,
    /// FII/Fiagro: gaveta própria (sem distinção swing/day trade).
    Fii,
    /// Cripto — não calculado nesta fatia, só aviso acima do limite.
    CryptoWarning,
    /// Ativo no exterior (Lei 14.754/2023, apuração anual) — só aviso.
    ForeignWarning,
    /// Fora do escopo (renda fixa, imóvel, empresa não listada, `metal` —
    /// cotação COMEX, não ouro ativo financeiro da B3).
    Ignored,
}

/// Units da B3 (certificados de depósito de ações, ex. TAEE11/SANB11/
/// KLBN11) não são ações pra Receita — sem isenção (SC Cosit 145/2021,
/// reafirmado no Manual do ReVar). FII/ETF também terminam em 11, mas são
/// classes próprias no app, então só `acao_br` precisa dessa checagem.
pub fn is_unit(asset_class: &str, ticker: &str) -> bool {
    asset_class == "acao_br" && ticker.trim().to_uppercase().ends_with("11")
}

pub fn treatment(asset_class: &str, ticker: &str) -> TaxTreatment {
    match asset_class {
        "acao_br" if is_unit(asset_class, ticker) => TaxTreatment::CommonNoExemption,
        "acao_br" => TaxTreatment::StockExemptEligible,
        "etf_br" | "bdr" => TaxTreatment::CommonNoExemption,
        "fii" => TaxTreatment::Fii,
        "cripto" => TaxTreatment::CryptoWarning,
        "acao_internacional" | "reit" | "etf_us" => TaxTreatment::ForeignWarning,
        // ETF de renda fixa (ex.: AUPO11): IR retido na fonte, fora da apuração mensal.
        "etf_renda_fixa" => TaxTreatment::Ignored,
        _ => TaxTreatment::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_lose_the_exemption() {
        assert_eq!(treatment("acao_br", "TAEE11"), TaxTreatment::CommonNoExemption);
        assert_eq!(treatment("acao_br", "taee11"), TaxTreatment::CommonNoExemption);
        assert_eq!(treatment("acao_br", "TAEE4"), TaxTreatment::StockExemptEligible);
        assert_eq!(treatment("acao_br", "PETR4"), TaxTreatment::StockExemptEligible);
    }

    #[test]
    fn fii_and_etf_ending_in_11_follow_their_own_class() {
        assert_eq!(treatment("fii", "HGLG11"), TaxTreatment::Fii);
        assert_eq!(treatment("etf_br", "BOVA11"), TaxTreatment::CommonNoExemption);
        assert!(!is_unit("fii", "HGLG11"));
    }

    #[test]
    fn out_of_scope_classes() {
        assert_eq!(treatment("cripto", "BTC"), TaxTreatment::CryptoWarning);
        assert_eq!(treatment("reit", "O"), TaxTreatment::ForeignWarning);
        assert_eq!(treatment("metal", "XAU"), TaxTreatment::Ignored);
        assert_eq!(treatment("renda_fixa", "CDB"), TaxTreatment::Ignored);
        assert_eq!(treatment("etf_renda_fixa", "AUPO11"), TaxTreatment::Ignored, "o final 11 não o torna unit nem ETF comum");
    }
}

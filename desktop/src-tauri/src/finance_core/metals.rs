// Catálogo de metais preciosos — porta de `lume/api/app/sources/metals_catalog.py`.
// Metais são só cotações do Yahoo de futuros COMEX/NYMEX (códigos ISO 4217
// XAU/XAG/XPT/XPD); preço e quantidade ficam em onça troy, a unidade real do
// contrato, sem conversão.

pub struct MetalInfo {
    pub code: &'static str,
    pub yahoo_symbol: &'static str,
    pub name: &'static str,
}

const CATALOG: [MetalInfo; 4] = [
    MetalInfo { code: "xau", yahoo_symbol: "GC=F", name: "Gold" },
    MetalInfo { code: "xag", yahoo_symbol: "SI=F", name: "Silver" },
    MetalInfo { code: "xpt", yahoo_symbol: "PL=F", name: "Platinum" },
    MetalInfo { code: "xpd", yahoo_symbol: "PA=F", name: "Palladium" },
];

/// Aceita o código em qualquer caixa (`XAU` ou `xau`).
pub fn metal_info(code: &str) -> Option<&'static MetalInfo> {
    let code = code.to_lowercase();
    CATALOG.iter().find(|m| m.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_case_insensitive() {
        assert_eq!(metal_info("XAU").unwrap().yahoo_symbol, "GC=F");
        assert_eq!(metal_info("xpd").unwrap().name, "Palladium");
        assert!(metal_info("copper").is_none());
    }
}

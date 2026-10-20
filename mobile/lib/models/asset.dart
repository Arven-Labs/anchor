/// 4 classes resolvidas pelo mesmo endpoint Yahoo `.SA` já usado por
/// `YahooQuoteService` (confirmado no desktop, Sessões 41/63: FII/ETF
/// BR/BDR devolvem o mesmo formato de `meta` que uma ação), mais 5 classes
/// com fonte própria — `acaoInternacional`/`reit`/`etfUs` (todas Yahoo sem
/// `.SA`, mesmo endpoint — REIT é o equivalente americano do FII, ETF US é
/// ETF listado nos EUA tipo SPY), `metal` (Yahoo sem `.SA`, ticker de
/// contrato futuro por trás) e `cripto` (CoinGecko, por `coin_id` em vez de
/// ticker) — ver `QuoteDispatcher`. Mais 4 classes **sem** cotação
/// automática (`hasAutoQuote == false`, espelha `ASSET_CLASSES_WITH_AUTO_QUOTE`
/// do desktop): `tesouroDireto`/`rendaFixa` (renda fixa, ganham campos
/// `fi_*` na transação de compra — ver `PortfolioTransaction`) e
/// `imovel`/`empresaNaoListada` (cadastro manual, histórico de avaliação em
/// `asset_valuations` — ver `ManualAssetDetailsScreen`). Valores string
/// iguais ao desktop (`desktop/src/portfolio/types.ts`).
enum AssetClass {
  acaoBr('acao_br', 'Ação BR'),
  fii('fii', 'FII'),
  etfBr('etf_br', 'ETF BR'),
  etfRendaFixa('etf_renda_fixa', 'ETF Renda Fixa BR'),
  bdr('bdr', 'BDR'),
  acaoInternacional('acao_internacional', 'Ação internacional'),
  reit('reit', 'REIT'),
  etfUs('etf_us', 'ETF US'),
  cripto('cripto', 'Cripto'),
  metal('metal', 'Metal'),
  tesouroDireto('tesouro_direto', 'Tesouro Direto'),
  rendaFixa('renda_fixa', 'Renda Fixa'),
  imovel('imovel', 'Imóvel'),
  empresaNaoListada('empresa_nao_listada', 'Empresa não listada');

  final String value;
  final String label;

  const AssetClass(this.value, this.label);

  static AssetClass fromValue(String value) {
    return AssetClass.values.firstWhere((c) => c.value == value);
  }
}

/// Ticker de exemplo e moeda default por classe — evita repetir o mesmo
/// switch em cada tela que precisa de um placeholder/moeda inicial.
extension AssetClassMeta on AssetClass {
  String get tickerHint {
    switch (this) {
      case AssetClass.acaoBr:
        return 'PETR4';
      case AssetClass.fii:
        return 'HGLG11';
      case AssetClass.etfBr:
        return 'BOVA11';
      case AssetClass.etfRendaFixa:
        return 'AUPO11';
      case AssetClass.bdr:
        return 'AAPL34';
      case AssetClass.acaoInternacional:
        return 'AAPL';
      case AssetClass.reit:
        return 'O';
      case AssetClass.etfUs:
        return 'SPY';
      case AssetClass.cripto:
        return 'ETH';
      case AssetClass.metal:
        return 'XAU';
      case AssetClass.tesouroDireto:
        return 'Tesouro Selic 2029';
      case AssetClass.rendaFixa:
        return 'CDB Banco X';
      case AssetClass.imovel:
        return 'Apto Centro';
      case AssetClass.empresaNaoListada:
        return 'Empresa XYZ';
    }
  }

  String get defaultCurrency {
    switch (this) {
      case AssetClass.acaoBr:
      case AssetClass.fii:
      case AssetClass.etfBr:
      case AssetClass.etfRendaFixa:
      case AssetClass.bdr:
      case AssetClass.tesouroDireto:
      case AssetClass.rendaFixa:
      case AssetClass.imovel:
      case AssetClass.empresaNaoListada:
        return 'BRL';
      case AssetClass.acaoInternacional:
      case AssetClass.reit:
      case AssetClass.etfUs:
      case AssetClass.metal:
        return 'USD';
      case AssetClass.cripto:
        // Cotada em reais direto no CoinGecko (`vs_currencies=brl`); cripto
        // cadastrada antes disso segue em USD, na moeda do próprio ativo.
        return 'BRL';
    }
  }

  /// Espelha `ASSET_CLASSES_WITH_AUTO_QUOTE`
  /// (`desktop/src/portfolio/types.ts`) — só as 10 classes de mercado
  /// (inclui `etfRendaFixa`, cotado como as da B3) têm fonte automática;
  /// renda fixa e os 2 cadastros manuais não têm cotação nenhuma (nem no desktop).
  bool get hasAutoQuote {
    switch (this) {
      case AssetClass.tesouroDireto:
      case AssetClass.rendaFixa:
      case AssetClass.imovel:
      case AssetClass.empresaNaoListada:
        return false;
      default:
        return true;
    }
  }

  /// Espelha `FIXED_INCOME_CLASSES` do desktop — só essas 2 ganham o bloco
  /// de campos `fi_*` na transação de compra.
  bool get isFixedIncome =>
      this == AssetClass.tesouroDireto || this == AssetClass.rendaFixa;

  /// As 2 classes de cadastro 100% manual (`ManualAssetDetails.tsx` no
  /// desktop) — ganham histórico de avaliação e, no caso de
  /// `empresaNaoListada`, os campos de participação societária.
  bool get isManualAsset =>
      this == AssetClass.imovel || this == AssetClass.empresaNaoListada;
}

class Asset {
  final int? id;
  final String ticker;
  final String name;
  final AssetClass assetClass;
  final String currency;
  final DateTime createdAt;

  /// `coin_id` do CoinGecko (ex: `ethereum`), resolvido e salvo no cadastro
  /// de um ativo `cripto` — evita rebuscar por texto a cada refresh do
  /// Portfolio. `null` pras outras classes.
  final String? externalId;

  /// Participação societária — só usados por `empresaNaoListada`. Nunca
  /// digitado direto: `equityPercentual`/`equityParticipationValue` abaixo
  /// são sempre calculados a partir destes 3, mesma regra de
  /// `ManualAssetDetails.tsx` no desktop ("percentage is always calculated
  /// from shares owned / total company shares").
  final double? equitySharesOwned;
  final double? equityTotalShares;
  final double? equityCompanyValuation;

  const Asset({
    this.id,
    required this.ticker,
    required this.name,
    required this.assetClass,
    required this.currency,
    required this.createdAt,
    this.externalId,
    this.equitySharesOwned,
    this.equityTotalShares,
    this.equityCompanyValuation,
  });

  double? get equityPercentual {
    final owned = equitySharesOwned;
    final total = equityTotalShares;
    if (owned == null || total == null || total <= 0) return null;
    return owned / total;
  }

  double? get equityParticipationValue {
    final percentual = equityPercentual;
    final valuation = equityCompanyValuation;
    if (percentual == null || valuation == null) return null;
    return percentual * valuation;
  }

  Map<String, Object?> toMap() {
    return {
      'id': id,
      'ticker': ticker,
      'name': name,
      'asset_class': assetClass.value,
      'currency': currency,
      'created_at': createdAt.toIso8601String(),
      'external_id': externalId,
      'equity_shares_owned': equitySharesOwned,
      'equity_total_shares': equityTotalShares,
      'equity_company_valuation': equityCompanyValuation,
    };
  }

  factory Asset.fromMap(Map<String, Object?> map) {
    return Asset(
      id: map['id'] as int,
      ticker: map['ticker'] as String,
      name: map['name'] as String,
      assetClass: AssetClass.fromValue(map['asset_class'] as String),
      currency: map['currency'] as String,
      createdAt: DateTime.parse(map['created_at'] as String),
      externalId: map['external_id'] as String?,
      equitySharesOwned: map['equity_shares_owned'] as double?,
      equityTotalShares: map['equity_total_shares'] as double?,
      equityCompanyValuation: map['equity_company_valuation'] as double?,
    );
  }
}

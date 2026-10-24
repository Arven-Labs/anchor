import 'dart:io';

import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

import '../data/db.dart';
import '../domain/cvm_inf_diario.dart' show formatCnpj, normalizeCnpj;
import '../domain/dirpf.dart';
import '../domain/fund_compensation.dart' show collapseAdministrator;
import '../models/asset.dart';
import '../models/asset_attachment.dart';
import '../models/asset_valuation.dart';
import '../models/custodia.dart';
import '../models/fund.dart';
import '../models/portfolio_transaction.dart';
import '../models/position.dart';

const _attachmentContentTypeByExtension = {
  '.pdf': 'application/pdf',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  '.xls': 'application/vnd.ms-excel',
  '.csv': 'text/csv',
};

String _sanitizeAttachmentFileName(String name) =>
    name.replaceAll('/', '_').replaceAll('\\', '_');

class PortfolioRepository {
  Future<int> insertAsset(Asset asset) async {
    final db = await AppDatabase.instance();
    final map = asset.toMap()..remove('id');
    return db.insert('assets', map);
  }

  Future<List<Asset>> listAssets() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('assets', orderBy: 'ticker');
    return rows.map(Asset.fromMap).toList();
  }

  Future<void> deleteAsset(int id) async {
    final db = await AppDatabase.instance();
    await db.delete('assets', where: 'id = ?', whereArgs: [id]);
  }

  /// Percentual/valor da participação nunca são gravados direto — sempre
  /// recalculados a partir destes 3 campos (ver `Asset.equityPercentual`),
  /// mesma regra do `update_asset_equity` do desktop.
  Future<void> updateAssetEquity(
    int assetId, {
    double? sharesOwned,
    double? totalShares,
    double? companyValuation,
  }) async {
    final db = await AppDatabase.instance();
    await db.update(
      'assets',
      {
        'equity_shares_owned': sharesOwned,
        'equity_total_shares': totalShares,
        'equity_company_valuation': companyValuation,
      },
      where: 'id = ?',
      whereArgs: [assetId],
    );
  }

  /// `origin` sempre `'manual'` — mesma regra do `add_asset_valuation` do
  /// desktop, nunca exposta como parâmetro pra tela não poder gravar outra
  /// coisa.
  Future<int> insertAssetValuation({
    required int assetId,
    required DateTime valuationDate,
    required double value,
    String? notes,
  }) async {
    final db = await AppDatabase.instance();
    return db.insert(
      'asset_valuations',
      AssetValuation(
        assetId: assetId,
        valuationDate: valuationDate,
        value: value,
        origin: 'manual',
        notes: notes,
        createdAt: DateTime.now(),
      ).toMap()..remove('id'),
    );
  }

  Future<List<AssetValuation>> listAssetValuations(int assetId) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'asset_valuations',
      where: 'asset_id = ?',
      whereArgs: [assetId],
      orderBy: 'valuation_date',
    );
    return rows.map(AssetValuation.fromMap).toList();
  }

  /// `null` quando o ativo nunca recebeu uma avaliação — usado pelo
  /// Portfolio como "preço atual" de `imovel` (quantidade é sempre 1, então
  /// 1×valor = valor, sem precisar de matemática nova).
  Future<AssetValuation?> getLatestValuation(int assetId) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'asset_valuations',
      where: 'asset_id = ?',
      whereArgs: [assetId],
      orderBy: 'valuation_date DESC',
      limit: 1,
    );
    return rows.isEmpty ? null : AssetValuation.fromMap(rows.first);
  }

  Future<void> deleteAssetValuation(int id) async {
    final db = await AppDatabase.instance();
    await db.delete('asset_valuations', where: 'id = ?', whereArgs: [id]);
  }

  /// Tipo tributário do fundo; `null` enquanto o usuário não escolheu.
  Future<FundTerm?> getFundTerm(int assetId) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'fund_details',
      where: 'asset_id = ?',
      whereArgs: [assetId],
    );
    return rows.isEmpty ? null : FundTerm.parse(rows.first['term'] as String?);
  }

  /// Só o tipo muda: a subclasse da CVM e a administradora, na mesma linha, ficam (um
  /// `INSERT OR REPLACE` as apagaria, como o `set_fund_term` do desktop nunca faz).
  Future<void> setFundTerm(int assetId, FundTerm term) async {
    final db = await AppDatabase.instance();
    final updated = await db.update(
      'fund_details',
      {'term': term.value},
      where: 'asset_id = ?',
      whereArgs: [assetId],
    );
    if (updated == 0) {
      await db.insert('fund_details', {
        'asset_id': assetId,
        'term': term.value,
      });
    }
  }

  /// Tipo de todos os fundos que já têm um, por id do ativo.
  Future<Map<int, FundTerm>> listFundTerms() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('fund_details');
    final terms = <int, FundTerm>{};
    for (final row in rows) {
      final term = FundTerm.parse(row['term'] as String?);
      if (term != null) terms[row['asset_id'] as int] = term;
    }
    return terms;
  }

  /// Uma data tem um valor só: lançar de novo na mesma data corrige o anterior.
  Future<void> setFundQuota({
    required int assetId,
    required DateTime date,
    required double value,
    String? notes,
  }) async {
    final db = await AppDatabase.instance();
    final quota = FundQuota(
      assetId: assetId,
      date: date,
      value: value,
      notes: notes,
      createdAt: DateTime.now(),
    );
    final map = quota.toMap()..remove('id');
    await db.transaction((txn) async {
      await txn.delete(
        'fund_quota_values',
        where: 'asset_id = ? AND quota_date = ?',
        whereArgs: [assetId, map['quota_date']],
      );
      await txn.insert('fund_quota_values', map);
    });
  }

  Future<List<FundQuota>> listFundQuotas(int assetId) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'fund_quota_values',
      where: 'asset_id = ?',
      whereArgs: [assetId],
      orderBy: 'quota_date DESC',
    );
    return rows.map(FundQuota.fromMap).toList();
  }

  Future<List<FundQuota>> listAllFundQuotas() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('fund_quota_values');
    return rows.map(FundQuota.fromMap).toList();
  }

  /// Última cota lançada — o "preço atual" da posição de fundo. Data futura não
  /// conta (mesma regra da estimativa do come-cotas).
  Future<FundQuota?> getLatestFundQuota(int assetId, {DateTime? today}) async {
    final db = await AppDatabase.instance();
    final now = today ?? DateTime.now();
    final rows = await db.query(
      'fund_quota_values',
      where: 'asset_id = ? AND quota_date <= ?',
      whereArgs: [
        assetId,
        '${now.year.toString().padLeft(4, '0')}-'
            '${now.month.toString().padLeft(2, '0')}-'
            '${now.day.toString().padLeft(2, '0')}',
      ],
      orderBy: 'quota_date DESC',
      limit: 1,
    );
    return rows.isEmpty ? null : FundQuota.fromMap(rows.first);
  }

  /// Subclasse da CVM escolhida de cada fundo que tem uma, por id do ativo.
  Future<Map<int, String>> listFundSubclasses() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('fund_details');
    return {
      for (final row in rows)
        if ((row['cvm_subclass'] as String?)?.isNotEmpty ?? false)
          row['asset_id'] as int: row['cvm_subclass'] as String,
    };
  }

  /// Administradora (ou intermediário) de cada fundo que tem uma, por id do ativo.
  Future<Map<int, String>> listFundAdministrators() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('fund_details');
    return {
      for (final row in rows)
        if ((row['administrator'] as String?)?.isNotEmpty ?? false)
          row['asset_id'] as int: row['administrator'] as String,
    };
  }

  /// Estrutura (fechado ou exclusivo) dos fundos que não são abertos, por id do ativo.
  Future<Map<int, FundStructure>> listFundStructures() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('fund_details');
    return {
      for (final row in rows)
        row['asset_id'] as int: ?FundStructure.parse(
          row['structure'] as String?,
        ),
    };
  }

  /// Como o estoque de 2023 foi tributado, dos fundos em que o usuário informou.
  Future<Map<int, EstoqueOption>> listFundEstoqueOptions() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('fund_details');
    return {
      for (final row in rows)
        row['asset_id'] as int: ?EstoqueOption.parse(
          row['estoque_option'] as String?,
        ),
    };
  }

  /// Grava se o fundo é aberto, fechado ou exclusivo (Lei 14.754); só os dois últimos
  /// têm estoque de 2023. Mora em `fund_details`, que só existe depois de o usuário
  /// escolher o tipo do fundo.
  Future<void> setFundStructure(int assetId, FundStructure structure) async {
    await _updateFundDetail(assetId, {
      'structure': structure.value,
    }, 'da estrutura');
  }

  /// Grava como o estoque de rendimento até 31/12/2023 foi tributado (`null` = não
  /// informado, nada é estimado). Mesma regra do tipo do fundo: precisa existir.
  Future<void> setFundEstoque(int assetId, EstoqueOption? option) async {
    await _updateFundDetail(assetId, {
      'estoque_option': option?.value,
    }, 'do estoque');
  }

  Future<void> _updateFundDetail(
    int assetId,
    Map<String, Object?> values,
    String what,
  ) async {
    final db = await AppDatabase.instance();
    final updated = await db.update(
      'fund_details',
      values,
      where: 'asset_id = ?',
      whereArgs: [assetId],
    );
    if (updated == 0) {
      throw FormatException('Escolha o tipo do fundo antes $what');
    }
  }

  /// Grava a administradora (ou o intermediário) do fundo, em texto livre: fundos do
  /// mesmo tipo com a mesma administradora compensam prejuízo entre si. Vazio apaga (o
  /// fundo fica isolado). Mora em `fund_details`, que só existe depois de o usuário
  /// escolher o tipo do fundo.
  Future<void> setFundAdministrator(int assetId, String? administrator) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'fund_details',
      columns: ['asset_id'],
      where: 'asset_id = ?',
      whereArgs: [assetId],
    );
    if (rows.isEmpty) {
      throw const FormatException(
        'Escolha o tipo do fundo antes da administradora',
      );
    }
    await db.update(
      'fund_details',
      {'administrator': collapseAdministrator(administrator)},
      where: 'asset_id = ?',
      whereArgs: [assetId],
    );
  }

  /// Grava o grupo, o código e o país da ficha "Bens e Direitos" do ativo, informados
  /// pelo usuário (vazio apaga o campo). Só completa zeros à esquerda; não confere com a
  /// tabela da Receita. Valor inválido lança [FormatException] e não grava nada.
  Future<void> setAssetDirpf(
    int assetId, {
    String? group,
    String? code,
    String? country,
  }) async {
    final normalizedGroup = normalizeDirpfField('group', group, dirpfGroupLength);
    final normalizedCode = normalizeDirpfField('code', code, dirpfCodeLength);
    final normalizedCountry = normalizeDirpfField(
      'country',
      country,
      dirpfCountryLength,
    );
    final db = await AppDatabase.instance();
    await db.update(
      'assets',
      {
        'dirpf_group': normalizedGroup,
        'dirpf_code': normalizedCode,
        'dirpf_country': normalizedCountry,
      },
      where: 'id = ?',
      whereArgs: [assetId],
    );
  }

  /// Grava o CNPJ (no ativo) e a subclasse do fundo para a leitura da CVM. Vazio =
  /// sem CNPJ. Mudar um dos dois apaga as cotas já lidas da CVM do fundo (eram de
  /// outra série); as lançadas à mão ficam. A subclasse mora em `fund_details`, que
  /// só existe depois de o usuário escolher o tipo do fundo.
  Future<void> setFundCvm(int assetId, {String? cnpj, String? subclass}) async {
    final db = await AppDatabase.instance();
    final digits = normalizeCnpj(cnpj ?? '');
    if (digits.isNotEmpty && digits.length != 14) {
      throw const FormatException('O CNPJ precisa ter 14 dígitos');
    }
    final newCnpj = digits.isEmpty ? null : formatCnpj(digits);
    final newSubclass = (subclass?.trim().isEmpty ?? true)
        ? null
        : subclass!.trim();

    final assets = await db.query(
      'assets',
      columns: ['cnpj'],
      where: 'id = ?',
      whereArgs: [assetId],
    );
    final details = await db.query(
      'fund_details',
      where: 'asset_id = ?',
      whereArgs: [assetId],
    );
    if (newSubclass != null && details.isEmpty) {
      throw const FormatException(
        'Escolha o tipo do fundo antes da subclasse da CVM',
      );
    }
    final oldCnpj = assets.isEmpty ? null : assets.first['cnpj'] as String?;
    final oldSubclass = details.isEmpty
        ? null
        : details.first['cvm_subclass'] as String?;
    final changed =
        normalizeCnpj(oldCnpj ?? '') != digits || oldSubclass != newSubclass;

    await db.transaction((txn) async {
      await txn.update(
        'assets',
        {'cnpj': newCnpj},
        where: 'id = ?',
        whereArgs: [assetId],
      );
      if (details.isNotEmpty) {
        await txn.update(
          'fund_details',
          {'cvm_subclass': newSubclass},
          where: 'asset_id = ?',
          whereArgs: [assetId],
        );
      }
      if (changed) {
        await txn.delete(
          'fund_quota_values',
          where: "asset_id = ? AND source = 'cvm'",
          whereArgs: [assetId],
        );
      }
    });
  }

  /// As datas das cotas lidas da CVM do fundo.
  Future<List<DateTime>> listCvmQuotaDates(int assetId) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'fund_quota_values',
      columns: ['quota_date'],
      where: "asset_id = ? AND source = 'cvm'",
      whereArgs: [assetId],
    );
    return [
      for (final row in rows)
        DateTime.parse('${row['quota_date']}T00:00:00Z'),
    ];
  }

  /// Grava as cotas lidas da CVM, pulando as datas em que o usuário lançou a sua e
  /// trocando as da CVM que já existiam.
  Future<void> upsertCvmQuotas(int assetId, Map<DateTime, double> quotas) async {
    if (quotas.isEmpty) return;
    final db = await AppDatabase.instance();
    final createdAt = DateTime.now().toIso8601String();
    await db.transaction((txn) async {
      final manual = {
        for (final row in await txn.query(
          'fund_quota_values',
          columns: ['quota_date'],
          where: "asset_id = ? AND source = 'manual'",
          whereArgs: [assetId],
        ))
          row['quota_date'] as String,
      };
      final batch = txn.batch();
      quotas.forEach((date, value) {
        final key = FundQuota(
          assetId: assetId,
          date: date,
          value: value,
          createdAt: DateTime.now(),
        ).toMap()['quota_date']! as String;
        if (manual.contains(key)) return;
        batch.delete(
          'fund_quota_values',
          where: 'asset_id = ? AND quota_date = ?',
          whereArgs: [assetId, key],
        );
        batch.insert('fund_quota_values', {
          'asset_id': assetId,
          'quota_date': key,
          'value': value,
          'source': 'cvm',
          'created_at': createdAt,
        });
      });
      await batch.commit(noResult: true);
    });
  }

  Future<void> deleteFundQuota(int id) async {
    final db = await AppDatabase.instance();
    await db.delete('fund_quota_values', where: 'id = ?', whereArgs: [id]);
  }

  Future<List<AssetAttachment>> listAssetAttachments(int assetId) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'asset_attachments',
      where: 'asset_id = ?',
      whereArgs: [assetId],
      orderBy: 'created_at',
    );
    return rows.map(AssetAttachment.fromMap).toList();
  }

  /// Copia o arquivo escolhido (`sourcePath`, vindo de `file_picker`) pro
  /// diretório de documentos do app — mesmo espírito do `add_asset_attachment`
  /// do desktop, que copia pro `app_data_dir()`. O mobile não guarda
  /// referência ao caminho original (galeria/Downloads podem apagá-lo a
  /// qualquer momento), só a cópia própria.
  Future<AssetAttachment> addAssetAttachment({
    required int assetId,
    required String sourcePath,
    String? documentType,
  }) async {
    final originalFileName = p.basename(sourcePath);
    final documentsDir = await getApplicationDocumentsDirectory();
    final dir = Directory(
      p.join(documentsDir.path, 'asset_attachments', assetId.toString()),
    );
    await dir.create(recursive: true);

    final storedName =
        '${DateTime.now().millisecondsSinceEpoch}_${_sanitizeAttachmentFileName(originalFileName)}';
    final destFile = File(p.join(dir.path, storedName));
    await File(sourcePath).copy(destFile.path);
    final fileSizeBytes = await destFile.length();
    final storedRelativePath = p.join(
      'asset_attachments',
      assetId.toString(),
      storedName,
    );

    final db = await AppDatabase.instance();
    final id = await db.insert(
      'asset_attachments',
      AssetAttachment(
        assetId: assetId,
        originalFileName: originalFileName,
        storedRelativePath: storedRelativePath,
        fileSizeBytes: fileSizeBytes,
        contentType:
            _attachmentContentTypeByExtension[p
                .extension(originalFileName)
                .toLowerCase()],
        documentType: documentType,
        createdAt: DateTime.now(),
      ).toMap()..remove('id'),
    );

    return AssetAttachment(
      id: id,
      assetId: assetId,
      originalFileName: originalFileName,
      storedRelativePath: storedRelativePath,
      fileSizeBytes: fileSizeBytes,
      contentType:
          _attachmentContentTypeByExtension[p
              .extension(originalFileName)
              .toLowerCase()],
      documentType: documentType,
      createdAt: DateTime.now(),
    );
  }

  /// Caminho absoluto de um anexo em disco — usado por `open_filex` pra
  /// abrir com o app padrão do sistema.
  Future<String> resolveAssetAttachmentPath(String storedRelativePath) async {
    final documentsDir = await getApplicationDocumentsDirectory();
    return p.join(documentsDir.path, storedRelativePath);
  }

  Future<void> deleteAssetAttachment(AssetAttachment attachment) async {
    final absolutePath = await resolveAssetAttachmentPath(
      attachment.storedRelativePath,
    );
    final file = File(absolutePath);
    if (await file.exists()) {
      await file.delete();
    }

    final db = await AppDatabase.instance();
    await db.delete(
      'asset_attachments',
      where: 'id = ?',
      whereArgs: [attachment.id],
    );
  }

  Future<int> insertCustodia(Custodia custodia) async {
    final db = await AppDatabase.instance();
    final map = custodia.toMap()..remove('id');
    return db.insert('custodias', map);
  }

  Future<List<Custodia>> listCustodias() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('custodias', orderBy: 'instituicao');
    return rows.map(Custodia.fromMap).toList();
  }

  Future<void> updateCustodia(Custodia custodia) async {
    final db = await AppDatabase.instance();
    await db.update(
      'custodias',
      custodia.toMap()..remove('id'),
      where: 'id = ?',
      whereArgs: [custodia.id],
    );
  }

  Future<void> deleteCustodia(int id) async {
    final db = await AppDatabase.instance();
    await db.delete('custodias', where: 'id = ?', whereArgs: [id]);
  }

  Future<int> insertTransaction(PortfolioTransaction transaction) async {
    final db = await AppDatabase.instance();
    final map = transaction.toMap()..remove('id');
    return db.insert('portfolio_transactions', map);
  }

  /// Grava várias transações numa só transação do banco: ou entram todas ou
  /// nenhuma (a permuta de cripto grava uma venda e uma compra juntas).
  Future<void> insertTransactions(
    List<PortfolioTransaction> transactions,
  ) async {
    final db = await AppDatabase.instance();
    await db.transaction((txn) async {
      for (final transaction in transactions) {
        await txn.insert(
          'portfolio_transactions',
          transaction.toMap()..remove('id'),
        );
      }
    });
  }

  Future<List<PortfolioTransaction>> listTransactionsForAsset(
    int assetId,
  ) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'portfolio_transactions',
      where: 'asset_id = ?',
      whereArgs: [assetId],
      orderBy: 'transaction_date',
    );
    return _decodeTransactions(rows);
  }

  Future<List<PortfolioTransaction>> listAllTransactions() async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'portfolio_transactions',
      orderBy: 'transaction_date DESC',
    );
    return _decodeTransactions(rows);
  }

  /// Pula a linha de um tipo que o app não conhece, em vez de derrubar a listagem toda.
  static List<PortfolioTransaction> _decodeTransactions(
    List<Map<String, Object?>> rows,
  ) {
    return [
      for (final row in rows)
        if (TransactionType.tryParse(row['transaction_type'] as String?) !=
            null)
          PortfolioTransaction.fromMap(row),
    ];
  }

  Future<void> updateTransaction(PortfolioTransaction transaction) async {
    final db = await AppDatabase.instance();
    await db.update(
      'portfolio_transactions',
      transaction.toMap()..remove('id'),
      where: 'id = ?',
      whereArgs: [transaction.id],
    );
  }

  Future<void> deleteTransaction(int id) async {
    final db = await AppDatabase.instance();
    await db.delete('portfolio_transactions', where: 'id = ?', whereArgs: [id]);
  }

  Future<List<Position>> getPositions() async {
    final assets = await listAssets();
    final transactions = await listAllTransactions();
    return computePositions(assets, transactions);
  }

  /// Mesma lógica de `get_portfolio_positions`
  /// (`desktop/src-tauri/src/commands/transaction.rs`): quantidade líquida
  /// é compra menos venda; preço médio é a média ponderada só das compras
  /// (vendas não reduzem a base de custo — não é FIFO, é a mesma
  /// simplificação deliberada do desktop). Função pura, sem banco, pra
  /// facilitar teste.
  static List<Position> computePositions(
    List<Asset> assets,
    List<PortfolioTransaction> transactions,
  ) {
    final positions = <Position>[];

    for (final asset in assets) {
      var netQuantity = 0.0;
      var buyQuantitySum = 0.0;
      var buyValueSum = 0.0;
      final custodiaQty = <int?, double>{};

      for (final tx in transactions.where((t) => t.assetId == asset.id)) {
        switch (tx.type) {
          case TransactionType.compra:
            netQuantity += tx.quantity!;
            buyQuantitySum += tx.quantity!;
            buyValueSum += tx.totalValue;
            custodiaQty[tx.custodiaId] =
                (custodiaQty[tx.custodiaId] ?? 0) + tx.quantity!;
          case TransactionType.venda:
          // O come-cotas tira cotas como uma venda e não mexe no preço médio
          // (mesma regra do desktop).
          case TransactionType.comeCotas:
            netQuantity -= tx.quantity!;
            custodiaQty[tx.custodiaId] =
                (custodiaQty[tx.custodiaId] ?? 0) - tx.quantity!;
          case TransactionType.transferencia:
            custodiaQty[tx.custodiaId] =
                (custodiaQty[tx.custodiaId] ?? 0) - tx.quantity!;
            custodiaQty[tx.transferToCustodiaId] =
                (custodiaQty[tx.transferToCustodiaId] ?? 0) + tx.quantity!;
          case TransactionType.aporte:
          case TransactionType.retirada:
          case TransactionType.provento:
          case TransactionType.amortizacao:
            // Fluxo de caixa (aporte/retirada), dividendo (provento) ou
            // amortização de fundo — nenhum move quantidade nem entra no preço
            // médio, mesma regra do desktop (`get_portfolio_positions`).
            break;
        }
      }

      positions.add(
        Position(
          asset: asset,
          netQuantity: netQuantity,
          averageBuyPrice: buyQuantitySum > 0
              ? buyValueSum / buyQuantitySum
              : null,
          byCustodia: custodiaQty.entries
              .map(
                (e) => CustodiaBreakdown(custodiaId: e.key, quantity: e.value),
              )
              .toList(),
        ),
      );
    }

    return positions;
  }
}

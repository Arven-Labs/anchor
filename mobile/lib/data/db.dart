import 'package:flutter/foundation.dart' show visibleForTesting;
import 'package:path/path.dart';
import 'package:sqflite/sqflite.dart';

/// Banco local do portfolio — 13 classes de ativo (as 9 com cotação
/// automática via `QuoteDispatcher`, mais Tesouro Direto/Renda Fixa — sem
/// cotação, só campos `fi_*` na transação de compra — e Imóvel/Empresa não
/// listada — cadastro manual, histórico de avaliação em `asset_valuations`,
/// ver `AssetClassMeta`), 8 tipos de transação
/// (compra/venda/aporte/retirada/provento/transferencia/come_cotas/amortizacao —
/// ver `TransactionTypeMeta`) e custódia (conta/corretora, sem `workspace_id` —
/// diferente do desktop, o mobile não tem Workspace).
///
/// Ao contrário do desktop (que exige `sea-orm-cli migrate up` manual), as
/// tabelas nascem sozinhas no `onCreate`, na primeira abertura — não existe
/// usuário/DBA rodando migração à mão num app mobile. `onUpgrade` cobre só
/// quem já tinha o app instalado antes de uma coluna nova aparecer.
class AppDatabase {
  static Database? _instance;

  static Future<Database> instance() async {
    _instance ??= await _open();
    return _instance!;
  }

  /// Fecha e esquece a conexão: o próximo `instance()` reabre (e migra) o arquivo
  /// que `getDatabasesPath()` apontar nesse momento. Só para teste.
  @visibleForTesting
  static Future<void> resetForTest() async {
    await _instance?.close();
    _instance = null;
  }

  static Future<Database> _open() async {
    final path = join(await getDatabasesPath(), 'anchor_mobile.db');
    return openDatabase(
      path,
      version: 15,
      onCreate: (db, version) async {
        await db.execute('''
          CREATE TABLE assets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ticker TEXT NOT NULL,
            name TEXT NOT NULL,
            asset_class TEXT NOT NULL,
            currency TEXT NOT NULL,
            created_at TEXT NOT NULL,
            external_id TEXT,
            equity_shares_owned REAL,
            equity_total_shares REAL,
            equity_company_valuation REAL,
            cnpj TEXT,
            dirpf_group TEXT,
            dirpf_code TEXT,
            dirpf_country TEXT
          )
        ''');
        await db.execute('''
          CREATE TABLE custodias (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            instituicao TEXT NOT NULL,
            titular TEXT NOT NULL,
            created_at TEXT NOT NULL
          )
        ''');
        await db.execute('''
          CREATE TABLE portfolio_transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            asset_id INTEGER REFERENCES assets(id) ON DELETE CASCADE,
            custodia_id INTEGER REFERENCES custodias(id),
            transfer_to_custodia_id INTEGER REFERENCES custodias(id),
            transaction_type TEXT NOT NULL,
            quantity REAL,
            unit_price REAL,
            total_value REAL NOT NULL,
            transaction_date TEXT NOT NULL,
            notes TEXT,
            created_at TEXT NOT NULL,
            fi_emissor TEXT,
            fi_indexador TEXT,
            fi_taxa_percentual REAL,
            fi_data_vencimento TEXT,
            fi_liquidez TEXT,
            fee REAL,
            foreign_tax_withheld REAL,
            loss_credit_used REAL,
            foreign_tax_paid_on TEXT
          )
        ''');
        await db.execute('''
          CREATE TABLE asset_valuations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
            valuation_date TEXT NOT NULL,
            value REAL NOT NULL,
            origin TEXT NOT NULL,
            notes TEXT,
            created_at TEXT NOT NULL
          )
        ''');
        await db.execute('''
          CREATE TABLE asset_attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
            original_file_name TEXT NOT NULL,
            stored_relative_path TEXT NOT NULL,
            file_size_bytes INTEGER NOT NULL,
            content_type TEXT,
            document_type TEXT,
            created_at TEXT NOT NULL
          )
        ''');
        await _createFundTables(db);
      },
      onUpgrade: (db, oldVersion, newVersion) async {
        if (oldVersion < 2) {
          await db.execute('ALTER TABLE assets ADD COLUMN external_id TEXT');
        }
        if (oldVersion < 3) {
          // `aporte`/`retirada` não referenciam ativo e `provento` não tem
          // quantidade/preço unitário — SQLite não altera constraint NOT
          // NULL via ALTER TABLE, então a tabela é recriada com o schema
          // relaxado e os dados existentes copiados.
          await db.execute('''
            CREATE TABLE portfolio_transactions_new (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              asset_id INTEGER REFERENCES assets(id) ON DELETE CASCADE,
              transaction_type TEXT NOT NULL,
              quantity REAL,
              unit_price REAL,
              total_value REAL NOT NULL,
              transaction_date TEXT NOT NULL,
              notes TEXT,
              created_at TEXT NOT NULL
            )
          ''');
          await db.execute('''
            INSERT INTO portfolio_transactions_new
            SELECT id, asset_id, transaction_type, quantity, unit_price,
                   total_value, transaction_date, notes, created_at
            FROM portfolio_transactions
          ''');
          await db.execute('DROP TABLE portfolio_transactions');
          await db.execute(
            'ALTER TABLE portfolio_transactions_new RENAME TO portfolio_transactions',
          );
        }
        if (oldVersion < 4) {
          await db.execute('''
            CREATE TABLE custodias (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              instituicao TEXT NOT NULL,
              titular TEXT NOT NULL,
              created_at TEXT NOT NULL
            )
          ''');
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN custodia_id INTEGER REFERENCES custodias(id)',
          );
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN transfer_to_custodia_id INTEGER REFERENCES custodias(id)',
          );
        }
        if (oldVersion < 5) {
          await db.execute(
            'ALTER TABLE assets ADD COLUMN equity_shares_owned REAL',
          );
          await db.execute(
            'ALTER TABLE assets ADD COLUMN equity_total_shares REAL',
          );
          await db.execute(
            'ALTER TABLE assets ADD COLUMN equity_company_valuation REAL',
          );
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN fi_emissor TEXT',
          );
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN fi_indexador TEXT',
          );
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN fi_taxa_percentual REAL',
          );
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN fi_data_vencimento TEXT',
          );
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN fi_liquidez TEXT',
          );
          await db.execute('''
            CREATE TABLE asset_valuations (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
              valuation_date TEXT NOT NULL,
              value REAL NOT NULL,
              origin TEXT NOT NULL,
              notes TEXT,
              created_at TEXT NOT NULL
            )
          ''');
        }
        if (oldVersion < 6) {
          await db.execute('''
            CREATE TABLE asset_attachments (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
              original_file_name TEXT NOT NULL,
              stored_relative_path TEXT NOT NULL,
              file_size_bytes INTEGER NOT NULL,
              content_type TEXT,
              document_type TEXT,
              created_at TEXT NOT NULL
            )
          ''');
        }
        if (oldVersion < 7) {
          // Taxa da compra/venda; linhas antigas ficam sem taxa (nula).
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN fee REAL',
          );
        }
        if (oldVersion < 8) {
          // Imposto retido no exterior sobre o provento; linhas antigas ficam
          // sem retenção (nula).
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN foreign_tax_withheld REAL',
          );
        }
        if (oldVersion < 9) {
          // Fundos: tipo do fundo e valor da cota por data.
          await _createFundTables(db);
        }
        if (oldVersion < 10) {
          // Leitura da cota pela CVM: CNPJ do fundo (no ativo, como no desktop) e a
          // subclasse escolhida. Quem veio do v9 já tem `fund_details` sem a coluna;
          // quem veio de antes acabou de criá-la no passo acima, já com a coluna.
          await db.execute('ALTER TABLE assets ADD COLUMN cnpj TEXT');
          if (oldVersion >= 9) {
            await db.execute(
              'ALTER TABLE fund_details ADD COLUMN cvm_subclass TEXT',
            );
          }
        }
        if (oldVersion < 11) {
          // Compensação de prejuízo entre fundos: a administradora (ou intermediário)
          // agrupa os fundos que se compensam. Quem veio de antes do v9 acabou de criar
          // `fund_details` já com a coluna.
          if (oldVersion >= 9) {
            await db.execute(
              'ALTER TABLE fund_details ADD COLUMN administrator TEXT',
            );
          }
        }
        if (oldVersion < 12) {
          // Fundos fechados e exclusivos (Lei 14.754): a estrutura do fundo e a opção
          // do estoque de 2023. Mesma regra do v10 e do v11: quem veio de antes do v9
          // acabou de criar `fund_details` já com as colunas.
          if (oldVersion >= 9) {
            await db.execute(
              "ALTER TABLE fund_details ADD COLUMN structure TEXT NOT NULL DEFAULT 'aberto'",
            );
            await db.execute(
              'ALTER TABLE fund_details ADD COLUMN estoque_option TEXT',
            );
          }
        }
        if (oldVersion < 13) {
          // Crédito de prejuízo que a administradora usou num come-cotas registrado
          // (rendimento abatido, em BRL); linhas antigas ficam sem valor (nulo =
          // não informado). `portfolio_transactions` existe em toda versão antiga.
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN loss_credit_used REAL',
          );
        }
        if (oldVersion < 14) {
          // Grupo, código e país da ficha "Bens e Direitos", informados pelo usuário em
          // cada ativo; linhas antigas ficam sem valor (nulo = não informado).
          for (final column in ['dirpf_group', 'dirpf_code', 'dirpf_country']) {
            await db.execute('ALTER TABLE assets ADD COLUMN $column TEXT');
          }
        }
        if (oldVersion < 15) {
          // Dia em que o imposto retido no exterior foi pago (o crédito converte pela
          // PTAX de compra dele); linhas antigas ficam sem valor (nulo = o dia do
          // provento).
          await db.execute(
            'ALTER TABLE portfolio_transactions ADD COLUMN foreign_tax_paid_on TEXT',
          );
        }
      },
    );
  }

  /// Tipo tributário do fundo e valores de cota lançados (mesmas tabelas do
  /// desktop; `source` já prevê a leitura da CVM, que o mobile ainda não faz).
  static Future<void> _createFundTables(Database db) async {
    await db.execute('''
      CREATE TABLE fund_details (
        asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
        term TEXT NOT NULL,
        cvm_subclass TEXT,
        administrator TEXT,
        structure TEXT NOT NULL DEFAULT 'aberto',
        estoque_option TEXT
      )
    ''');
    await db.execute('''
      CREATE TABLE fund_quota_values (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
        quota_date TEXT NOT NULL,
        value REAL NOT NULL,
        source TEXT NOT NULL DEFAULT 'manual',
        notes TEXT,
        created_at TEXT NOT NULL
      )
    ''');
  }
}

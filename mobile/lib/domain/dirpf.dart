// Grupo, código e país da ficha "Bens e Direitos" da declaração, informados pelo usuário em
// cada ativo. Porta de `normalize_dirpf_field` de `commands/asset.rs` do desktop. O app só
// completa zeros à esquerda; não confere se o par existe na tabela da Receita (as fontes
// divergem e a tabela oficial não é lida aqui).

/// Normaliza um campo da ficha: vazio vira `null`; só dígitos, até [maxLen]; completa com
/// zeros à esquerda até [maxLen] (`3` → `03`), o que é só formatação. Lança
/// [FormatException] com o nome do campo para valor inválido.
String? normalizeDirpfField(String label, String? raw, int maxLen) {
  final value = raw?.trim() ?? '';
  if (value.isEmpty) return null;
  if (!RegExp(r'^\d+$').hasMatch(value) || value.length > maxLen) {
    throw FormatException('$label must be up to $maxLen digits');
  }
  return value.padLeft(maxLen, '0');
}

/// Tamanho de cada campo: grupo e código com 2 dígitos, país com 3.
const dirpfGroupLength = 2;
const dirpfCodeLength = 2;
const dirpfCountryLength = 3;

/// "Grupo 03 · Código 01 · País 249" com o que o usuário informou; vazio quando nenhum dos
/// três foi informado.
String dirpfFichaText({String? group, String? code, String? country}) => [
  if (group != null && group.isNotEmpty) 'Grupo $group',
  if (code != null && code.isNotEmpty) 'Código $code',
  if (country != null && country.isNotEmpty) 'País $country',
].join(' · ');

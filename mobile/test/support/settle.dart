import 'package:flutter_test/flutter_test.dart';

// Esperas dos testes de tela. O SQLite ffi e os `Future`s reais correm no relógio **real**
// (`runAsync`); as animações e rotas correm no relógio **falso** (`pump`). Por isso as duas
// coisas andam juntas aqui.

/// Tempo fixo: 20 voltas de 25 ms reais, cada uma seguida de um `pump(frame)`.
///
/// Telas com rota (a transição corre no relógio falso e precisa de tempo para o botão do canto
/// entrar na tela) passam `frame: Duration(milliseconds: 50)`; lista simples usa o padrão.
Future<void> settle(WidgetTester tester, {Duration frame = Duration.zero}) async {
  for (var i = 0; i < 20; i++) {
    await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
    await tester.pump(frame);
  }
}

/// Espera a condição real além do tempo fixo: sob carga o banco, o `pop` da rota e o redesenho
/// passam do `settle`. Roda o [settle] **inteiro primeiro** e só depois sonda [done], até 200
/// voltas. Sondar sem o `settle` antes derrubou o TT7 do `theses_screen_test`: a condição "o
/// campo existe" vale já no 1º quadro, com a rota ainda deslizando fora da tela.
Future<void> settleUntil(
  WidgetTester tester,
  bool Function() done,
  String what, {
  Duration frame = Duration.zero,
}) async {
  await settle(tester, frame: frame);
  for (var i = 0; i < 200; i++) {
    if (done()) return;
    await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
    await tester.pump(frame);
  }
  fail('Não chegou a: $what');
}

/// Espera [finder] aparecer.
Future<void> waitFor(WidgetTester tester, Finder finder, String what, {Duration frame = Duration.zero}) =>
    settleUntil(tester, () => finder.evaluate().isNotEmpty, what, frame: frame);

/// Espera [finder] sumir.
Future<void> waitGone(WidgetTester tester, Finder finder, String what, {Duration frame = Duration.zero}) =>
    settleUntil(tester, () => finder.evaluate().isEmpty, what, frame: frame);

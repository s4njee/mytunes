import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart' show LicensePage;
import 'package:mytunes_pod/main.dart';
import 'package:mytunes_pod/pod_shell.dart';
import 'package:mytunes_pod/theme/font_licences.dart';
import 'package:mytunes_pod/theme/mt_theme.dart';
import 'package:mytunes_pod/theme/tokens.g.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('the portrait shell reacts to system appearance', (tester) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(402, 874);
    tester.view.viewPadding = const FakeViewPadding(top: 59, bottom: 34);
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.light;
    addTearDown(tester.view.reset);
    addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);

    await tester.pumpWidget(const MyTunesPod());
    final shell = find.byType(PodShell);
    final eyebrow = find.text('LIBRARY');
    final systemBars = find.byType(AnnotatedRegion<SystemUiOverlayStyle>);
    final shellElement = tester.element(shell);
    expect(find.text('0 songs · 0 albums'), findsOneWidget);
    expect(tester.getSize(shell), const Size(402, 874));
    expect(tester.getTopLeft(eyebrow).dx, MtSpace.gutter);
    expect(tester.getTopLeft(eyebrow).dy, closeTo(MtSpace.displayTop, 1));
    expect(
      tester.getTopRight(find.text('0 songs · 0 albums')).dx,
      402 - MtSpace.gutter,
    );
    expect(
      DefaultTextStyle.of(tester.element(eyebrow)).style.decoration,
      TextDecoration.none,
    );

    void expectAppearance(MtTheme theme, Brightness icons) {
      expect(MtTheme.of(shellElement), same(theme));
      expect(tester.widget<Text>(eyebrow).style?.fontFamily, MtFonts.mono);
      expect(tester.widget<Text>(eyebrow).style?.color, theme.colors.accent);
      final bars = tester.widget<AnnotatedRegion<SystemUiOverlayStyle>>(
        systemBars,
      );
      expect(bars.value.statusBarBrightness, theme.brightness);
      expect(bars.value.statusBarIconBrightness, icons);
      expect(bars.value.statusBarColor, const Color(0x00000000));
    }

    expectAppearance(MtTheme.light, Brightness.dark);
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
    await tester.pumpAndSettle();
    expect(tester.element(shell), same(shellElement));
    expectAppearance(MtTheme.dark, Brightness.light);
  });

  testWidgets('the licences route opens the Flutter licences page', (
    tester,
  ) async {
    await tester.pumpWidget(const MyTunesPod());
    final navigator = tester.state<NavigatorState>(find.byType(Navigator));
    navigator.pushNamed<void>('/licences');
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 350));
    expect(find.byType(LicensePage), findsOneWidget);
  });

  test(
    'the bundled font licences are registered for the licences page',
    () async {
      registerFontLicences();
      final fontNotices = <String, String>{};
      await for (final entry in LicenseRegistry.licenses) {
        for (final name in entry.packages) {
          if (name == MtFonts.sans || name == MtFonts.mono) {
            fontNotices[name] = entry.paragraphs
                .map((part) => part.text)
                .join('\n');
          }
        }
      }
      expect(fontNotices.keys, unorderedEquals([MtFonts.sans, MtFonts.mono]));
      for (final notice in fontNotices.values) {
        expect(notice, contains('SIL OPEN FONT LICENSE'));
      }
    },
  );

  test('generated tokens match the design handoff', () {
    expect(MtColors.dark.accent.toARGB32(), 0xFFF0A23C);
    expect(MtColors.light.accent.toARGB32(), 0xFF2F6DB5);
    expect(MtWheel.diameter, 252);
    expect(MtWheel.detentDegrees, 12);
  });
}

import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';

import 'tokens.g.dart';

/// Platform appearance and the corresponding wheel-first colour roles.
class MtTheme {
  const MtTheme._(this.brightness, this.colors);

  static const light = MtTheme._(Brightness.light, MtColors.light);
  static const dark = MtTheme._(Brightness.dark, MtColors.dark);

  final Brightness brightness;
  final MtColors colors;

  static MtTheme of(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark ? dark : light;

  SystemUiOverlayStyle get systemUi => SystemUiOverlayStyle(
    statusBarColor: Colors.transparent,
    statusBarBrightness: brightness,
    statusBarIconBrightness: _iconBrightness,
    systemStatusBarContrastEnforced: false,
    systemNavigationBarColor: Colors.transparent,
    systemNavigationBarIconBrightness: _iconBrightness,
    systemNavigationBarContrastEnforced: false,
  );

  Brightness get _iconBrightness =>
      brightness == Brightness.dark ? Brightness.light : Brightness.dark;

  ThemeData get materialTheme => ThemeData(
    brightness: brightness,
    fontFamily: MtFonts.sans,
    scaffoldBackgroundColor: colors.bg,
    colorScheme: ColorScheme.fromSeed(
      seedColor: colors.accent,
      brightness: brightness,
    ).copyWith(surface: colors.bg, onSurface: colors.textPrimary),
    appBarTheme: AppBarTheme(systemOverlayStyle: systemUi),
  );
}

/// A box shadow whose blur radius follows the CSS handoff.
class CssBoxShadow extends BoxShadow {
  const CssBoxShadow({
    required super.color,
    required super.offset,
    required super.blurRadius,
  });

  @override
  double get blurSigma => blurRadius / 2;
}

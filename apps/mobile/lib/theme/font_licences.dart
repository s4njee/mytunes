import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Adds the bundled font licences to Flutter's licences page.
void registerFontLicences() {
  LicenseRegistry.addLicense(() async* {
    const fonts = {
      'IBM Plex Sans': 'assets/fonts/IBM-Plex-Sans-OFL.txt',
      'JetBrains Mono': 'assets/fonts/JetBrains-Mono-OFL.txt',
    };
    for (final font in fonts.entries) {
      yield LicenseEntryWithLineBreaks([
        font.key,
      ], await rootBundle.loadString(font.value));
    }
  });
}

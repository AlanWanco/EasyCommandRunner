# Bundled font

- Font: **Sarasa Mono SC Regular**, version **1.0.41** (unmodified, hinted TTF).
- Project: https://github.com/be5invis/Sarasa-Gothic
- Release: https://github.com/be5invis/Sarasa-Gothic/releases/tag/v1.0.41
- Archive: `SarasaMonoSC-TTF-1.0.41.7z` (only the Regular face is included).
- License: SIL Open Font License 1.1; copyright and full license in `OFL-Sarasa.txt`.
- Archive SHA-256 (verified against upstream `SHA-256.txt`):
  `cd078f6103af293c8943f9a333beee2ecf1fe5b6901950a4f610eb37e1b3c0f2`
- TTF SHA-256:
  `95718854631c2386845e618aa5bc37908e5fb8a919eb56e40f70e1170d80af94`

The font and license are embedded via Qt resources. The application registers the
font with `QFontDatabase::addApplicationFont` before creating widgets, and uses it
in both themes for UI, command editing, previews and run logs. No OS installation
or runtime download is needed. It supports Latin and CJK text (two Latin cells per
full-width CJK character). The license is also viewable under Help → About → Details.

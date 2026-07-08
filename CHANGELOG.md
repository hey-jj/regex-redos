# Changelog

## [0.2.0] - 2026-07-07

### Changed
- Deeply nested group patterns now return unsafe before they can exhaust the process stack. (#11)
- Invalid or duplicate named capture names now return unsafe. Valid Unicode names stay safe. (#13)
- Reversed character class ranges now return unsafe. Valid class range edge cases keep their expected safe result. (#12)

## [0.2.0] - 2026-07-07

### Changed
- Deeply nested group patterns now return unsafe before they can exhaust the process stack. (#11)
- Invalid or duplicate named capture names now return unsafe. Valid Unicode names stay safe. (#13)
- Reversed character class ranges now return unsafe. Valid class range edge cases keep their expected safe result. (#12)

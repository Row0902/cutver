# Feature: cutver JSON format-preserving editor (issue #4)

Goal: close GitHub issue Row0902/cutver#4 — the JSON manifest editor must not
reorder keys or drop the trailing newline; ideally preserve the document
byte-for-byte except the version value substring.

Origin: native review findings R2-003/R3-3 (lineage review-27ee2d4ca1e46eea),
verified with an unsorted-keys fixture.

Approach decision: implement a targeted text-edit locator (scanner) instead of
re-serializing with serde_json. serde_json still parses for `read_version`
validation; `write_version` replaces only the exact value substring located by
the scanner — byte-perfect preservation of order, indentation, newline, and
spacing, no new dependency, no preserve_order feature needed.

## Tasks

- [ ] J1. `src/manifest/json_scan.rs`: structural scanner that locates the
      byte span of the string value at a dotted field path in raw JSON text
      (object key path traversal, arrays by index, string-escape aware),
      with unit tests (top-level, nested, arrays, duplicate key names in
      sibling branches, escaped strings, missing path errors).
- [ ] J2. Wire into `src/manifest/json.rs` `write_version`: replace only the
      located value span; update/extend unit tests (order preservation,
      newline preservation, indentation preservation, failure cases).
- [ ] J3. End-to-end verification: repro fixture with unsorted keys + no
      trailing newline must produce a diff containing only the version line;
      update design.md manifest table wording; close issue #4 with comment.

## Evidence log

| Task | Commit | Checks |
| --- | --- | --- |

# Examples

## Parse
```bash
zpl parse samples/usps_surepost_sample.zpl
```

## Lint with profile
```bash
zpl lint samples/usps_surepost_sample.zpl --profile profiles/zebra-generic-203.json
```

## Format
```bash
# Print formatted output to stdout
zpl format samples/usps_surepost_sample.zpl

# Overwrite file in-place
zpl format samples/usps_surepost_sample.zpl --write

# Check formatting in CI (exit 1 if not formatted)
zpl format samples/usps_surepost_sample.zpl --check

# Format with label indentation
zpl format samples/usps_surepost_sample.zpl --indent label
```

## Coverage summary (human)
```bash
zpl coverage --coverage generated/coverage.json
```

## Coverage summary (JSON)
```bash
zpl coverage --coverage generated/coverage.json --json
```

## Explain a diagnostic
```bash
zpl explain ZPL1401
```

## Force output mode
```bash
# Force JSON output (auto-detected for pipes)
zpl --output json lint samples/usps_surepost_sample.zpl# Force pretty output (auto-detected for terminals)
zpl --output pretty lint samples/usps_surepost_sample.zpl
```

//! Vendored AutoEQ CSV recognition, deliberately retaining its regex grammar.
use super::{FrFields, FrequencyResponse, invalid};
use crate::DspError;
use regex::Regex;
use std::path::Path;

impl FrequencyResponse {
    /// Python read_from_csv (181-242); p09_csv_*.json. Python text-mode reads
    /// normalize CRLF/CR to LF; parse_csv itself retains literal line endings.
    /// The name is everything in the filename before its last dot; no BOM trim.
    pub fn read_from_csv(path: &Path) -> Result<Self, DspError> {
        let filename = path
            .file_name()
            .and_then(|p| p.to_str())
            .ok_or_else(|| invalid("CSV filename is not UTF-8"))?;
        let name = filename.rsplit_once('.').map_or("", |(name, _)| name);
        let text = std::fs::read_to_string(path)
            .map_err(|e| DspError::InvalidArgument(format!("{}: {e}", path.display())))?;
        Self::parse_csv(name, &text.replace("\r\n", "\n").replace('\r', "\n"))
    }

    /// Python read_from_csv parsing branches (190-242); p09_csv_*.json.
    /// Float grammar requires two digit characters, does not parse exponents,
    /// and the strict header starts at byte zero. Guessing uses only the first
    /// two floats of each matching line. No caller-side error=-raw fixup here.
    pub fn parse_csv(name: &str, text: &str) -> Result<Self, DspError> {
        let columns = "raw|smoothed|error|error_smoothed|equalization|parametric_eq|fixed_band_eq|equalized_raw|equalized_smoothed|target";
        let float = r"-?\d+\.?\d+";
        let header = format!("frequency(,({columns}))+");
        let data_n = format!(r"{float}([ ,;:\t]+{float})+?");
        let strict =
            Regex::new(&format!(r"^{header}(\n{data_n})+\n*$")).expect("constant AutoEQ regex");
        let mut fields = FrFields::default();
        let mut frequency = Vec::new();
        if strict.is_match(text) {
            let mut lines = text.split('\n');
            let names: Vec<_> = lines.next().unwrap().split(',').collect();
            let rows: Vec<Vec<_>> = lines
                .filter(|l| !l.is_empty())
                .map(|l| l.split(',').collect())
                .collect();
            let column = |name: &str| -> Result<Option<Vec<f64>>, DspError> {
                // DictReader retains the final value for duplicate header names.
                let Some(index) = names.iter().rposition(|&n| n == name) else {
                    return Ok(None);
                };
                rows.iter()
                    .map(|row| {
                        row.get(index)
                            .ok_or_else(|| invalid("missing CSV value"))?
                            .trim()
                            .parse::<f64>()
                            .map_err(|_| invalid("invalid CSV float"))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(Some)
            };
            frequency = column("frequency")?.unwrap_or_default();
            fields = FrFields {
                raw: column("raw")?,
                smoothed: column("smoothed")?,
                error: column("error")?,
                error_smoothed: column("error_smoothed")?,
                equalization: column("equalization")?,
                equalized_raw: column("equalized_raw")?,
                equalized_smoothed: column("equalized_smoothed")?,
                target: column("target")?,
            };
            // Python parses these columns even though the app never uses them.
            column("parametric_eq")?;
            column("fixed_band_eq")?;
        } else {
            let data_two =
                Regex::new(&format!(r"^{float}[ ,;:\t]+{float}?")).expect("constant AutoEQ regex");
            let floats = Regex::new(float).expect("constant AutoEQ regex");
            let mut raw = Vec::new();
            for line in text.split('\n') {
                if data_two.is_match(line) {
                    let mut matches = floats.find_iter(line);
                    let mut value = || {
                        matches
                            .next()
                            .ok_or_else(|| invalid("missing guessed CSV float"))?
                            .as_str()
                            .parse::<f64>()
                            .map_err(|_| invalid("invalid guessed CSV float"))
                    };
                    frequency.push(value()?);
                    raw.push(value()?);
                }
            }
            fields.raw = Some(raw);
        }
        Self::with_fields(name, Some(frequency), fields)
    }
}

import type { CsvEncoding, CsvMapping, DecimalSeparator } from "../bindings";

export const DELIMITERS: { value: string; label: string }[] = [
  { value: ";", label: "Point-virgule ( ; )" },
  { value: ",", label: "Virgule ( , )" },
  { value: "\t", label: "Tabulation" },
  { value: "|", label: "Barre verticale ( | )" },
];

export const ENCODINGS: { value: CsvEncoding; label: string }[] = [
  { value: "WINDOWS-1252", label: "Windows-1252 (banques françaises)" },
  { value: "ISO-8859-1", label: "ISO-8859-1 (Latin-1)" },
  { value: "UTF-8", label: "UTF-8" },
];

/** Date formats of bank exports, as `chrono` patterns. */
export const DATE_FORMATS: { value: string; label: string }[] = [
  { value: "%d/%m/%Y", label: "JJ/MM/AAAA (20/10/2026)" },
  { value: "%d/%m/%y", label: "JJ/MM/AA (20/10/26)" },
  { value: "%Y-%m-%d", label: "AAAA-MM-JJ (2026-10-20)" },
  { value: "%d-%m-%Y", label: "JJ-MM-AAAA (20-10-2026)" },
  { value: "%d.%m.%Y", label: "JJ.MM.AAAA (20.10.2026)" },
];

export const DECIMAL_SEPARATORS: { value: DecimalSeparator; label: string }[] = [
  { value: ",", label: "Virgule (1 234,56)" },
  { value: ".", label: "Point (1,234.56)" },
];

/** Usual French bank export: `;`, Windows-1252, JJ/MM/AAAA, separate debit and credit. */
export function defaultMapping(): CsvMapping {
  return {
    delimiter: ";",
    encoding: "WINDOWS-1252",
    dateFormat: "%d/%m/%Y",
    decimalSeparator: ",",
    columnDate: 0,
    columnLabel: 1,
    amountColumns: { kind: "debitCredit", debit: 2, credit: 3 },
    skipLines: 1,
  };
}

/**
 * Splits a CSV line into fields, honouring double quotes, to show the file as columns while
 * defining a mapping. The backend parser remains the reference.
 */
export function splitLine(line: string, delimiter: string): string[] {
  const fields: string[] = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < line.length; i++) {
    const char = line[i];
    if (quoted) {
      if (char === '"' && line[i + 1] === '"') {
        field += '"';
        i++;
      } else if (char === '"') {
        quoted = false;
      } else {
        field += char;
      }
    } else if (char === '"') {
      quoted = true;
    } else if (char === delimiter) {
      fields.push(field);
      field = "";
    } else {
      field += char;
    }
  }
  fields.push(field);
  return fields;
}
